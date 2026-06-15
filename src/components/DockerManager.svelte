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
    Unplug, ChevronsUpDown, Database, Edit, Save
  } from 'lucide-svelte';
  import yaml from 'js-yaml';
  import toml from 'toml';
  import { stickToBottom } from '$lib/stickToBottom';

  // Props
  let { onRequestTerminalExec = (_ctx: { containerId: string; containerName: string; useSudo: boolean; shell: string }) => {} } = $props();

  // Sub-tab state
  let dockerTab = $state<'containers' | 'images' | 'networks' | 'compose' | 'volumes'>('containers');

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
  let containersLoading = $state(true);
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

  // Container shell picker
  let showShellModal = $state(false);
  let shellPickContainerId = $state('');
  let shellPickContainerName = $state('');
  let selectedShell = $state('/bin/bash');
  const shellOptions = [
    { value: '/bin/bash', label: 'bash' },
    { value: '/bin/sh', label: 'sh' },
    { value: '/bin/ash', label: 'ash (Alpine)' },
    { value: '/bin/zsh', label: 'zsh' },
  ];
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

  // Compose Pull modal states
  let showComposePullModal = $state(false);
  let composePullProjectName = $state('');
  let composePullConfigFile = $state('');
  let composePullLogs = $state<string[]>([]);
  let composePullProgress = $state(0);
  let composePullStatus = $state<'idle' | 'pulling' | 'finished' | 'error'>('idle');
  let composePullLayers = $state<Record<string, { service: string, status: string, percent: number }>>({});
  let composePullUnlisten: (() => void) | null = null;

  // Monaco Editor in Compose
  let composeEditorElement: HTMLDivElement | null = $state(null);
  let composeEditorInstance: any = null;

  // Bulk selection lists (selected IDs or Names)
  let selectedContainers = $state<string[]>([]);
  let selectedImages = $state<string[]>([]);
  let selectedNetworks = $state<string[]>([]);
  let selectedCompose = $state<string[]>([]);

  // Unused Images helper state
  let usedImageIds = $state<string[]>([]);

  // Volumes state
  let volumes = $state<any[]>([]);
  let volumeSearch = $state('');
  let expandedVolume = $state('');
  let volumeInspectData = $state('');
  let volumeInspectLoading = $state(false);

  // Volume browser state
  let showVolumeBrowser = $state(false);
  let browserVolumeName = $state('');
  let browserVolumePath = $state('');
  let browserRelativePath = $state('');
  let browserEntries = $state<any[]>([]);
  let browserLoading = $state(false);
  let browserEditingFile = $state<string | null>(null);
  let browserEditingContent = $state('');
  let browserEditorElement: HTMLDivElement | null = $state(null);
  let browserEditorInstance: any = null;
  let browserEditorSaving = $state(false);
  let browserErrorMsg = $state('');

  // Modify Container modal state
  let showModifyModal = $state(false);
  let modifyContainerId = $state('');
  let modifyName = $state('');
  let modifyImage = $state('');
  let modifyPorts = $state<{ host: string, container: string, proto: 'tcp' | 'udp' }[]>([]);
  let modifyVolumes = $state<{ host: string, container: string, ro: boolean }[]>([]);
  let modifyEnv = $state<{ key: string, value: string }[]>([]);
  let modifyNetworks = $state<string[]>([]);
  let modifyRestartPolicy = $state('unless-stopped');
  let modifyCmd = $state('');
  let modifyEntrypoint = $state('');
  let modifyLoading = $state(false);

  // Compose Stack network modify state
  let showComposeNetworkModal = $state(false);
  let composeNetworkProject = $state<any>(null);
  let composeSelectedNetwork = $state('');
  let composeNetworkLoading = $state(false);

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
    containersLoading = true;
    errorMsg = '';
    try {
      const output = await execDocker("docker ps -a --format '{{json .}}'");
      const lines = output.trim().split('\n').filter(l => l.trim());
      const parsed = lines.map(line => {
        try { return JSON.parse(line); }
        catch { return null; }
      }).filter(Boolean);
      containers = parsed;
      usedImageIds = parsed.map((c: any) => c.ImageID || c.Image).filter(Boolean);

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
      containersLoading = false;
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
        }
      });
      await invoke('start_container_logs', { containerId: id, tail: 200, useSudo });
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

  function openInteractiveShell(containerId: string, containerName: string) {
    shellPickContainerId = containerId;
    shellPickContainerName = containerName;
    selectedShell = '/bin/bash';
    showShellModal = true;
  }

  async function confirmShellLaunch() {
    const containerId = shellPickContainerId;
    const containerName = shellPickContainerName;
    const shell = selectedShell;
    showShellModal = false;

    const launch = () => onRequestTerminalExec({ containerId, containerName, useSudo, shell });

    if (useSudo) {
      try {
        const hasPass: boolean = await invoke('has_sudo_password');
        if (!hasPass) {
          pendingAction = async () => launch();
          showSudoModal = true;
          return;
        }
      } catch {
        pendingAction = async () => launch();
        showSudoModal = true;
        return;
      }
    }
    launch();
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
      try {
        const cOut = await execDocker("docker ps -a --format '{{.ImageID}}'").catch(() => '');
        usedImageIds = cOut.trim().split('\n').filter(Boolean);
      } catch { /* ignore */ }
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

      setTimeout(() => {
        if (composeEditorElement) {
          (window as any).MonacoEnvironment = {
            getWorkerUrl: function () {
              return `data:text/javascript;charset=utf-8,${encodeURIComponent(`
                self.MonacoEnvironment = {
                  baseUrl: 'https://cdnjs.cloudflare.com/ajax/libs/monaco-editor/0.43.0/min/'
                };
                importScripts('https://cdnjs.cloudflare.com/ajax/libs/monaco-editor/0.43.0/min/vs/base/worker/workerMain.js');
              `)}`;
            }
          };

          import('monaco-editor').then((monaco) => {
            if (composeEditorInstance) {
              composeEditorInstance.dispose();
            }
            const ext = filePath.split('.').pop()?.toLowerCase();
            const lang = (ext === 'toml') ? 'toml' : 'yaml';
            
            composeEditorInstance = monaco.editor.create(composeEditorElement!, {
              value: content,
              language: lang,
              theme: 'vs-dark',
              automaticLayout: true,
              fontSize: 14,
              fontFamily: '"JetBrains Mono", Consolas, monospace',
              minimap: { enabled: false },
            });

            composeEditorInstance.onDidChangeModelContent(() => {
              validateComposeContent(monaco, composeEditorInstance.getValue());
            });

            validateComposeContent(monaco, content);
          });
        }
      }, 100);
    } catch (err: any) {
      composeEditorContent = '# Błąd odczytu pliku: ' + err.toString();
    }
  }

  async function saveComposeFile() {
    if (!composeEditorInstance) return;
    composeEditorSaving = true;
    const content = composeEditorInstance.getValue();
    try {
      await invoke('sftp_write', { path: composeEditorPath, content });
      successMsg = 'Plik zapisany';
      setTimeout(() => successMsg = '', 3000);
      closeComposeEditor();
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
      composeLogs = output.trim() ? output.split('\n') : ['Brak logów'];
    } catch (err: any) {
      if (err.toString().includes('SUDO_PASSWORD_REQUIRED')) {
        pendingAction = () => openComposeLogs(name, configFiles);
        showSudoModal = true;
      } else {
        composeLogs = ['Błąd pobierania logów: ' + err.toString()];
      }
    }
  }

  function closeComposeEditor() {
    showComposeEditor = false;
    if (composeEditorInstance) {
      composeEditorInstance.dispose();
      composeEditorInstance = null;
    }
  }

  // ----------------------------------------------------------
  // Compose Pull
  // ----------------------------------------------------------
  async function openComposePull(name: string, configFiles: string) {
    const filePath = configFiles.split(',')[0].trim();
    composePullProjectName = name;
    composePullConfigFile = filePath;
    composePullLogs = [];
    composePullProgress = 0;
    composePullStatus = 'pulling';
    composePullLayers = {};
    showComposePullModal = true;

    try {
      composePullUnlisten = await listen('compose-pull-data', (event: any) => {
        const data = event.payload as string;
        composePullLogs = [...composePullLogs, data];
        parseComposePullProgress(data);
        if (data.includes('[Błąd pobierania') || data.includes('Error response from daemon')) {
          composePullStatus = 'error';
        }
      });
      await invoke('start_compose_pull', { configFile: filePath, useSudo });
    } catch (err: any) {
      if (err.toString().includes('SUDO_PASSWORD_REQUIRED')) {
        pendingAction = () => openComposePull(name, configFiles);
        showSudoModal = true;
        showComposePullModal = false;
      } else {
        composePullStatus = 'error';
        composePullLogs = [...composePullLogs, 'Błąd: ' + err.toString()];
      }
    }
  }

  function parseComposePullProgress(data: string) {
    const lines = data.split('\n');
    for (const line of lines) {
      const cleanLine = line.trim();
      if (!cleanLine) continue;
      
      const pctMatch = cleanLine.match(/(\d+)%/);
      if (pctMatch) {
        const percent = parseInt(pctMatch[1], 10);
        const firstWord = cleanLine.split(/\s+/)[0] || 'layer';
        if (!composePullLayers[firstWord]) {
          composePullLayers[firstWord] = { service: firstWord, status: 'Pobieranie', percent: 0 };
        }
        composePullLayers[firstWord].percent = percent;
        composePullLayers[firstWord].status = cleanLine.includes('Extracting') ? 'Rozpakowywanie' : 'Pobieranie';
      } else if (cleanLine.includes('Pull complete') || cleanLine.includes('Already exists')) {
        const firstWord = cleanLine.split(/\s+/)[0] || 'layer';
        if (!composePullLayers[firstWord]) {
          composePullLayers[firstWord] = { service: firstWord, status: 'Ukończono', percent: 100 };
        } else {
          composePullLayers[firstWord].percent = 100;
          composePullLayers[firstWord].status = 'Ukończono';
        }
      }
    }

    const keys = Object.keys(composePullLayers);
    if (keys.length > 0) {
      const total = keys.reduce((acc, k) => acc + composePullLayers[k].percent, 0);
      composePullProgress = Math.min(Math.round(total / keys.length), 100);
    }
    
    if (data.includes('[Błąd pobierania') || data.includes('Error')) {
      composePullStatus = 'error';
    } else if (data.includes('Finished') || data.includes('Pull complete') && composePullProgress >= 100) {
      // Done
    }
  }

  async function closeComposePull() {
    try {
      await invoke('stop_compose_pull');
    } catch { /* ignore */ }
    if (composePullUnlisten) {
      composePullUnlisten();
      composePullUnlisten = null;
    }
    if (composePullStatus === 'pulling') {
      composePullStatus = 'finished';
      composePullProgress = 100;
    }
    showComposePullModal = false;
    await loadComposeProjects();
    await loadImages();
  }

  // ----------------------------------------------------------
  // Monaco Syntax validation
  // ----------------------------------------------------------
  function validateComposeContent(monaco: any, content: string) {
    if (!composeEditorInstance) return;
    const model = composeEditorInstance.getModel();
    if (!model) return;
    const markers: any[] = [];
    
    const ext = composeEditorPath.split('.').pop()?.toLowerCase();
    if (ext === 'yaml' || ext === 'yml') {
      try {
        yaml.load(content);
      } catch (e: any) {
        markers.push({
          message: e.message || 'Błąd składni YAML',
          severity: monaco.MarkerSeverity.Error,
          startLineNumber: e.mark ? e.mark.line + 1 : 1,
          startColumn: e.mark ? e.mark.column + 1 : 1,
          endLineNumber: e.mark ? e.mark.line + 1 : 1,
          endColumn: e.mark ? e.mark.column + 100 : 100,
        });
      }
    } else if (ext === 'toml') {
      try {
        toml.parse(content);
      } catch (e: any) {
        markers.push({
          message: e.message || 'Błąd składni TOML',
          severity: monaco.MarkerSeverity.Error,
          startLineNumber: e.line || 1,
          startColumn: e.column || 1,
          endLineNumber: e.line || 1,
          endColumn: (e.column || 1) + 10,
        });
      }
    }
    
    monaco.editor.setModelMarkers(model, "compose-validation", markers);
  }

  // ----------------------------------------------------------
  // Bulk selection helpers
  // ----------------------------------------------------------
  function toggleSelectContainer(id: string) {
    if (selectedContainers.includes(id)) {
      selectedContainers = selectedContainers.filter(x => x !== id);
    } else {
      selectedContainers = [...selectedContainers, id];
    }
  }

  function toggleSelectAllContainers() {
    const visible = getFilteredContainers();
    if (selectedContainers.length === visible.length) {
      selectedContainers = [];
    } else {
      selectedContainers = visible.map(c => c.ID);
    }
  }

  function toggleSelectImage(id: string) {
    if (selectedImages.includes(id)) {
      selectedImages = selectedImages.filter(x => x !== id);
    } else {
      selectedImages = [...selectedImages, id];
    }
  }

  function toggleSelectAllImages() {
    const visible = getFilteredImages();
    if (selectedImages.length === visible.length) {
      selectedImages = [];
    } else {
      selectedImages = visible.map(i => i.ID);
    }
  }

  function toggleSelectNetwork(id: string) {
    if (selectedNetworks.includes(id)) {
      selectedNetworks = selectedNetworks.filter(x => x !== id);
    } else {
      selectedNetworks = [...selectedNetworks, id];
    }
  }

  function toggleSelectAllNetworks() {
    if (selectedNetworks.length === networks.length) {
      selectedNetworks = [];
    } else {
      selectedNetworks = networks.map(n => n.ID || n.Name);
    }
  }

  function toggleSelectCompose(name: string) {
    if (selectedCompose.includes(name)) {
      selectedCompose = selectedCompose.filter(x => x !== name);
    } else {
      selectedCompose = [...selectedCompose, name];
    }
  }

  function toggleSelectAllCompose() {
    if (selectedCompose.length === composeProjects.length) {
      selectedCompose = [];
    } else {
      selectedCompose = composeProjects.map(p => p.Name || p.name);
    }
  }

  // Bulk actions executers
  async function runBulkContainerAction(action: string) {
    if (selectedContainers.length === 0) return;
    const ids = selectedContainers.join(' ');
    if (action === 'rm -f') {
      confirmMessage = `Usunąć zaznaczone kontenery (${selectedContainers.length})?`;
      confirmAction = async () => {
        await handleWithSudo(async () => {
          isLoading = true;
          await execDocker(`docker rm -f ${ids}`);
          selectedContainers = [];
          await loadContainers();
          showConfirmModal = false;
        });
      };
      showConfirmModal = true;
    } else {
      await handleWithSudo(async () => {
        isLoading = true;
        await execDocker(`docker ${action} ${ids}`);
        selectedContainers = [];
        await loadContainers();
      });
    }
  }

  async function runBulkImageAction() {
    if (selectedImages.length === 0) return;
    const ids = selectedImages.join(' ');
    confirmMessage = `Usunąć zaznaczone obrazy (${selectedImages.length})?`;
    confirmAction = async () => {
      await handleWithSudo(async () => {
        isLoading = true;
        try {
          await execDocker(`docker rmi ${ids}`);
        } catch (e: any) {
          errorMsg = 'Błąd usuwania części obrazów: ' + e.toString();
        }
        selectedImages = [];
        await loadImages();
        showConfirmModal = false;
      });
    };
    showConfirmModal = true;
  }

  async function runBulkNetworkAction() {
    if (selectedNetworks.length === 0) return;
    const safeIds = selectedNetworks.filter(n => n !== 'bridge' && n !== 'host' && n !== 'none').join(' ');
    if (!safeIds) {
      errorMsg = 'Nie można usunąć wbudowanych sieci.';
      return;
    }
    confirmMessage = `Usunąć zaznaczone sieci (${selectedNetworks.filter(n => n !== 'bridge' && n !== 'host' && n !== 'none').length})?`;
    confirmAction = async () => {
      await handleWithSudo(async () => {
        isLoading = true;
        try {
          await execDocker(`docker network rm ${safeIds}`);
        } catch (e: any) {
          errorMsg = 'Błąd usuwania sieci: ' + e.toString();
        }
        selectedNetworks = [];
        await loadNetworks();
        showConfirmModal = false;
      });
    };
    showConfirmModal = true;
  }

  async function runBulkComposeAction(action: string) {
    if (selectedCompose.length === 0) return;

    const execute = async () => {
      isLoading = true;
      errorMsg = '';
      for (const name of selectedCompose) {
        const project = composeProjects.find(p => (p.Name || p.name) === name);
        if (project) {
          const file = (project.ConfigFiles || project['config files'] || '').split(',')[0].trim();
          try {
            await execDocker(`docker compose -f ${file} ${action}`);
          } catch (e: any) {
            errorMsg += `Błąd projektu ${name}: ${e.toString()}\n`;
          }
        }
      }
      selectedCompose = [];
      await loadComposeProjects();
      isLoading = false;
      if (!errorMsg) {
        successMsg = `Compose ${action} — wykonano dla zaznaczonych projektów`;
        setTimeout(() => successMsg = '', 3000);
      }
    };

    if (action === 'down') {
      confirmMessage = `Zatrzymać zaznaczone projekty Compose (${selectedCompose.length})?`;
      confirmAction = async () => {
        await handleWithSudo(execute);
        showConfirmModal = false;
      };
      showConfirmModal = true;
    } else {
      await handleWithSudo(execute);
    }
  }

  async function runBulkComposePull() {
    if (selectedCompose.length === 0) return;
    await handleWithSudo(async () => {
      isLoading = true;
      errorMsg = '';
      for (const name of selectedCompose) {
        const project = composeProjects.find(p => (p.Name || p.name) === name);
        if (project) {
          const file = (project.ConfigFiles || project['config files'] || '').split(',')[0].trim();
          try {
            await execDocker(`docker compose -f ${file} pull`);
          } catch (e: any) {
            errorMsg += `Błąd pull ${name}: ${e.toString()}\n`;
          }
        }
      }
      selectedCompose = [];
      await loadComposeProjects();
      await loadImages();
      isLoading = false;
      if (!errorMsg) {
        successMsg = 'Obrazy pobrane dla zaznaczonych projektów';
        setTimeout(() => successMsg = '', 3000);
      }
    });
  }

  // ----------------------------------------------------------
  // Unused images check
  // ----------------------------------------------------------
  function isImageUnused(image: any) {
    const imgId = (image.ID || '').replace('sha256:', '').substring(0, 12);
    if (usedImageIds.length > 0) {
      return !usedImageIds.some(id => {
        const used = id.replace('sha256:', '').substring(0, 12);
        return used === imgId;
      });
    }
    const repoTag = `${image.Repository}:${image.Tag}`;
    return !containers.some((c: any) => {
      const cImageId = (c.ImageID || '').replace('sha256:', '').substring(0, 12);
      const cImage = c.Image || '';
      return cImageId === imgId || cImage === repoTag || cImage === image.Repository;
    });
  }

  // ----------------------------------------------------------
  // Volumes Management
  // ----------------------------------------------------------
  async function loadVolumes() {
    isLoading = true;
    errorMsg = '';
    try {
      const output = await execDocker("docker volume ls --format '{{json .}}'");
      const lines = output.trim().split('\n').filter(l => l.trim());
      const parsed = lines.map(line => {
        try { return JSON.parse(line); }
        catch { return null; }
      }).filter(Boolean);
      volumes = parsed;
    } catch (err: any) {
      if (err.toString().includes('SUDO_PASSWORD_REQUIRED')) {
        pendingAction = loadVolumes;
        showSudoModal = true;
      } else {
        errorMsg = 'Błąd wczytywania wolumenów: ' + err.toString();
      }
    } finally {
      isLoading = false;
    }
  }

  function getFilteredVolumes() {
    if (!volumeSearch) return volumes;
    const q = volumeSearch.toLowerCase();
    return volumes.filter(v => (v.Name || '').toLowerCase().includes(q));
  }

  async function inspectVolume(name: string) {
    if (expandedVolume === name) {
      expandedVolume = '';
      return;
    }
    expandedVolume = name;
    volumeInspectLoading = true;
    volumeInspectData = '';
    try {
      const output = await execDocker(`docker volume inspect ${name}`);
      volumeInspectData = JSON.stringify(JSON.parse(output), null, 2);
    } catch (err: any) {
      volumeInspectData = 'Błąd inspekcji: ' + err.toString();
    } finally {
      volumeInspectLoading = false;
    }
  }

  async function removeVolume(name: string) {
    confirmMessage = `Usunąć wolumen "${name}"? Dane w nim zawarte zostaną utracone.`;
    confirmAction = async () => {
      await handleWithSudo(async () => {
        isLoading = true;
        await execDocker(`docker volume rm ${name}`);
        await loadVolumes();
        showConfirmModal = false;
        successMsg = 'Wolumen usunięty';
        setTimeout(() => successMsg = '', 3000);
      });
    };
    showConfirmModal = true;
  }

  async function openVolumeBrowser(name: string) {
    browserVolumeName = name;
    browserRelativePath = '';
    browserErrorMsg = '';
    showVolumeBrowser = true;
    
    try {
      browserLoading = true;
      const inspectOut = await execDocker(`docker volume inspect --format '{{.Mountpoint}}' ${name}`);
      browserVolumePath = inspectOut.trim();
      await loadVolumeDirectory();
    } catch (err: any) {
      browserErrorMsg = 'Nie można odczytać ścieżki wolumenu: ' + err.toString();
    } finally {
      browserLoading = false;
    }
  }

  async function loadVolumeDirectory() {
    browserLoading = true;
    browserErrorMsg = '';
    const fullPath = `${browserVolumePath}${browserRelativePath}`;
    const escapedPath = "'" + fullPath.replace(/'/g, "'\\''") + "'";
    
    const cmd = `sudo sh -c '
    cd ${escapedPath} 2>/dev/null || exit 1
    for f in * .*; do
      if [ "$f" = "*" ] && [ ! -e "$f" ]; then continue; fi
      if [ "$f" = ".*" ] && [ ! -e "$f" ]; then continue; fi
      if [ "$f" = "." ] || [ "$f" = ".." ]; then continue; fi
      is_dir=false
      if [ -d "$f" ]; then is_dir=true; fi
      size=$(stat -c%s "$f" 2>/dev/null || echo 0)
      perms=$(stat -c%a "$f" 2>/dev/null || echo 0)
      mod=$(stat -c%Y "$f" 2>/dev/null || echo 0)
      echo "{\\"name\\":\\"$f\\",\\"is_dir\\":$is_dir,\\"size\\":$size,\\"permissions\\":\\"$perms\\",\\"modified\\":$mod}"
    done
    '`;

    try {
      const output = await execDocker(cmd);
      const lines = output.trim().split('\n').filter(l => l.trim());
      const parsed = lines.map(line => {
        try { return JSON.parse(line); }
        catch { return null; }
      }).filter(Boolean);
      
      browserEntries = parsed.sort((a: any, b: any) => {
        if (a.is_dir !== b.is_dir) return b.is_dir ? 1 : -1;
        return a.name.localeCompare(b.name);
      });
    } catch (err: any) {
      browserErrorMsg = 'Błąd wczytywania folderu: ' + err.toString();
      browserEntries = [];
    } finally {
      browserLoading = false;
    }
  }

  function navigateVolumeDir(name: string) {
    browserRelativePath = `${browserRelativePath}/${name}`;
    loadVolumeDirectory();
  }

  function navigateVolumeUp() {
    if (!browserRelativePath || browserRelativePath === '/') return;
    const parts = browserRelativePath.split('/');
    parts.pop();
    browserRelativePath = parts.join('/');
    loadVolumeDirectory();
  }

  async function editVolumeFile(name: string) {
    const relFilePath = `${browserRelativePath}/${name}`;
    const fullPath = `${browserVolumePath}${relFilePath}`;
    const escapedPath = "'" + fullPath.replace(/'/g, "'\\''") + "'";
    browserLoading = true;
    browserErrorMsg = '';

    try {
      const content = await execDocker(`sudo cat ${escapedPath}`);
      browserEditingFile = relFilePath;
      browserEditingContent = content;

      setTimeout(() => {
        if (browserEditorElement) {
          import('monaco-editor').then((monaco) => {
            if (browserEditorInstance) {
              browserEditorInstance.dispose();
            }
            const ext = name.split('.').pop()?.toLowerCase() || '';
            let lang = 'plaintext';
            if (['js', 'json', 'yml', 'yaml', 'html', 'css', 'conf', 'sh', 'py', 'toml'].includes(ext)) {
              lang = ext === 'yml' ? 'yaml' : ext;
            }
            browserEditorInstance = monaco.editor.create(browserEditorElement!, {
               value: content,
               language: lang,
               theme: 'vs-dark',
               automaticLayout: true,
               fontSize: 14,
               fontFamily: '"JetBrains Mono", Consolas, monospace',
               minimap: { enabled: false },
            });
          });
        }
      }, 100);
    } catch (err: any) {
      browserErrorMsg = 'Błąd odczytu pliku: ' + err.toString();
    } finally {
      browserLoading = false;
    }
  }

  async function saveVolumeFile() {
    if (!browserEditingFile || !browserEditorInstance) return;
    browserEditorSaving = true;
    browserErrorMsg = '';
    const content = browserEditorInstance.getValue();
    
    const tempFile = `/tmp/jarvis_vol_edit_${Math.random().toString(36).substring(7)}`;
    const destFile = `${browserVolumePath}${browserEditingFile}`;
    const escapedDest = "'" + destFile.replace(/'/g, "'\\''") + "'";

    try {
      await invoke('sftp_write', { path: tempFile, content });
      const escapedTemp = "'" + tempFile.replace(/'/g, "'\\''") + "'";
      await execDocker(`sudo mv ${escapedTemp} ${escapedDest}`);
      
      successMsg = 'Plik zapisany pomyślnie';
      setTimeout(() => successMsg = '', 3000);
      closeVolumeEditor();
      await loadVolumeDirectory();
    } catch (err: any) {
      browserErrorMsg = 'Błąd zapisu pliku: ' + err.toString();
    } finally {
      browserEditorSaving = false;
    }
  }

  function closeVolumeEditor() {
    browserEditingFile = null;
    if (browserEditorInstance) {
      browserEditorInstance.dispose();
      browserEditorInstance = null;
    }
  }

  // ----------------------------------------------------------
  // Container Recreation (Portainer-like modify)
  // ----------------------------------------------------------
  async function openModifyContainer(id: string) {
    modifyContainerId = id;
    modifyLoading = true;
    modifyPorts = [];
    modifyVolumes = [];
    modifyEnv = [];
    modifyNetworks = [];
    modifyCmd = '';
    modifyEntrypoint = '';
    showModifyModal = true;

    try {
      const inspectOut = await execDocker(`docker inspect ${id}`);
      const inspect = JSON.parse(inspectOut)[0];

      modifyName = (inspect.Name || '').replace(/^\//, '');
      modifyImage = inspect.Config.Image || '';
      
      const portBindings = inspect.HostConfig.PortBindings || {};
      for (const cPortProto of Object.keys(portBindings)) {
        const bindings = portBindings[cPortProto] || [];
        if (bindings.length > 0) {
          const parts = cPortProto.split('/');
          const container = parts[0];
          const proto = (parts[1] || 'tcp') as 'tcp' | 'udp';
          const host = bindings[0].HostPort || '';
          modifyPorts.push({ host, container, proto });
        }
      }

      const binds = inspect.HostConfig.Binds || [];
      for (const bind of binds) {
        const parts = bind.split(':');
        if (parts.length >= 2) {
          const host = parts[0];
          const container = parts[1];
          const ro = parts[2] === 'ro';
          modifyVolumes.push({ host, container, ro });
        }
      }

      const envList = inspect.Config.Env || [];
      for (const env of envList) {
        const idx = env.indexOf('=');
        if (idx !== -1) {
          const key = env.substring(0, idx);
          const value = env.substring(idx + 1);
          modifyEnv.push({ key, value });
        }
      }

      const netSettings = inspect.NetworkSettings.Networks || {};
      modifyNetworks = Object.keys(netSettings);

      modifyRestartPolicy = inspect.HostConfig.RestartPolicy.Name || 'unless-stopped';
      if (modifyRestartPolicy === 'no') modifyRestartPolicy = 'no';

      modifyCmd = (inspect.Config.Cmd || []).join(' ');
      modifyEntrypoint = (inspect.Config.Entrypoint || []).join(' ');

    } catch (err: any) {
      errorMsg = 'Błąd wczytywania konfiguracji kontenera: ' + err.toString();
      showModifyModal = false;
    } finally {
      modifyLoading = false;
    }
  }

  function addModifyPort() {
    modifyPorts = [...modifyPorts, { host: '', container: '', proto: 'tcp' }];
  }

  function removeModifyPort(idx: number) {
    modifyPorts = modifyPorts.filter((_, i) => i !== idx);
  }

  function addModifyVolume() {
    modifyVolumes = [...modifyVolumes, { host: '', container: '', ro: false }];
  }

  function removeModifyVolume(idx: number) {
    modifyVolumes = modifyVolumes.filter((_, i) => i !== idx);
  }

  function addModifyEnv() {
    modifyEnv = [...modifyEnv, { key: '', value: '' }];
  }

  function removeModifyEnv(idx: number) {
    modifyEnv = modifyEnv.filter((_, i) => i !== idx);
  }

  async function saveModifiedContainer() {
    modifyLoading = true;
    errorMsg = '';
    
    let runCmd = `docker run -d --name ${modifyName}`;
    
    if (modifyRestartPolicy) {
      runCmd += ` --restart ${modifyRestartPolicy}`;
    }
    
    for (const p of modifyPorts) {
      if (p.host && p.container) {
        runCmd += ` -p ${p.host}:${p.container}/${p.proto}`;
      }
    }

    for (const v of modifyVolumes) {
      if (v.host && v.container) {
        runCmd += ` -v ${v.host}:${v.container}${v.ro ? ':ro' : ''}`;
      }
    }

    for (const e of modifyEnv) {
      if (e.key) {
        const escapedVal = e.value.replace(/'/g, "'\\''");
        runCmd += ` -e ${e.key}='${escapedVal}'`;
      }
    }

    const primaryNet = modifyNetworks[0] || 'bridge';
    runCmd += ` --network ${primaryNet}`;

    if (modifyEntrypoint.trim()) {
      const ep = modifyEntrypoint.replace(/'/g, "'\\''");
      runCmd += ` --entrypoint '${ep}'`;
    }

    runCmd += ` ${modifyImage}`;

    if (modifyCmd.trim()) {
      runCmd += ` ${modifyCmd}`;
    }

    await handleWithSudo(async () => {
      try {
        await execDocker(`docker stop ${modifyContainerId}`);
        await execDocker(`docker rm ${modifyContainerId}`);
        const newContainerId = (await execDocker(runCmd)).trim();

        if (modifyNetworks.length > 1) {
          for (let i = 1; i < modifyNetworks.length; i++) {
            await execDocker(`docker network connect ${modifyNetworks[i]} ${newContainerId}`);
          }
        }

        successMsg = 'Kontener zmodyfikowany pomyślnie!';
        setTimeout(() => successMsg = '', 3000);
        showModifyModal = false;
        await loadContainers();
      } catch (err: any) {
        errorMsg = 'Błąd modyfikacji kontenera: ' + err.toString();
      }
    });
    modifyLoading = false;
  }

  // Compose Stack network modify
  async function openChangeComposeNetwork(project: any) {
    composeNetworkProject = project;
    composeSelectedNetwork = '';
    if (networks.length === 0) {
      await loadNetworks();
    }
    showComposeNetworkModal = true;
  }

  async function saveComposeNetwork() {
    if (!composeNetworkProject || !composeSelectedNetwork) return;
    composeNetworkLoading = true;
    errorMsg = '';
    
    const composeFile = (composeNetworkProject.ConfigFiles || composeNetworkProject['config files'] || '').split(',')[0].trim();
    
    try {
      const content: string = await invoke('sftp_read', { path: composeFile });
      let updatedContent = content;
      
      const lines = content.split('\n');
      let networksIdx = lines.findIndex(l => l.trim() === 'networks:');
      
      if (networksIdx !== -1) {
        const parsed = yaml.load(content) as any;
        if (parsed) {
          if (!parsed.networks) parsed.networks = {};
          parsed.networks.default = {
            name: composeSelectedNetwork,
            external: true
          };
          updatedContent = yaml.dump(parsed);
        }
      } else {
        updatedContent = content + `\n\nnetworks:\n  default:\n    name: "${composeSelectedNetwork}"\n    external: true\n`;
      }

      await invoke('sftp_write', { path: composeFile, content: updatedContent });
      await execDocker(`docker compose -f ${composeFile} up -d --remove-orphans`);
      
      successMsg = 'Sieć projektu compose zaktualizowana!';
      setTimeout(() => successMsg = '', 3000);
      showComposeNetworkModal = false;
      await loadComposeProjects();
    } catch (err: any) {
      errorMsg = 'Błąd zmiany sieci compose: ' + err.toString();
    } finally {
      composeNetworkLoading = false;
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
      case 'volumes': await loadVolumes(); break;
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
    finally {
      containersLoading = false;
    }
  }

  onMount(async () => {
    await checkDockerStatus();
    if (dockerInstalled && dockerVersion) {
      await loadAllStats();
    } else {
      containersLoading = false;
    }
  });

  onDestroy(() => {
    if (logsUnlisten) {
      logsUnlisten();
      invoke('stop_container_logs').catch(() => {});
    }
    if (composePullUnlisten) {
      composePullUnlisten();
      invoke('stop_compose_pull').catch(() => {});
    }
    if (composeEditorInstance) {
      composeEditorInstance.dispose();
    }
    if (browserEditorInstance) {
      browserEditorInstance.dispose();
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
      <button class="tab-btn {dockerTab === 'volumes' ? 'active' : ''}" onclick={() => { dockerTab = 'volumes'; loadVolumes(); }}>
        <Database size={16} /> Wolumeny
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
          {#if selectedContainers.length > 0}
            <div class="bulk-actions">
              <span class="bulk-count">Zaznaczono: {selectedContainers.length}</span>
              <button class="secondary btn-sm" onclick={() => runBulkContainerAction('start')}><Play size={12} /> Start</button>
              <button class="secondary btn-sm" onclick={() => runBulkContainerAction('stop')}><Square size={12} /> Stop</button>
              <button class="secondary btn-sm" onclick={() => runBulkContainerAction('restart')}><RotateCw size={12} /> Restart</button>
              <button class="danger btn-sm" onclick={() => runBulkContainerAction('rm -f')}><Trash2 size={12} /> Usuń</button>
            </div>
          {/if}
          <button class="secondary" onclick={loadContainers} disabled={isLoading}>
            <RefreshCw size={16} class={isLoading ? 'spin' : ''} /> Odśwież
          </button>
        </div>

        <div class="table-container glass">
          {#if containersLoading}
            <div class="loading-state">
              <Loader2 class="spin" size={36} />
              <p>Wczytywanie kontenerów...</p>
            </div>
          {:else}
            <table class="data-table">
              <thead>
                <tr>
                  <th style="width: 5%;"><input type="checkbox" checked={selectedContainers.length > 0 && selectedContainers.length === getFilteredContainers().length} onchange={toggleSelectAllContainers} /></th>
                  <th style="width: 20%;">Nazwa</th>
                  <th style="width: 18%;">Obraz</th>
                  <th style="width: 12%;">Status</th>
                  <th style="width: 15%;">Porty</th>
                  <th style="width: 13%;">Utworzono</th>
                  <th style="width: 17%; text-align: right;">Operacje</th>
                </tr>
              </thead>
              <tbody>
                {#each getFilteredContainers() as container}
                  <tr>
                    <td><input type="checkbox" checked={selectedContainers.includes(container.ID)} onchange={() => toggleSelectContainer(container.ID)} /></td>
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
                      <button class="btn-action" onclick={() => openModifyContainer(container.ID)} title="Modyfikuj (Portainer-like)">
                        <Edit size={14} class="accent-amber-text" />
                      </button>
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
                        <button class="btn-action amber-text" onclick={() => openInteractiveShell(container.ID, container.Names)} title="Interaktywny shell">
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
                      <td colspan="7">
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

                {#if getFilteredContainers().length === 0 && !containersLoading}
                  <tr>
                    <td colspan="7" class="empty-state">
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
          {#if selectedImages.length > 0}
            <div class="bulk-actions">
              <span class="bulk-count">Zaznaczono: {selectedImages.length}</span>
              <button class="danger btn-sm" onclick={runBulkImageAction}><Trash2 size={12} /> Usuń wybrane</button>
            </div>
          {/if}
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
                  <th style="width: 5%;"><input type="checkbox" checked={selectedImages.length > 0 && selectedImages.length === getFilteredImages().length} onchange={toggleSelectAllImages} /></th>
                  <th style="width: 25%;">Repozytorium</th>
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
                    <td><input type="checkbox" checked={selectedImages.includes(image.ID)} onchange={() => toggleSelectImage(image.ID)} /></td>
                    <td class="mono-val">
                      <strong>{image.Repository}</strong>
                      {#if isImageUnused(image)}
                        <span class="badge secondary" style="margin-left: 8px; font-size: 0.7rem; padding: 2px 6px;">Nieużywany</span>
                      {/if}
                    </td>
                    <td><span class="badge warning">{image.Tag}</span></td>
                    <td class="mono-val id-cell">{image.ID}</td>
                    <td class="mono-val">{image.Size}</td>
                    <td class="time-cell">{image.CreatedSince || image.CreatedAt || '—'}</td>
                    <td class="actions-cell">
                      {#if isImageUnused(image)}
                        <button class="btn-action danger-text" onclick={() => removeImage(image.ID, image.Repository + ':' + image.Tag)} title="Usuń nieużywany obraz">
                          <Trash2 size={14} />
                        </button>
                      {/if}
                    </td>
                  </tr>
                {/each}

                {#if getFilteredImages().length === 0 && !isLoading}
                  <tr>
                    <td colspan="7" class="empty-state">
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
          {#if selectedNetworks.length > 0}
            <div class="bulk-actions">
              <span class="bulk-count">Zaznaczono: {selectedNetworks.length}</span>
              <button class="danger btn-sm" onclick={runBulkNetworkAction}><Trash2 size={12} /> Usuń wybrane</button>
            </div>
          {/if}
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
                  <th style="width: 5%;"><input type="checkbox" checked={selectedNetworks.length > 0 && selectedNetworks.length === networks.length} onchange={toggleSelectAllNetworks} /></th>
                  <th style="width: 25%;">Nazwa</th>
                  <th style="width: 18%;">Driver</th>
                  <th style="width: 15%;">Zakres</th>
                  <th style="width: 22%;">ID Sieci</th>
                  <th style="width: 15%; text-align: right;">Operacje</th>
                </tr>
              </thead>
              <tbody>
                {#each networks as network}
                  <tr>
                    <td><input type="checkbox" checked={selectedNetworks.includes(network.ID || network.Name)} onchange={() => toggleSelectNetwork(network.ID || network.Name)} /></td>
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
                      <td colspan="6">
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
                    <td colspan="6" class="empty-state">Brak sieci Docker</td>
                  </tr>
                {/if}
              </tbody>
            </table>
          {/if}
        </div>

      {:else if dockerTab === 'compose'}
        <!-- ======== COMPOSE TAB ======== -->
        <div class="ops-bar glass">
          {#if selectedCompose.length > 0}
            <div class="bulk-actions">
              <span class="bulk-count">Zaznaczono: {selectedCompose.length}</span>
              <button class="secondary btn-sm" onclick={() => runBulkComposeAction('up -d')}><Play size={12} /> Up</button>
              <button class="secondary btn-sm" onclick={() => runBulkComposeAction('down')}><Square size={12} /> Down</button>
              <button class="secondary btn-sm" onclick={() => runBulkComposeAction('restart')}><RotateCw size={12} /> Restart</button>
              <button class="secondary btn-sm" onclick={runBulkComposePull}><Download size={12} /> Pull</button>
            </div>
          {/if}
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
                  <th style="width: 5%;"><input type="checkbox" checked={selectedCompose.length > 0 && selectedCompose.length === composeProjects.length} onchange={toggleSelectAllCompose} /></th>
                  <th style="width: 20%;">Nazwa Projektu</th>
                  <th style="width: 12%;">Status</th>
                  <th style="width: 28%;">Plik Konfiguracyjny</th>
                  <th style="width: 35%; text-align: right;">Operacje</th>
                </tr>
              </thead>
              <tbody>
                {#each composeProjects as project}
                  {@const projectName = project.Name || project.name || '—'}
                  {@const configFiles = project.ConfigFiles || project['config files'] || ''}
                  <tr>
                    <td><input type="checkbox" checked={selectedCompose.includes(projectName)} onchange={() => toggleSelectCompose(projectName)} /></td>
                    <td class="mono-val"><strong>{projectName}</strong></td>
                    <td>
                      <span class="badge {(project.Status || project.status || '').toLowerCase().includes('running') ? 'success' : 'danger'}">
                        {project.Status || project.status || '—'}
                      </span>
                    </td>
                    <td class="mono-val config-cell" title={configFiles}>{configFiles || '—'}</td>
                    <td class="actions-cell compose-actions">
                      <button class="secondary btn-sm" onclick={() => composeAction('up -d', configFiles)}>
                        <Play size={14} /> Up
                      </button>
                      <button class="secondary btn-sm" onclick={() => composeAction('down', configFiles)}>
                        <Square size={14} /> Down
                      </button>
                      <button class="secondary btn-sm" onclick={() => composeAction('restart', configFiles)}>
                        <RotateCw size={14} /> Restart
                      </button>
                      <button class="secondary btn-sm" onclick={() => openComposePull(projectName, configFiles)} title="Pobierz obrazy">
                        <Download size={14} /> Pull
                      </button>
                      <button class="btn-action" onclick={() => openComposeEditor(configFiles)} title="Edytuj plik compose">
                        <FileText size={14} />
                      </button>
                      <button class="btn-action" onclick={() => openComposeLogs(projectName, configFiles)} title="Logi projektu">
                        <Eye size={14} />
                      </button>
                      <button class="btn-action" onclick={() => openChangeComposeNetwork(project)} title="Zmień sieć">
                        <Unplug size={14} />
                      </button>
                    </td>
                  </tr>
                {/each}

                {#if composeProjects.length === 0 && !isLoading}
                  <tr>
                    <td colspan="5" class="empty-state">
                      Brak projektów Docker Compose. Utwórz nowy projekt przyciskiem powyżej.
                    </td>
                  </tr>
                {/if}
              </tbody>
            </table>
          {/if}
        </div>

      {:else if dockerTab === 'volumes'}
        <!-- ======== VOLUMES TAB ======== -->
        <div class="ops-bar glass">
          <div class="search-bar">
            <Search size={16} class="search-icon" />
            <input type="text" placeholder="Szukaj wolumenów..." bind:value={volumeSearch} />
          </div>
          <button class="secondary" onclick={loadVolumes} disabled={isLoading}>
            <RefreshCw size={16} class={isLoading ? 'spin' : ''} /> Odśwież
          </button>
        </div>

        <div class="table-container glass">
          {#if isLoading && volumes.length === 0}
            <div class="loading-state">
              <RefreshCw class="spin" size={32} />
              <p>Wczytywanie wolumenów...</p>
            </div>
          {:else}
            <table class="data-table">
              <thead>
                <tr>
                  <th style="width: 30%;">Nazwa</th>
                  <th style="width: 20%;">Driver</th>
                  <th style="width: 35%; text-align: right;">Operacje</th>
                </tr>
              </thead>
              <tbody>
                {#each getFilteredVolumes() as volume}
                  <tr>
                    <td class="mono-val"><strong>{volume.Name}</strong></td>
                    <td><span class="badge warning">{volume.Driver || 'local'}</span></td>
                    <td class="actions-cell">
                      <button class="btn-action" onclick={() => inspectVolume(volume.Name)} title="Inspekcja">
                        <Eye size={14} />
                      </button>
                      <button class="btn-action" onclick={() => openVolumeBrowser(volume.Name)} title="Przeglądaj zawartość">
                        <FolderOpen size={14} />
                      </button>
                      <button class="btn-action danger-text" onclick={() => removeVolume(volume.Name)} title="Usuń wolumen">
                        <Trash2 size={14} />
                      </button>
                    </td>
                  </tr>

                  {#if expandedVolume === volume.Name}
                    <tr class="inspect-row">
                      <td colspan="3">
                        <div class="inspect-card">
                          <div class="inspect-header">
                            <span class="inspect-title">Inspekcja wolumenu: {volume.Name}</span>
                            <button class="btn-action" onclick={() => expandedVolume = ''}>
                              <X size={14} />
                            </button>
                          </div>
                          {#if volumeInspectLoading}
                            <div class="inspect-loading"><Loader2 size={20} class="spin" /> Ładowanie...</div>
                          {:else}
                            <pre class="inspect-json">{volumeInspectData}</pre>
                          {/if}
                        </div>
                      </td>
                    </tr>
                  {/if}
                {/each}

                {#if getFilteredVolumes().length === 0 && !isLoading}
                  <tr>
                    <td colspan="3" class="empty-state">
                      {#if volumes.length === 0}
                        Brak wolumenów Docker.
                      {:else}
                        Brak wyników dla frazy „{volumeSearch}"
                      {/if}
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
        <div class="logs-display" use:stickToBottom>
          <pre class="log-text">{getFilteredLogs().join('\n') || 'Oczekiwanie na logi...'}</pre>
        </div>
      </div>
    </div>
  {/if}

  <!-- Container Shell Modal -->
  {#if showShellModal}
    <div class="modal-overlay">
      <div class="modal-content glass exec-modal">
        <h3>Interaktywny shell: {shellPickContainerName}</h3>
        <p class="modal-desc">Wybierz powłokę dostępną w kontenerze.</p>
        <div class="form-group">
          <label for="shell-select">Powłoka</label>
          <select id="shell-select" bind:value={selectedShell}>
            {#each shellOptions as opt}
              <option value={opt.value}>{opt.label}</option>
            {/each}
          </select>
        </div>
        <div class="modal-actions">
          <button class="primary" onclick={confirmShellLaunch}>
            <Terminal size={16} /> Otwórz terminal
          </button>
          <button class="secondary" onclick={() => showShellModal = false}>Anuluj</button>
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
          <div class="exec-output-container" use:stickToBottom>
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
          <div class="pull-progress" use:stickToBottom>
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
    <div class="modal-overlay fullscreen-overlay">
      <div class="modal-content glass fullscreen-modal dir-picker-modal">
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
    <div class="modal-overlay fullscreen-overlay">
      <div class="modal-content glass fullscreen-modal compose-editor-modal">
        <div class="compose-editor-header">
          <h3>Edytor Compose</h3>
          <span class="mono-val compose-filepath">{composeEditorPath}</span>
        </div>
        <div bind:this={composeEditorElement} class="compose-editor-container"></div>
        <div class="modal-actions">
          <button class="primary" onclick={saveComposeFile} disabled={composeEditorSaving}>
            {#if composeEditorSaving}
              <Loader2 size={16} class="spin" /> Zapisywanie...
            {:else}
              Zapisz
            {/if}
          </button>
          <button class="secondary" onclick={closeComposeEditor}>Zamknij</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Compose Pull Modal -->
  {#if showComposePullModal}
    <div class="modal-overlay logs-overlay">
      <div class="modal-content glass compose-pull-modal">
        <div class="logs-header">
          <h3>Pull obrazów: {composePullProjectName}</h3>
          <button class="secondary btn-sm" onclick={closeComposePull}>
            <X size={14} /> Zamknij
          </button>
        </div>

        <div class="pull-progress-section">
          <div class="pull-progress-bar">
            <div class="pull-progress-fill" style="width: {composePullProgress}%"></div>
          </div>
          <span class="pull-progress-label mono-val">{composePullProgress}% — {composePullStatus === 'pulling' ? 'Pobieranie...' : composePullStatus === 'finished' ? 'Zakończono' : composePullStatus === 'error' ? 'Błąd' : 'Oczekiwanie'}</span>
        </div>

        {#if Object.keys(composePullLayers).length > 0}
          <div class="pull-layers">
            {#each Object.entries(composePullLayers) as [key, layer]}
              <div class="pull-layer-row">
                <span class="mono-val layer-name">{layer.service}</span>
                <span class="layer-status">{layer.status}</span>
                <div class="layer-bar">
                  <div class="layer-bar-fill" style="width: {layer.percent}%"></div>
                </div>
                <span class="mono-val layer-pct">{layer.percent}%</span>
              </div>
            {/each}
          </div>
        {/if}

        <div class="logs-display pull-logs" use:stickToBottom>
          <pre class="log-text">{composePullLogs.join('') || 'Oczekiwanie na dane pobierania...'}</pre>
        </div>
      </div>
    </div>
  {/if}

  <!-- Volume Browser Modal -->
  {#if showVolumeBrowser}
    <div class="modal-overlay fullscreen-overlay logs-overlay">
      <div class="modal-content glass fullscreen-modal volume-browser-modal">
        {#if browserEditingFile}
          <div class="volume-editor-header">
            <h3>Edycja: {browserEditingFile.split('/').pop()}</h3>
            <div class="volume-editor-actions">
              <button class="primary btn-sm" onclick={saveVolumeFile} disabled={browserEditorSaving}>
                {#if browserEditorSaving}
                  <Loader2 size={14} class="spin" /> Zapisywanie...
                {:else}
                  <Save size={14} /> Zapisz
                {/if}
              </button>
              <button class="secondary btn-sm" onclick={closeVolumeEditor}>
                <X size={14} /> Zamknij edytor
              </button>
            </div>
          </div>
          <div bind:this={browserEditorElement} class="volume-editor-container"></div>
        {:else}
          <div class="logs-header">
            <h3>Wolumen: {browserVolumeName}</h3>
            <button class="secondary btn-sm" onclick={() => { showVolumeBrowser = false; closeVolumeEditor(); }}>
              <X size={14} /> Zamknij
            </button>
          </div>
          <div class="volume-path-bar glass">
            <button class="secondary btn-sm" onclick={navigateVolumeUp} disabled={!browserRelativePath}>
              <ArrowUp size={14} /> W górę
            </button>
            <span class="mono-val volume-path">{browserVolumePath}{browserRelativePath}</span>
          </div>
          {#if browserErrorMsg}
            <div class="error-text">{browserErrorMsg}</div>
          {/if}
          <div class="volume-file-list">
            {#if browserLoading}
              <div class="loading-state"><Loader2 size={24} class="spin" /></div>
            {:else}
              {#each browserEntries as entry}
                <button
                  class="volume-file-entry"
                  onclick={() => entry.is_dir ? navigateVolumeDir(entry.name) : editVolumeFile(entry.name)}
                >
                  {#if entry.is_dir}
                    <FolderOpen size={14} class="accent-amber-text" />
                  {:else}
                    <FileText size={14} />
                  {/if}
                  <span>{entry.name}</span>
                  {#if !entry.is_dir}
                    <span class="mono-val file-size">{entry.size} B</span>
                  {/if}
                </button>
              {/each}
              {#if browserEntries.length === 0}
                <div class="dir-picker-empty">Katalog pusty lub brak dostępu</div>
              {/if}
            {/if}
          </div>
        {/if}
      </div>
    </div>
  {/if}

  <!-- Modify Container Modal -->
  {#if showModifyModal}
    <div class="modal-overlay fullscreen-overlay">
      <div class="modal-content glass fullscreen-modal modify-modal">
        <div class="fullscreen-modal-header">
          <h3>Modyfikuj kontener</h3>
          <button class="secondary btn-sm" onclick={() => showModifyModal = false} title="Zamknij">
            <X size={16} />
          </button>
        </div>
        {#if modifyLoading}
          <div class="inspect-loading"><Loader2 size={24} class="spin" /> Wczytywanie konfiguracji...</div>
        {:else}
          <div class="modify-form">
            <div class="form-row">
              <div class="form-group">
                <label for="mod-name">Nazwa</label>
                <input id="mod-name" type="text" bind:value={modifyName} />
              </div>
              <div class="form-group">
                <label for="mod-image">Obraz</label>
                <input id="mod-image" type="text" bind:value={modifyImage} />
              </div>
            </div>

            <div class="form-group">
              <label>Restart Policy</label>
              <select bind:value={modifyRestartPolicy}>
                <option value="no">no</option>
                <option value="always">always</option>
                <option value="unless-stopped">unless-stopped</option>
                <option value="on-failure">on-failure</option>
              </select>
            </div>

            <div class="modify-section">
              <div class="modify-section-header">
                <span>Mapowanie portów</span>
                <button class="secondary btn-sm" onclick={addModifyPort}><Plus size={12} /> Dodaj</button>
              </div>
              {#each modifyPorts as port, i}
                <div class="modify-row">
                  <input type="text" placeholder="Host" bind:value={port.host} class="mono-val" />
                  <span>→</span>
                  <input type="text" placeholder="Kontener" bind:value={port.container} class="mono-val" />
                  <select bind:value={port.proto}>
                    <option value="tcp">TCP</option>
                    <option value="udp">UDP</option>
                  </select>
                  <button class="btn-action danger-text" onclick={() => removeModifyPort(i)}><X size={14} /></button>
                </div>
              {/each}
            </div>

            <div class="modify-section">
              <div class="modify-section-header">
                <span>Wolumeny / montowania</span>
                <button class="secondary btn-sm" onclick={addModifyVolume}><Plus size={12} /> Dodaj</button>
              </div>
              {#each modifyVolumes as vol, i}
                <div class="modify-row">
                  <input type="text" placeholder="Ścieżka hosta" bind:value={vol.host} class="mono-val" />
                  <span>→</span>
                  <input type="text" placeholder="Ścieżka kontenera" bind:value={vol.container} class="mono-val" />
                  <label class="ro-label"><input type="checkbox" bind:checked={vol.ro} /> RO</label>
                  <button class="btn-action danger-text" onclick={() => removeModifyVolume(i)}><X size={14} /></button>
                </div>
              {/each}
            </div>

            <div class="modify-section">
              <div class="modify-section-header">
                <span>Zmienne środowiskowe</span>
                <button class="secondary btn-sm" onclick={addModifyEnv}><Plus size={12} /> Dodaj</button>
              </div>
              {#each modifyEnv as env, i}
                <div class="modify-row">
                  <input type="text" placeholder="Klucz" bind:value={env.key} class="mono-val" />
                  <span>=</span>
                  <input type="text" placeholder="Wartość" bind:value={env.value} class="mono-val" />
                  <button class="btn-action danger-text" onclick={() => removeModifyEnv(i)}><X size={14} /></button>
                </div>
              {/each}
            </div>

            <div class="form-group">
              <label for="mod-cmd">CMD</label>
              <input id="mod-cmd" type="text" bind:value={modifyCmd} class="mono-val" placeholder="np. nginx -g 'daemon off;'" />
            </div>
            <div class="form-group">
              <label for="mod-entry">Entrypoint</label>
              <input id="mod-entry" type="text" bind:value={modifyEntrypoint} class="mono-val" />
            </div>

            <div class="form-group">
              <label>Sieci (pierwsza = podstawowa)</label>
              <div class="network-tags">
                {#each modifyNetworks as net}
                  <span class="badge warning">{net}</span>
                {/each}
              </div>
            </div>
          </div>

          <div class="modal-actions">
            <button class="primary" onclick={saveModifiedContainer} disabled={modifyLoading}>
              Zastosuj zmiany
            </button>
            <button class="secondary" onclick={() => showModifyModal = false}>Anuluj</button>
          </div>
        {/if}
      </div>
    </div>
  {/if}

  <!-- Compose Network Change Modal -->
  {#if showComposeNetworkModal}
    <div class="modal-overlay">
      <div class="modal-content glass">
        <h3>Zmień sieć projektu</h3>
        <p class="modal-desc">
          Projekt: <strong>{composeNetworkProject?.Name || composeNetworkProject?.name}</strong><br />
          Zostanie dodana sekcja <code>networks.default</code> z wybraną siecią zewnętrzną.
        </p>
        <div class="form-group">
          <label for="compose-net-select">Sieć zewnętrzna</label>
          <select id="compose-net-select" bind:value={composeSelectedNetwork}>
            <option value="">— wybierz sieć —</option>
            {#each networks as network}
              <option value={network.Name}>{network.Name} ({network.Driver})</option>
            {/each}
          </select>
        </div>
        <div class="modal-actions">
          <button class="primary" onclick={saveComposeNetwork} disabled={!composeSelectedNetwork || composeNetworkLoading}>
            {#if composeNetworkLoading}
              <Loader2 size={16} class="spin" /> Zapisuję...
            {:else}
              Zastosuj i uruchom
            {/if}
          </button>
          <button class="secondary" onclick={() => showComposeNetworkModal = false}>Anuluj</button>
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
        <div class="logs-display" use:stickToBottom>
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
    flex-wrap: wrap;
  }

  .bulk-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    padding: 4px 10px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
  }

  .bulk-count {
    font-size: 0.8rem;
    color: var(--accent-amber);
    font-weight: 600;
    margin-right: 4px;
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

  .modal-overlay.fullscreen-overlay {
    align-items: stretch;
    justify-content: stretch;
    padding: 0;
    background: var(--bg-primary);
    backdrop-filter: none;
  }

  .modal-content {
    width: 440px;
    padding: 24px;
    border-radius: var(--radius-md);
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .modal-content.fullscreen-modal {
    width: 100%;
    height: 100%;
    max-width: none;
    max-height: none;
    border-radius: 0;
    padding: 20px 28px;
    overflow: hidden;
  }

  .fullscreen-modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-shrink: 0;
    gap: 16px;
    padding-bottom: 4px;
    border-bottom: 1px solid var(--border-color);
  }

  .fullscreen-modal-header h3 {
    font-size: 1.15rem;
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
    width: 100%;
    max-height: none;
  }

  .dir-picker-modal .dir-picker-list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
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
    width: 100%;
    max-height: none;
  }

  .compose-editor-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    flex-shrink: 0;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--border-color);
  }

  .compose-filepath {
    font-size: 0.72rem;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 300px;
  }

  .compose-editor-container {
    width: 100%;
    flex: 1;
    min-height: 0;
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    overflow: hidden;
  }

  /* Compose Pull modal */
  .compose-pull-modal {
    width: 80vw;
    max-width: 900px;
    height: 70vh;
    max-height: 650px;
  }

  .pull-progress-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .pull-progress-bar {
    height: 8px;
    background: var(--bg-primary);
    border-radius: 4px;
    overflow: hidden;
    border: 1px solid var(--border-color);
  }

  .pull-progress-fill {
    height: 100%;
    background: linear-gradient(90deg, var(--accent-amber), var(--accent-green));
    transition: width 0.3s ease;
  }

  .pull-progress-label {
    font-size: 0.8rem;
    color: var(--text-secondary);
  }

  .pull-layers {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-height: 120px;
    overflow-y: auto;
  }

  .pull-layer-row {
    display: grid;
    grid-template-columns: 1fr auto 100px 40px;
    gap: 8px;
    align-items: center;
    font-size: 0.75rem;
  }

  .layer-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .layer-status {
    color: var(--text-muted);
    font-size: 0.72rem;
  }

  .layer-bar {
    height: 4px;
    background: var(--bg-primary);
    border-radius: 2px;
    overflow: hidden;
  }

  .layer-bar-fill {
    height: 100%;
    background: var(--accent-amber);
    transition: width 0.2s ease;
  }

  .layer-pct {
    text-align: right;
    color: var(--text-muted);
  }

  .pull-logs {
    flex: 1;
    min-height: 0;
  }

  /* Volume browser */
  .volume-browser-modal {
    width: 100%;
    max-width: none;
    height: 100%;
    max-height: none;
  }

  .volume-path-bar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
    border-radius: var(--radius-sm);
  }

  .volume-path {
    font-size: 0.8rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .volume-file-list {
    flex: 1;
    overflow-y: auto;
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    background: var(--bg-primary);
    min-height: 0;
  }

  .volume-file-entry {
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

  .volume-file-entry:hover {
    background: var(--bg-hover);
  }

  .file-size {
    margin-left: auto;
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  .volume-editor-container {
    flex: 1;
    min-height: 0;
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    overflow: hidden;
  }

  .volume-editor-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
  }

  .volume-editor-actions {
    display: flex;
    gap: 8px;
  }

  /* Modify container modal */
  .modify-modal {
    width: 100%;
    max-height: none;
    overflow: hidden;
  }

  .modify-form {
    display: flex;
    flex-direction: column;
    gap: 14px;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding-right: 8px;
  }

  .modify-modal .modal-actions {
    flex-shrink: 0;
    padding-top: 12px;
    border-top: 1px solid var(--border-color);
    margin-top: auto;
  }

  .form-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }

  .modify-section {
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    padding: 12px;
    background: rgba(255, 255, 255, 0.02);
  }

  .modify-section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 10px;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--text-secondary);
  }

  .modify-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
  }

  .modify-row input {
    flex: 1;
    font-size: 0.82rem;
  }

  .modify-row select {
    width: auto;
    min-width: 70px;
  }

  .ro-label {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 0.75rem;
    color: var(--text-muted);
    white-space: nowrap;
  }

  .network-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  select {
    width: 100%;
  }
</style>
