<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import {
    Container, Box, Network, Layers,
    Play, Square, RotateCw, Trash2, RefreshCw,
    Search, Eye, EyeOff, Terminal, Download,
    Plus, X, KeyRound, AlertCircle, Loader2,
    ChevronRight, ChevronLeft, FolderOpen, FolderPlus,
    FileText, Pause, Eraser, Info, ArrowUp, ArrowDown,
    Unplug, ChevronsUpDown
  } from 'lucide-svelte';

  // Props
  let { onRequestTerminalExec = (containerId: string) => {} } = $props();

  // Sub-tab state
  let dockerTab = $state<'containers' | 'images' | 'networks' | 'compose'>('containers');

  // Global loading/error
  let isLoading = $state(false);
  let errorMsg = $state('');
  let successMsg = $state('');

  // Docker status
  let dockerInstalled = $state(true);
  let dockerVersion = $state('');
  let useSudo = $state(false);

  // Summary stats
  let runningCount = $state(0);
  let stoppedCount = $state(0);
  let totalImages = $state(0);
  let totalNetworks = $state(0);

  // Sudo auth state
  let showSudoModal = $state(false);
  let sudoPassword = $state('');
  let pendingAction: (() => Promise<void>) | null = null;
  let sudoError = $state('');

  // Containers state
  let containers = $state<any[]>([]);
  let containerSearch = $state('');
  let expandedContainer = $state('');
  let containerInspectData = $state('');
  let inspectLoading = $state(false);

  // Container logs state
  let showLogsModal = $state(false);
  let logsContainerId = $state('');
  let logsContainerName = $state('');
  let logLines = $state<string[]>([]);
  let logsPaused = $state(false);
  let logSearch = $state('');
  let logsUnlisten: (() => void) | null = null;
  let logScrollEl = $state<HTMLDivElement | undefined>(undefined);

  // Container exec state
  let showExecModal = $state(false);
  let execContainerId = $state('');
  let execContainerName = $state('');
  let execCommand = $state('');
  let execOutput = $state('');
  let execRunning = $state(false);

  // Images state
  let images = $state<any[]>([]);
  let imageSearch = $state('');
  let showPullModal = $state(false);
  let pullImageName = $state('');
  let pullProgress = $state('');
  let isPulling = $state(false);

  // Networks state
  let networks = $state<any[]>([]);
  let expandedNetwork = $state('');
  let networkInspectData = $state('');
  let networkInspectLoading = $state(false);
  let showCreateNetworkModal = $state(false);
  let newNetworkName = $state('');
  let newNetworkDriver = $state('bridge');

  // Compose state
  let composeProjects = $state<any[]>([]);
  let showDirPicker = $state(false);
  let dirPickerPath = $state('');
  let dirPickerEntries = $state<any[]>([]);
  let dirPickerLoading = $state(false);
  let newProjectFolder = $state('');
  let showComposeEditor = $state(false);
  let composeEditorPath = $state('');
  let composeEditorContent = $state('');
  let composeEditorSaving = $state(false);
  let composeLogs = $state<string[]>([]);
  let showComposeLogsModal = $state(false);
  let composeLogsProjectName = $state('');

  // Confirm delete state
  let showConfirmModal = $state(false);
  let confirmMessage = $state('');
  let confirmAction: (() => Promise<void>) | null = null;

  // ----------------------------------------------------------
  // Sudo-aware Docker exec helper
  // ----------------------------------------------------------
  async function execDocker(cmd: string): Promise<string> {
    try {
      const result: string = await invoke('exec_custom_command', { cmd, useSudo });
      return result;
    } catch (err: any) {
      const errStr = err.toString().toLowerCase();
      if (errStr.includes('permission denied') || errStr.includes('got permission denied')) {
        // Retry with sudo
        useSudo = true;
        try {
          const result: string = await invoke('exec_custom_command', { cmd, useSudo: true });
          return result;
        } catch (err2: any) {
          if (err2.toString() === 'SUDO_PASSWORD_REQUIRED') {
            throw new Error('SUDO_PASSWORD_REQUIRED');
          }
          throw err2;
        }
      }
      if (err.toString() === 'SUDO_PASSWORD_REQUIRED') {
        throw new Error('SUDO_PASSWORD_REQUIRED');
      }
      throw err;
    }
  }

  async function handleWithSudo(action: () => Promise<void>) {
    try {
      await action();
    } catch (err: any) {
      if (err.toString().includes('SUDO_PASSWORD_REQUIRED')) {
        pendingAction = () => handleWithSudo(action);
        showSudoModal = true;
      } else {
        errorMsg = err.toString();
      }
    }
  }

  async function submitSudoPassword() {
    sudoError = '';
    try {
      await invoke('set_sudo_password', { password: sudoPassword });
      showSudoModal = false;
      sudoPassword = '';
      if (pendingAction) {
        const action = pendingAction;
        pendingAction = null;
        await action();
      }
    } catch (err: any) {
      sudoError = err.toString();
    }
  }

  // ----------------------------------------------------------
  // Docker status check
  // ----------------------------------------------------------
  async function checkDockerStatus() {
    isLoading = true;
    errorMsg = '';
    try {
      const versionOut = await execDocker("docker info --format '{{.ServerVersion}}'");
      dockerVersion = versionOut.trim();
      dockerInstalled = true;
    } catch (err: any) {
      const errStr = err.toString().toLowerCase();
      if (errStr.includes('SUDO_PASSWORD_REQUIRED')) {
        pendingAction = checkDockerStatus;
        showSudoModal = true;
        isLoading = false;
        return;
      }
      if (errStr.includes('not found') || errStr.includes('not installed') || errStr.includes('command not found')) {
        dockerInstalled = false;
        isLoading = false;
        return;
      }
      // Docker might be installed but daemon not running
      if (errStr.includes('cannot connect') || errStr.includes('is the docker daemon running')) {
        dockerInstalled = true;
        dockerVersion = '';
        errorMsg = 'Docker jest zainstalowany, ale demon nie jest uruchomiony. Uruchom go za pomocą: systemctl start docker';
        isLoading = false;
        return;
      }
      errorMsg = 'Błąd sprawdzania Docker: ' + err.toString();
    } finally {
      isLoading = false;
    }
  }

  // ----------------------------------------------------------
  // Containers
  // ----------------------------------------------------------
  async function loadContainers() {
    isLoading = true;
    errorMsg = '';
    try {
      const output = await execDocker("docker ps -a --format '{{json .}}'");
      const lines = output.trim().split('\n').filter(l => l.trim());
      const parsed = lines.map(line => {
        try { return JSON.parse(line); }
        catch { return null; }
      }).filter(Boolean);
      containers = parsed;

      runningCount = parsed.filter((c: any) => c.State === 'running').length;
      stoppedCount = parsed.filter((c: any) => c.State !== 'running').length;
    } catch (err: any) {
      if (err.toString().includes('SUDO_PASSWORD_REQUIRED')) {
        pendingAction = loadContainers;
        showSudoModal = true;
      } else {
        errorMsg = 'Błąd wczytywania kontenerów: ' + err.toString();
      }
    } finally {
      isLoading = false;
    }
  }

  function getFilteredContainers() {
    if (!containerSearch) return containers;
    const q = containerSearch.toLowerCase();
    return containers.filter(c =>
      (c.Names || '').toLowerCase().includes(q) ||
      (c.Image || '').toLowerCase().includes(q) ||
      (c.ID || '').toLowerCase().includes(q)
    );
  }

  function getStatusBadge(state: string) {
    switch (state?.toLowerCase()) {
      case 'running': return 'success';
      case 'paused': return 'warning';
      case 'exited': case 'dead': case 'created': return 'danger';
      default: return 'warning';
    }
  }

  async function containerAction(action: string, id: string) {
    await handleWithSudo(async () => {
      isLoading = true;
      await execDocker(`docker ${action} ${id}`);
      await loadContainers();
      successMsg = `Kontener ${action === 'start' ? 'uruchomiony' : action === 'stop' ? 'zatrzymany' : action === 'restart' ? 'zrestartowany' : 'usunięty'}`;
      setTimeout(() => successMsg = '', 3000);
    });
  }

  async function removeContainer(id: string, name: string) {
    confirmMessage = `Usunąć kontener "${name}"? Ta operacja jest nieodwracalna.`;
    confirmAction = async () => {
      await containerAction('rm -f', id);
      showConfirmModal = false;
    };
    showConfirmModal = true;
  }

  async function inspectContainer(id: string) {
    if (expandedContainer === id) {
      expandedContainer = '';
      return;
    }
    expandedContainer = id;
    inspectLoading = true;
    containerInspectData = '';
    try {
      const output = await execDocker(`docker inspect ${id}`);
      containerInspectData = JSON.stringify(JSON.parse(output), null, 2);
    } catch (err: any) {
      containerInspectData = 'Błąd inspekcji: ' + err.toString();
    } finally {
      inspectLoading = false;
    }
  }

  // ----------------------------------------------------------
  // Container Logs
  // ----------------------------------------------------------
  async function openLogs(id: string, name: string) {
    logsContainerId = id;
    logsContainerName = name;
    logLines = [];
    logsPaused = false;
    logSearch = '';
    showLogsModal = true;

    try {
      logsUnlisten = await listen('docker-log-data', (event: any) => {
        if (!logsPaused) {
          logLines = [...logLines, event.payload as string];
          // Auto-scroll
          if (logScrollEl) {
            requestAnimationFrame(() => {
              if (logScrollEl) logScrollEl.scrollTop = logScrollEl.scrollHeight;
            });
          }
        }
      });
      await invoke('start_container_logs', { containerId: id, tail: 200 });
    } catch (err: any) {
      logLines = ['Błąd rozpoczęcia streamowania logów: ' + err.toString()];
    }
  }

  async function closeLogs() {
    try {
      await invoke('stop_container_logs');
    } catch { /* ignore */ }
    if (logsUnlisten) {
      logsUnlisten();
      logsUnlisten = null;
    }
    showLogsModal = false;
  }

  function getFilteredLogs() {
    if (!logSearch) return logLines;
    const q = logSearch.toLowerCase();
    return logLines.filter(line => line.toLowerCase().includes(q));
  }

  function downloadLogs() {
    const blob = new Blob([logLines.join('\n')], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${logsContainerName || logsContainerId}_logs.txt`;
    a.click();
    URL.revokeObjectURL(url);
  }

  // ----------------------------------------------------------
  // Container Exec
  // ----------------------------------------------------------
  function openExec(id: string, name: string) {
    execContainerId = id;
    execContainerName = name;
    execCommand = '';
    execOutput = '';
    showExecModal = true;
  }

  async function runExec() {
    if (!execCommand.trim()) return;
    execRunning = true;
    execOutput = '';
    try {
      const output = await execDocker(`docker exec ${execContainerId} ${execCommand}`);
      execOutput = output;
    } catch (err: any) {
      if (err.toString().includes('SUDO_PASSWORD_REQUIRED')) {
        pendingAction = runExec;
        showSudoModal = true;
      } else {
        execOutput = 'Błąd: ' + err.toString();
      }
    } finally {
      execRunning = false;
    }
  }

  function openInteractiveShell(containerId: string) {
    onRequestTerminalExec(containerId);
  }

  // ----------------------------------------------------------
  // Images
  // ----------------------------------------------------------
  async function loadImages() {
    isLoading = true;
    errorMsg = '';
    try {
      const output = await execDocker("docker images --format '{{json .}}'");
      const lines = output.trim().split('\n').filter(l => l.trim());
      const parsed = lines.map(line => {
        try { return JSON.parse(line); }
        catch { return null; }
      }).filter(Boolean);
      images = parsed;
      totalImages = parsed.length;
    } catch (err: any) {
      if (err.toString().includes('SUDO_PASSWORD_REQUIRED')) {
        pendingAction = loadImages;
        showSudoModal = true;
      } else {
        errorMsg = 'Błąd wczytywania obrazów: ' + err.toString();
      }
    } finally {
      isLoading = false;
    }
  }

  function getFilteredImages() {
    if (!imageSearch) return images;
    const q = imageSearch.toLowerCase();
    return images.filter(i =>
      (i.Repository || '').toLowerCase().includes(q) ||
      (i.Tag || '').toLowerCase().includes(q) ||
      (i.ID || '').toLowerCase().includes(q)
    );
  }

  async function pullImage() {
    if (!pullImageName.trim()) return;
    isPulling = true;
    pullProgress = 'Pobieranie...';
    try {
      const output = await execDocker(`docker pull ${pullImageName.trim()}`);
      pullProgress = output;
      await loadImages();
      successMsg = `Obraz "${pullImageName}" pobrany pomyślnie`;
      setTimeout(() => successMsg = '', 3000);
      setTimeout(() => { showPullModal = false; pullImageName = ''; pullProgress = ''; }, 1500);
    } catch (err: any) {
      if (err.toString().includes('SUDO_PASSWORD_REQUIRED')) {
        pendingAction = pullImage;
        showSudoModal = true;
      } else {
        pullProgress = 'Błąd pobierania: ' + err.toString();
      }
    } finally {
      isPulling = false;
    }
  }

  async function removeImage(id: string, name: string) {
    confirmMessage = `Usunąć obraz "${name}"?`;
    confirmAction = async () => {
      await handleWithSudo(async () => {
        isLoading = true;
        await execDocker(`docker rmi ${id}`);
        await loadImages();
        showConfirmModal = false;
        successMsg = 'Obraz usunięty';
        setTimeout(() => successMsg = '', 3000);
      });
    };
    showConfirmModal = true;
  }

  async function pruneImages() {
    confirmMessage = 'Usunąć wszystkie nieużywane obrazy? Zwolni to przestrzeń dyskową.';
    confirmAction = async () => {
      await handleWithSudo(async () => {
        isLoading = true;
        const output = await execDocker('docker image prune -f');
        await loadImages();
        showConfirmModal = false;
        successMsg = 'Nieużywane obrazy usunięte. ' + output.trim().split('\n').pop();
        setTimeout(() => successMsg = '', 5000);
      });
    };
    showConfirmModal = true;
  }

  // ----------------------------------------------------------
  // Networks
  // ----------------------------------------------------------
  async function loadNetworks() {
    isLoading = true;
    errorMsg = '';
    try {
      const output = await execDocker("docker network ls --format '{{json .}}'");
      const lines = output.trim().split('\n').filter(l => l.trim());
      const parsed = lines.map(line => {
        try { return JSON.parse(line); }
        catch { return null; }
      }).filter(Boolean);
      networks = parsed;
      totalNetworks = parsed.length;
    } catch (err: any) {
      if (err.toString().includes('SUDO_PASSWORD_REQUIRED')) {
        pendingAction = loadNetworks;
        showSudoModal = true;
      } else {
        errorMsg = 'Błąd wczytywania sieci: ' + err.toString();
      }
    } finally {
      isLoading = false;
    }
  }

  async function inspectNetwork(id: string) {
    if (expandedNetwork === id) {
      expandedNetwork = '';
      return;
    }
    expandedNetwork = id;
    networkInspectLoading = true;
    networkInspectData = '';
    try {
      const output = await execDocker(`docker network inspect ${id}`);
      networkInspectData = JSON.stringify(JSON.parse(output), null, 2);
    } catch (err: any) {
      networkInspectData = 'Błąd inspekcji: ' + err.toString();
    } finally {
      networkInspectLoading = false;
    }
  }

  async function createNetwork() {
    if (!newNetworkName.trim()) return;
    await handleWithSudo(async () => {
      isLoading = true;
      await execDocker(`docker network create --driver ${newNetworkDriver} ${newNetworkName.trim()}`);
      showCreateNetworkModal = false;
      newNetworkName = '';
      newNetworkDriver = 'bridge';
      await loadNetworks();
      successMsg = 'Sieć utworzona';
      setTimeout(() => successMsg = '', 3000);
    });
  }

  async function removeNetwork(id: string, name: string) {
    confirmMessage = `Usunąć sieć "${name}"?`;
    confirmAction = async () => {
      await handleWithSudo(async () => {
        isLoading = true;
        await execDocker(`docker network rm ${id}`);
        await loadNetworks();
        showConfirmModal = false;
        successMsg = 'Sieć usunięta';
        setTimeout(() => successMsg = '', 3000);
      });
    };
    showConfirmModal = true;
  }

  // ----------------------------------------------------------
  // Compose
  // ----------------------------------------------------------
  async function loadComposeProjects() {
    isLoading = true;
    errorMsg = '';
    try {
      const output = await execDocker('docker compose ls --format json');
      try {
        const parsed = JSON.parse(output);
        composeProjects = Array.isArray(parsed) ? parsed : [];
      } catch {
        // If not valid JSON array, try line-by-line
        const lines = output.trim().split('\n').filter(l => l.trim());
        const parsed = lines.map(l => { try { return JSON.parse(l); } catch { return null; } }).filter(Boolean);
        composeProjects = parsed;
      }
    } catch (err: any) {
      if (err.toString().includes('SUDO_PASSWORD_REQUIRED')) {
        pendingAction = loadComposeProjects;
        showSudoModal = true;
      } else {
        // Compose might not be available
        composeProjects = [];
        if (!err.toString().toLowerCase().includes('not found')) {
          errorMsg = 'Błąd wczytywania projektów Compose: ' + err.toString();
        }
      }
    } finally {
      isLoading = false;
    }
  }

  async function composeAction(action: string, configFiles: string) {
    // configFiles may be comma-separated
    const firstFile = configFiles.split(',')[0].trim();
    await handleWithSudo(async () => {
      isLoading = true;
      errorMsg = '';
      try {
        await execDocker(`docker compose -f ${firstFile} ${action}`);
        await loadComposeProjects();
        successMsg = `Compose ${action} — wykonano`;
        setTimeout(() => successMsg = '', 3000);
      } catch (err: any) {
        errorMsg = 'Błąd Compose: ' + err.toString();
      } finally {
        isLoading = false;
      }
    });
  }

  // Directory picker for compose
  async function openDirPicker() {
    showDirPicker = true;
    dirPickerLoading = true;
    newProjectFolder = '';
    try {
      const home: string = await invoke('sftp_get_home_dir');
      dirPickerPath = home;
      await loadDirEntries(home);
    } catch (err: any) {
      errorMsg = 'Błąd otwierania przeglądarki katalogów: ' + err.toString();
      showDirPicker = false;
    } finally {
      dirPickerLoading = false;
    }
  }

  async function loadDirEntries(path: string) {
    dirPickerLoading = true;
    try {
      const entries: any[] = await invoke('sftp_list', { path });
      dirPickerEntries = entries
        .filter((e: any) => e.is_dir || e.file_type === 'directory')
        .sort((a: any, b: any) => (a.name || a.filename || '').localeCompare(b.name || b.filename || ''));
      dirPickerPath = path;
    } catch (err: any) {
      errorMsg = 'Błąd wczytywania katalogu: ' + err.toString();
    } finally {
      dirPickerLoading = false;
    }
  }

  function navigateDir(dirName: string) {
    const newPath = dirPickerPath.endsWith('/') ? dirPickerPath + dirName : dirPickerPath + '/' + dirName;
    loadDirEntries(newPath);
  }

  function navigateUp() {
    const parts = dirPickerPath.split('/').filter(Boolean);
    if (parts.length <= 1) {
      loadDirEntries('/');
    } else {
      parts.pop();
      loadDirEntries('/' + parts.join('/'));
    }
  }

  async function createComposeProject() {
    if (!newProjectFolder.trim()) return;
    const projectPath = dirPickerPath.endsWith('/')
      ? dirPickerPath + newProjectFolder.trim()
      : dirPickerPath + '/' + newProjectFolder.trim();
    const composePath = projectPath + '/docker-compose.yml';

    const starterYaml = `# Docker Compose - ${newProjectFolder.trim()}
# Utworzono przez Jarvis Server Manager

services:
  # app:
  #   image: nginx:latest
  #   ports:
  #     - "80:80"
  #   volumes:
  #     - ./data:/usr/share/nginx/html
  #   restart: unless-stopped

networks:
  default:
    driver: bridge
`;

    try {
      isLoading = true;
      await invoke('sftp_create_dir', { path: projectPath });
      await invoke('sftp_write', { path: composePath, content: starterYaml });
      showDirPicker = false;
      newProjectFolder = '';
      await loadComposeProjects();
      successMsg = `Projekt "${newProjectFolder.trim() || 'nowy'}" utworzony w ${projectPath}`;
      setTimeout(() => successMsg = '', 4000);
    } catch (err: any) {
      errorMsg = 'Błąd tworzenia projektu: ' + err.toString();
    } finally {
      isLoading = false;
    }
  }

  // Compose editor
  async function openComposeEditor(configFiles: string) {
    const filePath = configFiles.split(',')[0].trim();
    composeEditorPath = filePath;
    showComposeEditor = true;
    composeEditorContent = '';
    try {
      const content: string = await invoke('sftp_read', { path: filePath });
      composeEditorContent = content;
    } catch (err: any) {
      composeEditorContent = '# Błąd odczytu pliku: ' + err.toString();
    }
  }

  async function saveComposeFile() {
    composeEditorSaving = true;
    try {
      await invoke('sftp_write', { path: composeEditorPath, content: composeEditorContent });
      successMsg = 'Plik docker-compose.yml zapisany';
      setTimeout(() => successMsg = '', 3000);
      showComposeEditor = false;
    } catch (err: any) {
      errorMsg = 'Błąd zapisu pliku: ' + err.toString();
    } finally {
      composeEditorSaving = false;
    }
  }

  // Compose logs
  async function openComposeLogs(name: string, configFiles: string) {
    const firstFile = configFiles.split(',')[0].trim();
    composeLogsProjectName = name;
    composeLogs = [];
    showComposeLogsModal = true;

    try {
      const output = await execDocker(`docker compose -f ${firstFile} logs --tail 200`);
      composeLogs = output.trim().split('\n');
    } catch (err: any) {
      composeLogs = ['Błąd odczytu logów: ' + err.toString()];
    }
  }

  // ----------------------------------------------------------
  // Data loading
  // ----------------------------------------------------------
  async function loadCurrentTab() {
    errorMsg = '';
    switch (dockerTab) {
      case 'containers': await loadContainers(); break;
      case 'images': await loadImages(); break;
      case 'networks': await loadNetworks(); break;
      case 'compose': await loadComposeProjects(); break;
    }
  }

  async function loadAllStats() {
    try {
      // Quick parallel loads for stats
      const [cOut, iOut, nOut] = await Promise.all([
        execDocker("docker ps -a --format '{{json .}}'").catch(() => ''),
        execDocker("docker images --format '{{json .}}'").catch(() => ''),
        execDocker("docker network ls --format '{{json .}}'").catch(() => '')
      ]);

      const cLines = cOut.trim().split('\n').filter(l => l.trim());
      const cParsed = cLines.map(l => { try { return JSON.parse(l); } catch { return null; } }).filter(Boolean);
      containers = cParsed;
      runningCount = cParsed.filter((c: any) => c.State === 'running').length;
      stoppedCount = cParsed.filter((c: any) => c.State !== 'running').length;

      const iLines = iOut.trim().split('\n').filter(l => l.trim());
      totalImages = iLines.map(l => { try { return JSON.parse(l); } catch { return null; } }).filter(Boolean).length;

      const nLines = nOut.trim().split('\n').filter(l => l.trim());
      totalNetworks = nLines.map(l => { try { return JSON.parse(l); } catch { return null; } }).filter(Boolean).length;
    } catch { /* silent */ }
  }

  onMount(async () => {
    await checkDockerStatus();
    if (dockerInstalled && dockerVersion) {
      await loadAllStats();
    }
  });

  onDestroy(() => {
    if (logsUnlisten) {
      logsUnlisten();
      invoke('stop_container_logs').catch(() => {});
    }
  });
</script>

<div class="docker-manager fade-in">
  <header class="dm-header">
    <div class="title-area">
      <h1>Docker</h1>
      <p class="subtitle">Zarządzaj kontenerami, obrazami i sieciami na serwerze</p>
    </div>
    {#if errorMsg}
      <div class="error-badge">
        <AlertCircle size={14} />
        {errorMsg}
        <button class="dismiss-btn" onclick={() => errorMsg = ''}>
          <X size={12} />
        </button>
      </div>
    {/if}
    {#if successMsg}
      <div class="success-badge">{successMsg}</div>
    {/if}
  </header>

  {#if !dockerInstalled}
    <!-- Docker not installed -->
    <div class="not-installed-card glass">
      <div class="not-installed-icon">
        <Container size={48} />
      </div>
      <h2>Docker nie jest zainstalowany</h2>
      <p class="not-installed-desc">
        Na tym serwerze nie znaleziono Docker Engine. Zainstaluj Docker, aby korzystać z konteneryzacji.
      </p>
      <a href="https://docs.docker.com/engine/install/" target="_blank" rel="noopener" class="install-link">
        Instrukcja instalacji Docker →
      </a>
    </div>
  {:else}
    <!-- Docker status bar -->
    <div class="docker-status-bar glass">
      <div class="status-row">
        <div class="status-item">
          <Container size={18} class="status-icon" />
          <div>
            <span class="status-label">Docker Engine</span>
            <span class="status-value mono-val">{dockerVersion || '—'}</span>
          </div>
        </div>

        <div class="stats-row">
          <div class="stat-chip running">
            <Play size={12} />
            <span class="mono-val">{runningCount}</span>
            <span class="stat-label">aktywne</span>
          </div>
          <div class="stat-chip stopped">
            <Square size={12} />
            <span class="mono-val">{stoppedCount}</span>
            <span class="stat-label">zatrzymane</span>
          </div>
          <div class="stat-chip neutral">
            <Box size={12} />
            <span class="mono-val">{totalImages}</span>
            <span class="stat-label">obrazów</span>
          </div>
          <div class="stat-chip neutral">
            <Network size={12} />
            <span class="mono-val">{totalNetworks}</span>
            <span class="stat-label">sieci</span>
          </div>
        </div>
      </div>
    </div>

    <!-- Sub-tabs -->
    <div class="tabs-bar glass">
      <button class="tab-btn {dockerTab === 'containers' ? 'active' : ''}" onclick={() => { dockerTab = 'containers'; loadContainers(); }}>
        <Container size={16} /> Kontenery
      </button>
      <button class="tab-btn {dockerTab === 'images' ? 'active' : ''}" onclick={() => { dockerTab = 'images'; loadImages(); }}>
        <Box size={16} /> Obrazy
      </button>
      <button class="tab-btn {dockerTab === 'networks' ? 'active' : ''}" onclick={() => { dockerTab = 'networks'; loadNetworks(); }}>
        <Network size={16} /> Sieci
      </button>
      <button class="tab-btn {dockerTab === 'compose' ? 'active' : ''}" onclick={() => { dockerTab = 'compose'; loadComposeProjects(); }}>
        <Layers size={16} /> Compose
      </button>
    </div>

    <!-- Tab content -->
    <div class="tab-content">
      {#if dockerTab === 'containers'}
        <!-- ======== CONTAINERS TAB ======== -->
        <div class="ops-bar glass">
          <div class="search-bar">
            <Search size={16} class="search-icon" />
            <input type="text" placeholder="Szukaj kontenerów..." bind:value={containerSearch} />
          </div>
          <button class="secondary" onclick={loadContainers} disabled={isLoading}>
            <RefreshCw size={16} class={isLoading ? 'spin' : ''} /> Odśwież
          </button>
        </div>

        <div class="table-container glass">
          {#if isLoading && containers.length === 0}
            <div class="loading-state">
              <RefreshCw class="spin" size={32} />
              <p>Wczytywanie kontenerów...</p>
            </div>
          {:else}
            <table class="data-table">
              <thead>
                <tr>
                  <th style="width: 20%;">Nazwa</th>
                  <th style="width: 20%;">Obraz</th>
                  <th style="width: 12%;">Status</th>
                  <th style="width: 15%;">Porty</th>
                  <th style="width: 13%;">Utworzono</th>
                  <th style="width: 20%; text-align: right;">Operacje</th>
                </tr>
              </thead>
              <tbody>
                {#each getFilteredContainers() as container}
                  <tr>
                    <td class="mono-val"><strong>{container.Names}</strong></td>
                    <td class="image-cell" title={container.Image}>{container.Image}</td>
                    <td>
                      <span class="badge {getStatusBadge(container.State)}">
                        {container.State}
                      </span>
                    </td>
                    <td class="ports-cell mono-val">{container.Ports || '—'}</td>
                    <td class="time-cell">{container.CreatedAt?.split(' ')[0] || container.RunningFor || '—'}</td>
                    <td class="actions-cell">
                      {#if container.State === 'running'}
                        <button class="btn-action danger-text" onclick={() => containerAction('stop', container.ID)} title="Zatrzymaj">
                          <Square size={14} />
                        </button>
                        <button class="btn-action" onclick={() => containerAction('restart', container.ID)} title="Restartuj">
                          <RotateCw size={14} />
                        </button>
                      {:else}
                        <button class="btn-action success-text" onclick={() => containerAction('start', container.ID)} title="Uruchom">
                          <Play size={14} />
                        </button>
                      {/if}
                      <button class="btn-action" onclick={() => inspectContainer(container.ID)} title="Inspekcja">
                        <Eye size={14} />
                      </button>
                      <button class="btn-action" onclick={() => openLogs(container.ID, container.Names)} title="Logi">
                        <FileText size={14} />
                      </button>
                      <button class="btn-action" onclick={() => openExec(container.ID, container.Names)} title="Wykonaj komendę">
                        <Terminal size={14} />
                      </button>
                      {#if container.State === 'running'}
                        <button class="btn-action amber-text" onclick={() => openInteractiveShell(container.ID)} title="Interaktywny shell">
                          <ChevronsUpDown size={14} />
                        </button>
                      {/if}
                      <button class="btn-action danger-text" onclick={() => removeContainer(container.ID, container.Names)} title="Usuń">
                        <Trash2 size={14} />
                      </button>
                    </td>
                  </tr>

                  <!-- Expanded inspect -->
                  {#if expandedContainer === container.ID}
                    <tr class="inspect-row">
                      <td colspan="6">
                        <div class="inspect-card">
                          <div class="inspect-header">
                            <span class="inspect-title">Inspekcja: {container.Names}</span>
                            <button class="btn-action" onclick={() => expandedContainer = ''}>
                              <X size={14} />
                            </button>
                          </div>
                          {#if inspectLoading}
                            <div class="inspect-loading"><Loader2 size={20} class="spin" /> Ładowanie...</div>
                          {:else}
                            <pre class="inspect-json">{containerInspectData}</pre>
                          {/if}
                        </div>
                      </td>
                    </tr>
                  {/if}
                {/each}

                {#if getFilteredContainers().length === 0 && !isLoading}
                  <tr>
                    <td colspan="6" class="empty-state">
                      {#if containers.length === 0}
                        Brak kontenerów. Utwórz nowy kontener za pomocą <code>docker run</code> lub Docker Compose.
                      {:else}
                        Brak wyników dla frazy „{containerSearch}"
                      {/if}
                    </td>
                  </tr>
                {/if}
              </tbody>
            </table>
          {/if}
        </div>

      {:else if dockerTab === 'images'}
        <!-- ======== IMAGES TAB ======== -->
        <div class="ops-bar glass">
          <div class="search-bar">
            <Search size={16} class="search-icon" />
            <input type="text" placeholder="Szukaj obrazów..." bind:value={imageSearch} />
          </div>
          <button class="secondary" onclick={loadImages} disabled={isLoading}>
            <RefreshCw size={16} class={isLoading ? 'spin' : ''} /> Odśwież
          </button>
          <button class="secondary" onclick={pruneImages}>
            <Eraser size={16} /> Wyczyść nieużywane
          </button>
          <button class="primary" onclick={() => { showPullModal = true; pullImageName = ''; pullProgress = ''; }}>
            <Download size={16} /> Pobierz obraz
          </button>
        </div>

        <div class="table-container glass">
          {#if isLoading && images.length === 0}
            <div class="loading-state">
              <RefreshCw class="spin" size={32} />
              <p>Wczytywanie obrazów...</p>
            </div>
          {:else}
            <table class="data-table">
              <thead>
                <tr>
                  <th style="width: 30%;">Repozytorium</th>
                  <th style="width: 15%;">Tag</th>
                  <th style="width: 18%;">ID Obrazu</th>
                  <th style="width: 12%;">Rozmiar</th>
                  <th style="width: 12%;">Utworzono</th>
                  <th style="width: 13%; text-align: right;">Operacje</th>
                </tr>
              </thead>
              <tbody>
                {#each getFilteredImages() as image}
                  <tr>
                    <td class="mono-val"><strong>{image.Repository}</strong></td>
                    <td><span class="badge warning">{image.Tag}</span></td>
                    <td class="mono-val id-cell">{image.ID}</td>
                    <td class="mono-val">{image.Size}</td>
                    <td class="time-cell">{image.CreatedSince || image.CreatedAt || '—'}</td>
                    <td class="actions-cell">
                      <button class="btn-action danger-text" onclick={() => removeImage(image.ID, image.Repository + ':' + image.Tag)} title="Usuń obraz">
                        <Trash2 size={14} />
                      </button>
                    </td>
                  </tr>
                {/each}

                {#if getFilteredImages().length === 0 && !isLoading}
                  <tr>
                    <td colspan="6" class="empty-state">
                      {#if images.length === 0}
                        Brak obrazów. Pobierz nowy obraz przyciskiem powyżej.
                      {:else}
                        Brak wyników dla frazy „{imageSearch}"
                      {/if}
                    </td>
                  </tr>
                {/if}
              </tbody>
            </table>
          {/if}
        </div>

      {:else if dockerTab === 'networks'}
        <!-- ======== NETWORKS TAB ======== -->
        <div class="ops-bar glass">
          <button class="secondary" onclick={loadNetworks} disabled={isLoading}>
            <RefreshCw size={16} class={isLoading ? 'spin' : ''} /> Odśwież
          </button>
          <button class="primary" onclick={() => { showCreateNetworkModal = true; newNetworkName = ''; newNetworkDriver = 'bridge'; }}>
            <Plus size={16} /> Utwórz sieć
          </button>
        </div>

        <div class="table-container glass">
          {#if isLoading && networks.length === 0}
            <div class="loading-state">
              <RefreshCw class="spin" size={32} />
              <p>Wczytywanie sieci...</p>
            </div>
          {:else}
            <table class="data-table">
              <thead>
                <tr>
                  <th style="width: 28%;">Nazwa</th>
                  <th style="width: 18%;">Driver</th>
                  <th style="width: 15%;">Zakres</th>
                  <th style="width: 22%;">ID Sieci</th>
                  <th style="width: 17%; text-align: right;">Operacje</th>
                </tr>
              </thead>
              <tbody>
                {#each networks as network}
                  <tr>
                    <td class="mono-val"><strong>{network.Name}</strong></td>
                    <td><span class="badge warning">{network.Driver}</span></td>
                    <td>{network.Scope || '—'}</td>
                    <td class="mono-val id-cell">{network.ID?.substring(0, 12) || '—'}</td>
                    <td class="actions-cell">
                      <button class="btn-action" onclick={() => inspectNetwork(network.ID || network.Name)} title="Inspekcja">
                        <Eye size={14} />
                      </button>
                      {#if network.Name !== 'bridge' && network.Name !== 'host' && network.Name !== 'none'}
                        <button class="btn-action danger-text" onclick={() => removeNetwork(network.ID || network.Name, network.Name)} title="Usuń sieć">
                          <Trash2 size={14} />
                        </button>
                      {/if}
                    </td>
                  </tr>

                  <!-- Expanded inspect -->
                  {#if expandedNetwork === (network.ID || network.Name)}
                    <tr class="inspect-row">
                      <td colspan="5">
                        <div class="inspect-card">
                          <div class="inspect-header">
                            <span class="inspect-title">Inspekcja sieci: {network.Name}</span>
                            <button class="btn-action" onclick={() => expandedNetwork = ''}>
                              <X size={14} />
                            </button>
                          </div>
                          {#if networkInspectLoading}
                            <div class="inspect-loading"><Loader2 size={20} class="spin" /> Ładowanie...</div>
                          {:else}
                            <pre class="inspect-json">{networkInspectData}</pre>
                          {/if}
                        </div>
                      </td>
                    </tr>
                  {/if}
                {/each}

                {#if networks.length === 0 && !isLoading}
                  <tr>
                    <td colspan="5" class="empty-state">Brak sieci Docker</td>
                  </tr>
                {/if}
              </tbody>
            </table>
          {/if}
        </div>

      {:else if dockerTab === 'compose'}
        <!-- ======== COMPOSE TAB ======== -->
        <div class="ops-bar glass">
          <button class="secondary" onclick={loadComposeProjects} disabled={isLoading}>
            <RefreshCw size={16} class={isLoading ? 'spin' : ''} /> Odśwież
          </button>
          <button class="primary" onclick={openDirPicker}>
            <FolderPlus size={16} /> Nowy Projekt
          </button>
        </div>

        <div class="table-container glass">
          {#if isLoading && composeProjects.length === 0}
            <div class="loading-state">
              <RefreshCw class="spin" size={32} />
              <p>Wczytywanie projektów Compose...</p>
            </div>
          {:else}
            <table class="data-table">
              <thead>
                <tr>
                  <th style="width: 22%;">Nazwa Projektu</th>
                  <th style="width: 13%;">Status</th>
                  <th style="width: 30%;">Plik Konfiguracyjny</th>
                  <th style="width: 35%; text-align: right;">Operacje</th>
                </tr>
              </thead>
              <tbody>
                {#each composeProjects as project}
                  <tr>
                    <td class="mono-val"><strong>{project.Name || project.name || '—'}</strong></td>
                    <td>
                      <span class="badge {(project.Status || project.status || '').toLowerCase().includes('running') ? 'success' : 'danger'}">
                        {project.Status || project.status || '—'}
                      </span>
                    </td>
                    <td class="mono-val config-cell" title={project.ConfigFiles || project['config files'] || ''}>{project.ConfigFiles || project['config files'] || '—'}</td>
                    <td class="actions-cell compose-actions">
                      <button class="secondary btn-sm" onclick={() => composeAction('up -d', project.ConfigFiles || project['config files'] || '')}>
                        <Play size={14} /> Up
                      </button>
                      <button class="secondary btn-sm" onclick={() => composeAction('down', project.ConfigFiles || project['config files'] || '')}>
                        <Square size={14} /> Down
                      </button>
                      <button class="secondary btn-sm" onclick={() => composeAction('restart', project.ConfigFiles || project['config files'] || '')}>
                        <RotateCw size={14} /> Restart
                      </button>
                      <button class="btn-action" onclick={() => openComposeEditor(project.ConfigFiles || project['config files'] || '')} title="Edytuj plik compose">
                        <FileText size={14} />
                      </button>
                      <button class="btn-action" onclick={() => openComposeLogs(project.Name || project.name || '', project.ConfigFiles || project['config files'] || '')} title="Logi projektu">
                        <Eye size={14} />
                      </button>
                    </td>
                  </tr>
                {/each}

                {#if composeProjects.length === 0 && !isLoading}
                  <tr>
                    <td colspan="4" class="empty-state">
                      Brak projektów Docker Compose. Utwórz nowy projekt przyciskiem powyżej.
                    </td>
                  </tr>
                {/if}
              </tbody>
            </table>
          {/if}
        </div>
      {/if}
    </div>
  {/if}

  <!-- ====================================================== -->
  <!--                        MODALS                          -->
  <!-- ====================================================== -->

  <!-- Sudo Password Modal -->
  {#if showSudoModal}
    <div class="modal-overlay">
      <div class="modal-content glass">
        <div class="modal-header-icon">
          <KeyRound size={32} class="accent-amber-text" />
        </div>
        <h3>Wymagane uwierzytelnienie Sudo</h3>
        <p class="modal-desc">Ta operacja wymaga uprawnień roota. Wprowadź swoje hasło użytkownika (sudo):</p>
        <input
          type="password"
          placeholder="Wpisz hasło..."
          bind:value={sudoPassword}
          onkeydown={(e) => e.key === 'Enter' && submitSudoPassword()}
        />
        {#if sudoError}
          <span class="error-text">{sudoError}</span>
        {/if}
        <div class="modal-actions">
          <button class="primary" onclick={submitSudoPassword}>Zatwierdź</button>
          <button class="secondary" onclick={() => { showSudoModal = false; sudoPassword = ''; pendingAction = null; }}>Anuluj</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Confirm Modal -->
  {#if showConfirmModal}
    <div class="modal-overlay">
      <div class="modal-content glass">
        <div class="modal-header-icon">
          <AlertCircle size={32} class="accent-red-text" />
        </div>
        <h3>Potwierdź operację</h3>
        <p class="modal-desc">{confirmMessage}</p>
        <div class="modal-actions">
          <button class="danger" onclick={() => confirmAction && confirmAction()}>Tak, wykonaj</button>
          <button class="secondary" onclick={() => { showConfirmModal = false; confirmAction = null; }}>Anuluj</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Container Logs Modal -->
  {#if showLogsModal}
    <div class="modal-overlay logs-overlay">
      <div class="modal-content glass logs-modal">
        <div class="logs-header">
          <h3>Logi: {logsContainerName}</h3>
          <div class="logs-controls">
            <div class="search-bar search-bar-sm">
              <Search size={14} class="search-icon" />
              <input type="text" placeholder="Filtruj logi..." bind:value={logSearch} />
            </div>
            <button class="secondary btn-sm" onclick={() => logsPaused = !logsPaused}>
              {#if logsPaused}
                <Play size={14} /> Wznów
              {:else}
                <Pause size={14} /> Pauza
              {/if}
            </button>
            <button class="secondary btn-sm" onclick={() => logLines = []}>
              <Eraser size={14} /> Wyczyść
            </button>
            <button class="secondary btn-sm" onclick={downloadLogs}>
              <Download size={14} /> Pobierz
            </button>
            <button class="secondary btn-sm" onclick={closeLogs}>
              <X size={14} />
            </button>
          </div>
        </div>
        <div class="logs-display" bind:this={logScrollEl}>
          <pre class="log-text">{getFilteredLogs().join('\n') || 'Oczekiwanie na logi...'}</pre>
        </div>
      </div>
    </div>
  {/if}

  <!-- Container Exec Modal -->
  {#if showExecModal}
    <div class="modal-overlay">
      <div class="modal-content glass exec-modal">
        <h3>Wykonaj komendę: {execContainerName}</h3>
        <p class="modal-desc">Uruchom polecenie wewnątrz kontenera.</p>
        <div class="exec-input-row">
          <input
            type="text"
            placeholder="np. ls -la /app"
            bind:value={execCommand}
            onkeydown={(e) => e.key === 'Enter' && runExec()}
            class="exec-input"
          />
          <button class="primary" onclick={runExec} disabled={execRunning || !execCommand.trim()}>
            {#if execRunning}
              <Loader2 size={16} class="spin" />
            {:else}
              <Play size={16} />
            {/if}
            Wykonaj
          </button>
        </div>
        {#if execOutput}
          <div class="exec-output-container">
            <pre class="exec-output">{execOutput}</pre>
          </div>
        {/if}
        <div class="modal-actions">
          <button class="secondary" onclick={() => showExecModal = false}>Zamknij</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Pull Image Modal -->
  {#if showPullModal}
    <div class="modal-overlay">
      <div class="modal-content glass">
        <h3>Pobierz obraz Docker</h3>
        <div class="form-group">
          <label for="pull-name">Nazwa obrazu (np. nginx:latest, redis:7-alpine)</label>
          <input
            id="pull-name"
            type="text"
            placeholder="nginx:latest"
            bind:value={pullImageName}
            onkeydown={(e) => e.key === 'Enter' && pullImage()}
          />
        </div>
        {#if pullProgress}
          <div class="pull-progress">
            <pre class="pull-progress-text">{pullProgress}</pre>
          </div>
        {/if}
        <div class="modal-actions">
          <button class="primary" onclick={pullImage} disabled={isPulling || !pullImageName.trim()}>
            {#if isPulling}
              <Loader2 size={16} class="spin" /> Pobieranie...
            {:else}
              <Download size={16} /> Pobierz
            {/if}
          </button>
          <button class="secondary" onclick={() => { showPullModal = false; pullImageName = ''; pullProgress = ''; }}>Anuluj</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Create Network Modal -->
  {#if showCreateNetworkModal}
    <div class="modal-overlay">
      <div class="modal-content glass">
        <h3>Utwórz sieć Docker</h3>
        <div class="form-group">
          <label for="net-name">Nazwa sieci</label>
          <input id="net-name" type="text" placeholder="my-network" bind:value={newNetworkName} />
        </div>
        <div class="form-group">
          <label for="net-driver">Driver sieci</label>
          <select id="net-driver" bind:value={newNetworkDriver}>
            <option value="bridge">Bridge (domyślny)</option>
            <option value="overlay">Overlay</option>
            <option value="host">Host</option>
          </select>
        </div>
        <div class="modal-actions">
          <button class="primary" onclick={createNetwork} disabled={!newNetworkName.trim()}>Utwórz</button>
          <button class="secondary" onclick={() => showCreateNetworkModal = false}>Anuluj</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Directory Picker Modal (Compose) -->
  {#if showDirPicker}
    <div class="modal-overlay">
      <div class="modal-content glass dir-picker-modal">
        <h3>Nowy projekt Compose</h3>
        <p class="modal-desc">Wybierz katalog, w którym utworzyć projekt, i podaj nazwę folderu.</p>

        <!-- Current path -->
        <div class="dir-picker-path glass">
          <FolderOpen size={16} class="accent-amber-text" />
          <span class="mono-val">{dirPickerPath}</span>
        </div>

        <!-- Directory listing -->
        <div class="dir-picker-list">
          <button class="dir-entry" onclick={navigateUp}>
            <ChevronLeft size={14} />
            <span>..</span>
          </button>
          {#if dirPickerLoading}
            <div class="dir-picker-loading"><Loader2 size={20} class="spin" /></div>
          {:else}
            {#each dirPickerEntries as entry}
              <button class="dir-entry" onclick={() => navigateDir(entry.name || entry.filename)}>
                <FolderOpen size={14} />
                <span>{entry.name || entry.filename}</span>
                <ChevronRight size={14} class="dir-chevron" />
              </button>
            {/each}
            {#if dirPickerEntries.length === 0}
              <div class="dir-picker-empty">Brak podkatalogów w tym folderze</div>
            {/if}
          {/if}
        </div>

        <!-- New folder name -->
        <div class="form-group">
          <label for="new-folder">Nazwa nowego folderu projektu</label>
          <input
            id="new-folder"
            type="text"
            placeholder="my-app"
            bind:value={newProjectFolder}
          />
        </div>

        <div class="modal-actions">
          <button class="primary" onclick={createComposeProject} disabled={!newProjectFolder.trim() || isLoading}>
            {#if isLoading}
              <Loader2 size={16} class="spin" />
            {:else}
              <FolderPlus size={16} />
            {/if}
            Utwórz projekt
          </button>
          <button class="secondary" onclick={() => showDirPicker = false}>Anuluj</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Compose Editor Modal -->
  {#if showComposeEditor}
    <div class="modal-overlay">
      <div class="modal-content glass compose-editor-modal">
        <div class="compose-editor-header">
          <h3>Edytor Compose</h3>
          <span class="mono-val compose-filepath">{composeEditorPath}</span>
        </div>
        <textarea
          class="compose-textarea"
          bind:value={composeEditorContent}
          spellcheck="false"
        ></textarea>
        <div class="modal-actions">
          <button class="primary" onclick={saveComposeFile} disabled={composeEditorSaving}>
            {#if composeEditorSaving}
              <Loader2 size={16} class="spin" /> Zapisywanie...
            {:else}
              Zapisz
            {/if}
          </button>
          <button class="secondary" onclick={() => showComposeEditor = false}>Zamknij</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Compose Logs Modal -->
  {#if showComposeLogsModal}
    <div class="modal-overlay logs-overlay">
      <div class="modal-content glass logs-modal">
        <div class="logs-header">
          <h3>Logi Compose: {composeLogsProjectName}</h3>
          <button class="secondary btn-sm" onclick={() => showComposeLogsModal = false}>
            <X size={14} /> Zamknij
          </button>
        </div>
        <div class="logs-display">
          <pre class="log-text">{composeLogs.join('\n') || 'Brak logów'}</pre>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .docker-manager {
    padding: 30px;
    display: flex;
    flex-direction: column;
    gap: 20px;
    height: 100%;
    overflow: hidden;
  }

  .dm-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-shrink: 0;
    gap: 16px;
    flex-wrap: wrap;
  }

  .title-area h1 {
    font-size: 2rem;
    color: white;
  }

  .subtitle {
    color: var(--text-secondary);
    font-size: 0.9rem;
    margin-top: 4px;
  }

  .error-badge {
    background: var(--accent-red-glow);
    border: 1px solid rgba(244, 63, 94, 0.3);
    padding: 8px 16px;
    border-radius: var(--radius-sm);
    color: #ff8595;
    font-size: 0.82rem;
    display: flex;
    align-items: center;
    gap: 8px;
    max-width: 500px;
  }

  .dismiss-btn {
    background: transparent;
    border: none;
    color: inherit;
    padding: 2px;
    cursor: pointer;
    border-radius: 2px;
    flex-shrink: 0;
  }

  .dismiss-btn:hover {
    background: rgba(255, 255, 255, 0.1);
  }

  .success-badge {
    background: var(--accent-green-glow);
    border: 1px solid rgba(16, 185, 129, 0.3);
    padding: 8px 16px;
    border-radius: var(--radius-sm);
    color: var(--accent-green);
    font-size: 0.82rem;
  }

  /* Not installed card */
  .not-installed-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 60px 40px;
    border-radius: var(--radius-md);
    text-align: center;
    gap: 16px;
    flex: 1;
  }

  .not-installed-icon {
    color: var(--text-muted);
    opacity: 0.5;
  }

  .not-installed-card h2 {
    color: white;
    font-size: 1.3rem;
  }

  .not-installed-desc {
    color: var(--text-secondary);
    font-size: 0.9rem;
    max-width: 400px;
    line-height: 1.5;
  }

  .install-link {
    color: var(--accent-amber);
    text-decoration: none;
    font-weight: 500;
    font-size: 0.9rem;
    transition: var(--transition-fast);
  }

  .install-link:hover {
    text-decoration: underline;
    filter: brightness(1.2);
  }

  /* Docker status bar */
  .docker-status-bar {
    padding: 16px 20px;
    border-radius: var(--radius-md);
    flex-shrink: 0;
  }

  .status-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .status-item {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .status-icon {
    color: var(--accent-amber);
  }

  .status-label {
    display: block;
    font-size: 0.75rem;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    font-weight: 600;
  }

  .status-value {
    display: block;
    font-size: 1rem;
    color: white;
    font-weight: 600;
  }

  .stats-row {
    display: flex;
    gap: 12px;
  }

  .stat-chip {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border-radius: var(--radius-sm);
    font-size: 0.8rem;
    border: 1px solid var(--border-color);
    background: rgba(255, 255, 255, 0.02);
  }

  .stat-chip .mono-val {
    font-weight: 700;
    font-size: 0.9rem;
  }

  .stat-label {
    color: var(--text-muted);
    font-size: 0.72rem;
  }

  .stat-chip.running {
    color: var(--accent-green);
    border-color: rgba(16, 185, 129, 0.2);
    background: rgba(16, 185, 129, 0.05);
  }

  .stat-chip.stopped {
    color: var(--text-secondary);
    border-color: var(--border-color);
  }

  .stat-chip.neutral {
    color: var(--text-secondary);
  }

  /* Tabs bar */
  .tabs-bar {
    display: flex;
    padding: 6px;
    border-radius: var(--radius-md);
    gap: 6px;
    flex-shrink: 0;
  }

  .tab-btn {
    flex: 1;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    padding: 10px;
    cursor: pointer;
    font-size: 0.9rem;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    transition: var(--transition-fast);
  }

  .tab-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .tab-btn.active {
    background: var(--bg-active);
    color: var(--accent-amber);
    border: 1px solid rgba(245, 158, 11, 0.2);
    font-weight: 600;
  }

  /* Tab content area */
  .tab-content {
    display: flex;
    flex-direction: column;
    gap: 16px;
    flex: 1;
    overflow: hidden;
    min-height: 0;
  }

  /* Ops bar */
  .ops-bar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    border-radius: var(--radius-md);
    flex-shrink: 0;
  }

  .search-bar {
    position: relative;
    flex: 1;
  }

  .search-bar input {
    width: 100%;
    padding-left: 36px;
  }

  .search-icon {
    position: absolute;
    left: 12px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--text-muted);
  }

  /* Table */
  .table-container {
    flex: 1;
    overflow-y: auto;
    border-radius: var(--radius-md);
    min-height: 0;
  }

  .data-table {
    width: 100%;
    border-collapse: collapse;
    text-align: left;
  }

  .data-table th,
  .data-table td {
    padding: 12px 16px;
    border-bottom: 1px solid var(--border-color);
  }

  .data-table th {
    font-size: 0.75rem;
    text-transform: uppercase;
    color: var(--text-muted);
    font-weight: 600;
    letter-spacing: 0.05em;
    position: sticky;
    top: 0;
    background: var(--bg-secondary);
    z-index: 1;
  }

  .data-table tr {
    transition: var(--transition-fast);
  }

  .data-table tbody tr:hover {
    background: rgba(255, 255, 255, 0.02);
  }

  .image-cell, .config-cell {
    color: var(--text-secondary);
    font-size: 0.85rem;
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ports-cell {
    font-size: 0.78rem;
    color: var(--text-secondary);
    max-width: 140px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .id-cell {
    font-size: 0.78rem;
    color: var(--text-muted);
  }

  .time-cell {
    font-size: 0.82rem;
    color: var(--text-secondary);
  }

  .actions-cell {
    text-align: right;
    display: flex;
    justify-content: flex-end;
    gap: 4px;
    flex-wrap: nowrap;
  }

  .compose-actions {
    gap: 6px;
  }

  .btn-action {
    background: transparent;
    border: none;
    padding: 6px;
    border-radius: 4px;
    color: var(--text-secondary);
    cursor: pointer;
    transition: var(--transition-fast);
    flex-shrink: 0;
  }

  .btn-action:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .success-text:hover {
    color: var(--accent-green) !important;
    background: var(--accent-green-glow) !important;
  }

  .danger-text:hover {
    color: var(--accent-red) !important;
    background: var(--accent-red-glow) !important;
  }

  .amber-text:hover {
    color: var(--accent-amber) !important;
    background: var(--accent-amber-glow) !important;
  }

  .btn-sm {
    padding: 6px 12px;
    font-size: 0.8rem;
  }

  .empty-state {
    text-align: center;
    color: var(--text-muted);
    font-size: 0.9rem;
    padding: 40px !important;
  }

  .empty-state code {
    color: var(--accent-amber);
    font-family: var(--font-mono);
    font-size: 0.82rem;
  }

  .loading-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 200px;
    gap: 16px;
    color: var(--text-secondary);
  }

  .spin {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  /* Inspect card */
  .inspect-row {
    background: rgba(0, 0, 0, 0.15) !important;
  }

  .inspect-row:hover {
    background: rgba(0, 0, 0, 0.15) !important;
  }

  .inspect-card {
    padding: 12px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-color);
    background: var(--bg-primary);
  }

  .inspect-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 10px;
  }

  .inspect-title {
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--accent-amber);
  }

  .inspect-loading {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-secondary);
    font-size: 0.85rem;
    padding: 20px 0;
    justify-content: center;
  }

  .inspect-json {
    font-family: var(--font-mono);
    font-size: 0.72rem;
    line-height: 1.5;
    color: var(--text-secondary);
    white-space: pre-wrap;
    word-break: break-all;
    max-height: 300px;
    overflow: auto;
    user-select: text;
  }

  /* Modals */
  .modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .modal-content {
    width: 440px;
    padding: 24px;
    border-radius: var(--radius-md);
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .modal-header-icon {
    display: flex;
    justify-content: center;
  }

  .modal-desc {
    font-size: 0.88rem;
    color: var(--text-secondary);
    line-height: 1.4;
  }

  .error-text {
    color: var(--accent-red);
    font-size: 0.8rem;
    margin-top: -8px;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 4px;
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .form-group label {
    font-size: 0.8rem;
    color: var(--text-secondary);
    font-weight: 500;
  }

  /* Logs modal */
  .logs-overlay {
    z-index: 110;
  }

  .logs-modal {
    width: 85vw;
    max-width: 1100px;
    height: 75vh;
    max-height: 700px;
  }

  .logs-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-shrink: 0;
    gap: 12px;
    flex-wrap: wrap;
  }

  .logs-header h3 {
    font-size: 1rem;
    white-space: nowrap;
  }

  .logs-controls {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .search-bar-sm {
    min-width: 180px;
    max-width: 220px;
  }

  .search-bar-sm input {
    padding: 6px 10px 6px 32px;
    font-size: 0.8rem;
  }

  .search-bar-sm .search-icon {
    left: 10px;
  }

  .logs-display {
    flex: 1;
    overflow: auto;
    background: var(--bg-primary);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    padding: 12px;
    min-height: 0;
  }

  .log-text {
    font-family: var(--font-mono);
    font-size: 0.75rem;
    line-height: 1.55;
    color: #d1d4db;
    white-space: pre-wrap;
    word-break: break-all;
    user-select: text;
  }

  /* Exec modal */
  .exec-modal {
    width: 600px;
  }

  .exec-input-row {
    display: flex;
    gap: 10px;
  }

  .exec-input {
    flex: 1;
    font-family: var(--font-mono);
    font-size: 0.85rem;
  }

  .exec-output-container {
    background: var(--bg-primary);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    padding: 12px;
    max-height: 300px;
    overflow: auto;
  }

  .exec-output {
    font-family: var(--font-mono);
    font-size: 0.78rem;
    line-height: 1.5;
    color: var(--text-primary);
    white-space: pre-wrap;
    word-break: break-all;
    user-select: text;
  }

  /* Pull progress */
  .pull-progress {
    background: var(--bg-primary);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    padding: 12px;
    max-height: 200px;
    overflow: auto;
  }

  .pull-progress-text {
    font-family: var(--font-mono);
    font-size: 0.75rem;
    line-height: 1.5;
    color: var(--text-secondary);
    white-space: pre-wrap;
  }

  /* Directory picker modal */
  .dir-picker-modal {
    width: 520px;
    max-height: 80vh;
  }

  .dir-picker-path {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    border-radius: var(--radius-sm);
    font-size: 0.82rem;
    overflow: hidden;
  }

  .dir-picker-path .mono-val {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dir-picker-list {
    max-height: 250px;
    overflow-y: auto;
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    background: var(--bg-primary);
  }

  .dir-entry {
    width: 100%;
    background: transparent;
    border: none;
    border-bottom: 1px solid var(--border-color);
    padding: 10px 14px;
    color: var(--text-primary);
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 0.85rem;
    text-align: left;
    transition: var(--transition-fast);
  }

  .dir-entry:last-child {
    border-bottom: none;
  }

  .dir-entry:hover {
    background: var(--bg-hover);
    color: var(--accent-amber);
  }

  .dir-chevron {
    margin-left: auto;
    color: var(--text-muted);
  }

  .dir-picker-loading {
    display: flex;
    justify-content: center;
    padding: 20px;
    color: var(--text-secondary);
  }

  .dir-picker-empty {
    text-align: center;
    padding: 20px;
    color: var(--text-muted);
    font-size: 0.85rem;
  }

  /* Compose editor modal */
  .compose-editor-modal {
    width: 700px;
    max-height: 85vh;
  }

  .compose-editor-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
  }

  .compose-filepath {
    font-size: 0.72rem;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 300px;
  }

  .compose-textarea {
    width: 100%;
    min-height: 350px;
    max-height: 500px;
    font-family: var(--font-mono);
    font-size: 0.82rem;
    line-height: 1.6;
    resize: vertical;
    background: var(--bg-primary);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    padding: 16px;
    tab-size: 2;
  }

  .compose-textarea:focus {
    border-color: var(--accent-amber);
    box-shadow: 0 0 0 2px var(--accent-amber-glow);
  }

  select {
    width: 100%;
  }
</style>
