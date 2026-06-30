<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import {
    Container, Box, Network, Layers, Activity,
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
  import SortableTh from './ui/SortableTh.svelte';
  import ListSortBar from './ui/ListSortBar.svelte';
  import PathAutocomplete from './ui/PathAutocomplete.svelte';
  import { applySort, nextSort, type SortState } from '$lib/sort/sortUtils';
  import { get } from 'svelte/store';
    import { notifications } from '$lib/notifications.svelte';
  import {
    formatInvokeError,
    isSudoPasswordIncorrect,
    isSudoPasswordRequired,
    parseAppError,
  } from '$lib/backendErrors';
  import { validateContent } from '$lib/syntaxValidator';

  // Props
  let { onRequestTerminalExec = (_ctx: { containerId: string; containerName: string; useSudo: boolean; shell: string }) => {}, visible = true } = $props();

  export async function refresh() {
    await checkDockerStatus();
    if (dockerInstalled && dockerVersion) await loadAllStats();
  }

  // Sub-tab state
  let dockerTab = $state<'containers' | 'images' | 'networks' | 'compose' | 'volumes' | 'stats'>('containers');

  // Global loading/error
  let isLoading = $state(false);
  let errorMsg = $state('');
  let successMsg = $state('');

  $effect(() => {
    if (errorMsg) {
      notifications.error(errorMsg);
      errorMsg = '';
    }
  });

  $effect(() => {
    if (successMsg) {
      notifications.success(successMsg);
      successMsg = '';
    }
  });

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

  // Container detail view (Portainer-like)
  let detailContainerId = $state('');
  let detailContainerName = $state('');
  let detailLoading = $state(false);
  let detailData = $state<any>(null);
  let detailRestartPolicy = $state('unless-stopped');
  let detailRenaming = $state(false);
  let detailNewName = $state('');
  let detailJoinNetwork = $state('');
  let detailShowRaw = $state(false);
  let detailRawData = $state('');

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
  const shellOptions = $derived([
    { value: '/bin/bash', label: "bash" },
    { value: '/bin/sh', label: 'sh' },
    { value: '/bin/ash', label: "ash (Alpine)" },
    { value: '/bin/zsh', label: "zsh" },
  ]);
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

  // Table sorting
  let containerSort = $state<SortState<string>>({ column: 'name', direction: 'asc' });
  let imageSort = $state<SortState<string>>({ column: 'repository', direction: 'asc' });
  let networkSort = $state<SortState<string>>({ column: 'name', direction: 'asc' });
  let composeSort = $state<SortState<string>>({ column: 'name', direction: 'asc' });
  let volumeSort = $state<SortState<string>>({ column: 'name', direction: 'asc' });
  let dirPickerSort = $state<SortState<string>>({ column: 'name', direction: 'asc' });
  let browserSort = $state<SortState<string>>({ column: 'name', direction: 'asc' });

  function getSortedContainers() {
    return applySort(getFilteredContainers(), containerSort, {
      name: (c) => c.Names || '',
      image: (c) => c.Image || '',
      status: (c) => c.State || '',
      ports: (c) => c.Ports || '',
      created: (c) => c.CreatedAt || c.RunningFor || '',
    });
  }

  function getSortedImages() {
    return applySort(getFilteredImages(), imageSort, {
      repository: (i) => i.Repository || '',
      tag: (i) => i.Tag || '',
      id: (i) => i.ID || '',
      size: (i) => i.Size || '',
      created: (i) => i.CreatedAt || '',
    });
  }

  function getSortedNetworks() {
    return applySort(networks, networkSort, {
      name: (n) => n.Name || '',
      driver: (n) => n.Driver || '',
      scope: (n) => n.Scope || '',
      id: (n) => n.ID || '',
    });
  }

  function getSortedCompose() {
    return applySort(composeProjects, composeSort, {
      name: (p) => p.Name || p.name || '',
      status: (p) => p.Status || p.status || '',
      config: (p) => p.ConfigFiles || p['config files'] || p.config_file || '',
    });
  }

  function getSortedVolumes() {
    return applySort(getFilteredVolumes(), volumeSort, {
      name: (v) => v.Name || '',
      driver: (v) => v.Driver || '',
    });
  }

  function getSortedDirPickerEntries() {
    return applySort(dirPickerEntries, dirPickerSort, {
      name: (e) => e.name || e.filename || '',
    });
  }

  function getSortedBrowserEntries() {
    return applySort(
      browserEntries,
      browserSort,
      {
        name: (e) => e.name || '',
        size: (e) => e.size || 0,
        modified: (e) => e.modified || 0,
      },
      { dirsFirst: (e) => e.is_dir },
    );
  }

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
  let browserSyntaxError = $state<string | null>(null);
  let browserErrorMsg = $state('');

  $effect(() => {
    if (browserErrorMsg) {
      notifications.error(browserErrorMsg);
      browserErrorMsg = '';
    }
  });

  // Container edit state (inline Portainer-like editor)
  let modifyContainerId = $state('');
  let modifyName = $state('');
  let modifyImage = $state('');
  let modifyPorts = $state<{ host: string, container: string, proto: 'tcp' | 'udp' }[]>([]);
  let modifyVolumes = $state<{ host: string, container: string, ro: boolean }[]>([]);
  let modifyEnv = $state<{ key: string, value: string }[]>([]);
  let modifyLabels = $state<{ key: string, value: string }[]>([]);
  let modifyNetworks = $state<string[]>([]);
  let modifyRestartPolicy = $state('unless-stopped');
  let modifyCmd = $state('');
  let modifyEntrypoint = $state('');
  let modifyLoading = $state(false);

  // Command & logging
  let modifyWorkingDir = $state('');
  let modifyUser = $state('');
  let modifyTty = $state(false);
  let modifyInteractive = $state(false);
  let modifyLogDriver = $state('');
  let modifyLogOpts = $state<{ key: string, value: string }[]>([]);

  // Network
  let modifyHostname = $state('');
  let modifyDns = $state<string[]>([]);

  // Runtime & resources (memory/shm as human-readable strings, e.g. "512m"; cpus as decimal string)
  let modifyMemory = $state('');
  let modifyMemoryReservation = $state('');
  let modifyCpus = $state('');
  let modifyShmSize = $state('');
  let modifyPrivileged = $state(false);

  // Capabilities
  const CAP_LIST = [
    'AUDIT_WRITE', 'CHOWN', 'DAC_OVERRIDE', 'FOWNER', 'FSETID', 'KILL', 'MKNOD',
    'NET_ADMIN', 'NET_BIND_SERVICE', 'NET_RAW', 'SETFCAP', 'SETGID', 'SETPCAP',
    'SETUID', 'SYS_ADMIN', 'SYS_CHROOT', 'SYS_MODULE', 'SYS_NICE', 'SYS_PTRACE',
    'SYS_TIME', 'SYS_RESOURCE', 'DAC_READ_SEARCH', 'IPC_LOCK', 'MAC_ADMIN',
  ];
  let modifyCapAdd = $state<string[]>([]);
  let modifyCapDrop = $state<string[]>([]);

  // Inline edit view mode
  let detailMode = $state<'view' | 'edit'>('view');
  type EditSection = 'command' | 'volumes' | 'network' | 'env' | 'labels' | 'restart' | 'runtime' | 'caps';
  let editSection = $state<EditSection>('command');
  let modifyIsCompose = $state(false);

  // Compose Stack network modify state
  let showComposeNetworkModal = $state(false);
  let composeNetworkProject = $state<any>(null);
  let composeSelectedNetwork = $state('');
  let composeNetworkLoading = $state(false);

  // Docker Stats tab state
  let statsList = $state<any[]>([]);
  let autoRefreshStats = $state(true);
  let refreshIntervalSeconds = $state(3);
  let statsSearchQuery = $state('');
  let statsLoading = $state(false);
  let isFetchingStats = false;
  let statsInterval: any = null;
  let statsSort = $state<SortState<string>>({ column: 'name', direction: 'asc' });

  function parseByteValue(valStr: string): number {
    if (!valStr) return 0;
    const clean = valStr.trim().toLowerCase();
    const match = clean.match(/^([0-9.]+)\s*([a-z]*)/);
    if (!match) return 0;
    const num = parseFloat(match[1]) || 0;
    const unit = match[2];
    if (unit.startsWith('t')) return num * 1024 * 1024 * 1024 * 1024;
    if (unit.startsWith('g')) return num * 1024 * 1024 * 1024;
    if (unit.startsWith('m')) return num * 1024 * 1024;
    if (unit.startsWith('k')) return num * 1024;
    return num;
  }

  function getSortedStats() {
    const filtered = statsList.filter(s =>
      s.Name.toLowerCase().includes(statsSearchQuery.toLowerCase()) ||
      s.Container.toLowerCase().includes(statsSearchQuery.toLowerCase())
    );
    return applySort(filtered, statsSort, {
      name: (s) => s.Name || '',
      cpu: (s) => s.cpuPercent || 0.0,
      memory: (s) => s.memPercent || 0.0,
      net: (s) => parseByteValue(s.NetIO.split('/')[0]) || 0,
      block: (s) => parseByteValue(s.BlockIO.split('/')[0]) || 0,
      pids: (s) => parseInt(s.PIDs) || 0
    });
  }

  async function loadStats() {
    if (!dockerInstalled || isFetchingStats) return;
    isFetchingStats = true;
    if (statsList.length === 0) statsLoading = true;
    try {
      const result = await execDocker("docker stats --no-stream --format '{{json .}}'");
      const lines = result.trim().split('\n');
      const parsedStats = [];
      for (const line of lines) {
        if (!line.trim()) continue;
        try {
          const obj = JSON.parse(line);
          const cpuPercent = parseFloat(obj.CPUPerc.replace('%', '')) || 0.0;
          const memPercent = parseFloat(obj.MemPerc.replace('%', '')) || 0.0;
          parsedStats.push({
            ...obj,
            cpuPercent,
            memPercent,
          });
        } catch (e) {
          // ignore
        }
      }
      statsList = parsedStats;
    } catch (err: any) {
      console.error(err);
      errorMsg = String(err);
    } finally {
      statsLoading = false;
      isFetchingStats = false;
    }
  }

  $effect(() => {
    if (dockerTab === 'stats') {
      loadStats();
      if (statsInterval) clearInterval(statsInterval);
      if (autoRefreshStats) {
        statsInterval = setInterval(() => {
          // Skip the tick while this pane is hidden (kept alive) to avoid wasted SSH calls.
          if (visible) loadStats();
        }, refreshIntervalSeconds * 1000);
      }
    } else {
      if (statsInterval) {
        clearInterval(statsInterval);
        statsInterval = null;
      }
    }
    return () => {
      if (statsInterval) {
        clearInterval(statsInterval);
        statsInterval = null;
      }
    };
  });

  // ----------------------------------------------------------
  // Sudo-aware Docker exec helper
  // ----------------------------------------------------------
  async function execDocker(cmd: string): Promise<string> {
    try {
      const result: string = await invoke('exec_custom_command', { cmd, useSudo });
      return result;
    } catch (err: any) {
      const appErr = parseAppError(err);
      const errCode = appErr?.code || '';
      const errDetails = appErr?.details || '';
      const errStr = (errDetails || errCode || String(err)).toLowerCase();
      
      if (errStr.includes('permission denied') || errStr.includes('got permission denied')) {
        // Retry with sudo
        useSudo = true;
        try {
          const result: string = await invoke('exec_custom_command', { cmd, useSudo: true });
          return result;
        } catch (err2: any) {
          if (isSudoPasswordRequired(err2)) {
            throw new Error('SUDO_PASSWORD_REQUIRED');
          }
          throw err2;
        }
      }
      if (isSudoPasswordRequired(err)) {
        throw new Error('SUDO_PASSWORD_REQUIRED');
      }
      throw err;
    }
  }

  async function handleWithSudo(action: () => Promise<void>) {
    try {
      await action();
    } catch (err: unknown) {
      if (isSudoPasswordRequired(err)) {
        pendingAction = () => handleWithSudo(action);
        showSudoModal = true;
      } else {
        errorMsg = formatInvokeError(err);
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
    } catch (err: unknown) {
      sudoError = isSudoPasswordIncorrect(err)
        ? "Incorrect sudo password. Try again."
        : formatInvokeError(err);
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
      const appErr = parseAppError(err);
      const errCode = appErr?.code || '';
      const errDetails = appErr?.details || '';
      const errStr = (errDetails || errCode || String(err)).toLowerCase();
      
      if (isSudoPasswordRequired(err) || errStr.includes('sudo_password_required')) {
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
        errorMsg = "Docker is installed but the daemon is not running. Start it with: systemctl start docker";
        isLoading = false;
        return;
      }
      errorMsg = `Docker check error: ${formatInvokeError(err)}`;
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
      if (isSudoPasswordRequired(err)) {
        pendingAction = loadContainers;
        showSudoModal = true;
      } else {
        errorMsg = `Failed to load containers: ${formatInvokeError(err)}`;
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
      successMsg = action === 'start' ? "Container started" : action === 'stop' ? "Container stopped" : action === 'restart' ? "Container restarted" : "Container removed";
      setTimeout(() => successMsg = '', 3000);
    });
  }

  async function removeContainer(id: string, name: string) {
    confirmMessage = `Remove container "${name}"? This cannot be undone.`;
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
      containerInspectData = `Inspect error: ${formatInvokeError(err)}`;
    } finally {
      inspectLoading = false;
    }
  }

  // ----------------------------------------------------------
  // Container detail view (Portainer-like)
  // ----------------------------------------------------------
  function parseInspect(insp: any) {
    const ports: { host: string; container: string; proto: string }[] = [];
    const pb = insp.HostConfig?.PortBindings || {};
    for (const k of Object.keys(pb)) {
      const [cport, proto = 'tcp'] = k.split('/');
      for (const b of (pb[k] || [])) {
        ports.push({ host: b.HostPort || '', container: cport, proto });
      }
    }
    const mounts = (insp.Mounts || []).map((m: any) => ({
      source: m.Name || m.Source || '',
      destination: m.Destination || '',
      type: m.Type || '',
      rw: m.RW !== false,
    }));
    const env = (insp.Config?.Env || []).map((e: string) => {
      const i = e.indexOf('=');
      return { key: i >= 0 ? e.slice(0, i) : e, value: i >= 0 ? e.slice(i + 1) : '' };
    });
    const labels = Object.entries(insp.Config?.Labels || {}).map(([key, value]) => ({ key, value }));
    const nets = Object.entries(insp.NetworkSettings?.Networks || {}).map(([name, n]: any) => ({
      name,
      ipAddress: n.IPAddress || '',
      gateway: n.Gateway || '',
      macAddress: n.MacAddress || '',
    }));
    const fmtDate = (d: string) =>
      d && !d.startsWith('0001') ? d.replace('T', ' ').split('.')[0] : '';
    return {
      id: insp.Id || '',
      name: (insp.Name || '').replace(/^\//, ''),
      image: insp.Config?.Image || '',
      imageId: insp.Image || '',
      state: insp.State?.Status || '',
      running: !!insp.State?.Running,
      paused: !!insp.State?.Paused,
      created: fmtDate(insp.Created || ''),
      startedAt: fmtDate(insp.State?.StartedAt || ''),
      cmd: (insp.Config?.Cmd || []).join(' '),
      entrypoint: (insp.Config?.Entrypoint || []).join(' '),
      env,
      labels,
      ports,
      mounts,
      networks: nets,
      restartPolicy: insp.HostConfig?.RestartPolicy?.Name || 'no',
    };
  }

  async function viewContainer(id: string, name: string) {
    detailContainerId = id;
    detailContainerName = name;
    detailLoading = true;
    detailData = null;
    detailRenaming = false;
    detailJoinNetwork = '';
    detailShowRaw = false;
    if (networks.length === 0) loadNetworks();
    try {
      const out = await execDocker(`docker inspect ${id}`);
      detailRawData = JSON.stringify(JSON.parse(out), null, 2);
      const insp = JSON.parse(out)[0];
      detailData = parseInspect(insp);
      detailRestartPolicy = detailData.restartPolicy;
      detailNewName = detailData.name;
    } catch (err: any) {
      if (isSudoPasswordRequired(err)) {
        pendingAction = () => viewContainer(id, name);
        showSudoModal = true;
      } else {
        errorMsg = `Inspect error: ${formatInvokeError(err)}`;
        detailContainerId = '';
      }
    } finally {
      detailLoading = false;
    }
  }

  function closeDetail() {
    detailContainerId = '';
    detailData = null;
    detailMode = 'view';
  }

  // Called by the workspace when the Docker tab is re-selected while already active.
  // If a container detail is open, return to the container list.
  export function onTabReselect() {
    if (dockerTab === 'containers' && detailContainerId) {
      closeDetail();
    }
  }

  async function detailAction(action: string) {
    await handleWithSudo(async () => {
      await execDocker(`docker ${action} ${detailContainerId}`);
      await loadContainers();
      await viewContainer(detailContainerId, detailContainerName);
    });
  }

  function detailRemove() {
    confirmMessage = `Remove container "${detailContainerName}"? This cannot be undone.`;
    confirmAction = async () => {
      await handleWithSudo(async () => {
        await execDocker(`docker rm -f ${detailContainerId}`);
        showConfirmModal = false;
        closeDetail();
        await loadContainers();
        successMsg = 'Container removed';
        setTimeout(() => (successMsg = ''), 3000);
      });
    };
    showConfirmModal = true;
  }

  async function renameContainer() {
    const newName = detailNewName.trim();
    if (!newName || newName === detailData?.name) {
      detailRenaming = false;
      return;
    }
    if (!/^[a-zA-Z0-9][a-zA-Z0-9_.-]*$/.test(newName)) {
      errorMsg = 'Invalid container name. Use letters, digits, and _ . - only.';
      return;
    }
    await handleWithSudo(async () => {
      await execDocker(`docker rename ${detailContainerId} ${newName}`);
      detailContainerName = newName;
      detailRenaming = false;
      await loadContainers();
      await viewContainer(detailContainerId, newName);
      successMsg = 'Container renamed';
      setTimeout(() => (successMsg = ''), 3000);
    });
  }

  async function updateDetailRestartPolicy() {
    await handleWithSudo(async () => {
      await execDocker(`docker update --restart=${detailRestartPolicy} ${detailContainerId}`);
      await viewContainer(detailContainerId, detailContainerName);
      successMsg = 'Restart policy updated';
      setTimeout(() => (successMsg = ''), 3000);
    });
  }

  async function joinDetailNetwork() {
    const net = detailJoinNetwork;
    if (!net) return;
    await handleWithSudo(async () => {
      await execDocker(`docker network connect ${net} ${detailContainerId}`);
      detailJoinNetwork = '';
      await viewContainer(detailContainerId, detailContainerName);
      successMsg = 'Joined network';
      setTimeout(() => (successMsg = ''), 3000);
    });
  }

  async function leaveDetailNetwork(net: string) {
    await handleWithSudo(async () => {
      await execDocker(`docker network disconnect ${net} ${detailContainerId}`);
      await viewContainer(detailContainerId, detailContainerName);
      successMsg = 'Left network';
      setTimeout(() => (successMsg = ''), 3000);
    });
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
      logLines = [`Failed to start log streaming: ${formatInvokeError(err)}`];
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
      if (isSudoPasswordRequired(err)) {
        pendingAction = runExec;
        showSudoModal = true;
      } else {
        execOutput = `Error: ${formatInvokeError(err)}`;
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
      if (isSudoPasswordRequired(err)) {
        pendingAction = loadImages;
        showSudoModal = true;
      } else {
        errorMsg = `Failed to load images: ${formatInvokeError(err)}`;
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
    pullProgress = 'Pulling…';
    try {
      const output = await execDocker(`docker pull ${pullImageName.trim()}`);
      pullProgress = output;
      await loadImages();
      successMsg = `Image "${pullImageName}" pulled successfully`;
      setTimeout(() => successMsg = '', 3000);
      setTimeout(() => { showPullModal = false; pullImageName = ''; pullProgress = ''; }, 1500);
    } catch (err: any) {
      if (isSudoPasswordRequired(err)) {
        pendingAction = pullImage;
        showSudoModal = true;
      } else {
        pullProgress = `Pull error: ${formatInvokeError(err)}`;
      }
    } finally {
      isPulling = false;
    }
  }

  async function removeImage(id: string, name: string) {
    confirmMessage = `Remove image "${name}"?`;
    confirmAction = async () => {
      await handleWithSudo(async () => {
        isLoading = true;
        await execDocker(`docker rmi ${id}`);
        await loadImages();
        showConfirmModal = false;
        successMsg = "Image removed";
        setTimeout(() => successMsg = '', 3000);
      });
    };
    showConfirmModal = true;
  }

  async function pruneImages() {
    confirmMessage = "Remove all unused images? This will free disk space.";
    confirmAction = async () => {
      await handleWithSudo(async () => {
        isLoading = true;
        const output = await execDocker('docker image prune -f');
        await loadImages();
        showConfirmModal = false;
        successMsg = `Unused images removed. ${output.trim().split('\n').pop() || ''}`;
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
      if (isSudoPasswordRequired(err)) {
        pendingAction = loadNetworks;
        showSudoModal = true;
      } else {
        errorMsg = `Failed to load networks: ${formatInvokeError(err)}`;
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
      networkInspectData = `Inspect error: ${formatInvokeError(err)}`;
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
      successMsg = "Network created";
      setTimeout(() => successMsg = '', 3000);
    });
  }

  async function removeNetwork(id: string, name: string) {
    confirmMessage = `Remove network "${name}"?`;
    confirmAction = async () => {
      await handleWithSudo(async () => {
        isLoading = true;
        await execDocker(`docker network rm ${id}`);
        await loadNetworks();
        showConfirmModal = false;
        successMsg = "Network removed";
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
      if (isSudoPasswordRequired(err)) {
        pendingAction = loadComposeProjects;
        showSudoModal = true;
      } else {
        composeProjects = [];
        const appErr = parseAppError(err);
        const errCode = appErr?.code || '';
        const errDetails = appErr?.details || '';
        const errStr = (errDetails || errCode || String(err)).toLowerCase();
        if (!errStr.includes('not found')) {
          errorMsg = `Failed to load Compose projects: ${formatInvokeError(err)}`;
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
        successMsg = `Compose ${action} — done`;
        setTimeout(() => successMsg = '', 3000);
      } catch (err: any) {
        errorMsg = `Compose error: ${formatInvokeError(err)}`;
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
      errorMsg = `Directory browser error: ${formatInvokeError(err)}`;
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
        .filter((e: any) => e.is_dir || e.file_type === 'directory');
      dirPickerPath = path;
    } catch (err: any) {
      errorMsg = `Directory load error: ${formatInvokeError(err)}`;
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
# Created by Jarvis Server Manager

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
      successMsg = `Project "${newProjectFolder.trim() || 'new'}" created in ${projectPath}`;
      setTimeout(() => successMsg = '', 4000);
    } catch (err: any) {
      errorMsg = `Compose error: ${formatInvokeError(err)}`;
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
      composeEditorContent = '# ' + `File read error: ${formatInvokeError(err)}`;
    }
  }

  async function saveComposeFile() {
    if (!composeEditorInstance) return;
    composeEditorSaving = true;
    const content = composeEditorInstance.getValue();
    try {
      await invoke('sftp_write', { path: composeEditorPath, content });
      successMsg = 'File saved';
      setTimeout(() => successMsg = '', 3000);
      closeComposeEditor();
    } catch (err: any) {
      errorMsg = `Compose save error: ${formatInvokeError(err)}`;
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
      composeLogs = output.trim() ? output.split('\n') : ["(no data)"];
    } catch (err: any) {
      if (isSudoPasswordRequired(err)) {
        pendingAction = () => openComposeLogs(name, configFiles);
        showSudoModal = true;
      } else {
        composeLogs = [`Log download error: ${formatInvokeError(err)}`];
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
        if (data.includes("Error response from daemon") || data.includes('[Pull error') || data.includes('Error response from daemon')) {
          composePullStatus = 'error';
        }
      });
      await invoke('start_compose_pull', { configFile: filePath, useSudo });
    } catch (err: any) {
      if (isSudoPasswordRequired(err)) {
        pendingAction = () => openComposePull(name, configFiles);
        showSudoModal = true;
        showComposePullModal = false;
      } else {
        composePullStatus = 'error';
        composePullLogs = [...composePullLogs, `Pull error: ${formatInvokeError(err)}`];
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
          composePullLayers[firstWord] = { service: firstWord, status: 'Pulling', percent: 0 };
        }
        composePullLayers[firstWord].percent = percent;
        composePullLayers[firstWord].status = cleanLine.includes('Extracting') ? 'Extracting' : 'Pulling';
      } else if (cleanLine.includes('Pull complete') || cleanLine.includes('Already exists')) {
        const firstWord = cleanLine.split(/\s+/)[0] || 'layer';
        if (!composePullLayers[firstWord]) {
          composePullLayers[firstWord] = { service: firstWord, status: "Done", percent: 100 };
        } else {
          composePullLayers[firstWord].percent = 100;
          composePullLayers[firstWord].status = "Done";
        }
      }
    }

    const keys = Object.keys(composePullLayers);
    if (keys.length > 0) {
      const total = keys.reduce((acc, k) => acc + composePullLayers[k].percent, 0);
      composePullProgress = Math.min(Math.round(total / keys.length), 100);
    }
    
    if (data.includes("Error response from daemon") || data.includes('[Pull error') || data.includes('Error')) {
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
          message: e.message || "YAML syntax error",
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
          message: e.message || "TOML syntax error",
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
      confirmMessage = `Remove container "${String(selectedContainers.length)}"? This cannot be undone.`;
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
    confirmMessage = `Remove image "${String(selectedImages.length)}"?`;
    confirmAction = async () => {
      await handleWithSudo(async () => {
        isLoading = true;
        try {
          await execDocker(`docker rmi ${ids}`);
        } catch (e: any) {
          errorMsg = `Prune error: ${formatInvokeError(e)}`;
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
      errorMsg = `Network removal error: $built-in`;
      return;
    }
    confirmMessage = `Remove network "${String(selectedNetworks.filter(n => n !== 'bridge' && n !== 'host' && n !== 'none').length)}"?`;
    confirmAction = async () => {
      await handleWithSudo(async () => {
        isLoading = true;
        try {
          await execDocker(`docker network rm ${safeIds}`);
        } catch (e: any) {
          errorMsg = `Network removal error: ${formatInvokeError(e)}`;
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
            errorMsg += `Compose error: ${name}: ${formatInvokeError(e)}\n`;
          }
        }
      }
      selectedCompose = [];
      await loadComposeProjects();
      isLoading = false;
      if (!errorMsg) {
        successMsg = `Compose ${action} — done`;
        setTimeout(() => successMsg = '', 3000);
      }
    };

    if (action === 'down') {
      confirmMessage = "Stop" + ' (' + selectedCompose.length + ')?';
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
            errorMsg += `Pull error: ${name}: ${formatInvokeError(e)}\n`;
          }
        }
      }
      selectedCompose = [];
      await loadComposeProjects();
      await loadImages();
      isLoading = false;
      if (!errorMsg) {
        successMsg = `Image "$compose" pulled successfully`;
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
      if (isSudoPasswordRequired(err)) {
        pendingAction = loadVolumes;
        showSudoModal = true;
      } else {
        errorMsg = `Failed to load volumes: ${formatInvokeError(err)}`;
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
      volumeInspectData = `Inspect error: ${formatInvokeError(err)}`;
    } finally {
      volumeInspectLoading = false;
    }
  }

  async function removeVolume(name: string) {
    confirmMessage = `Remove volume "${name}"?`;
    confirmAction = async () => {
      await handleWithSudo(async () => {
        isLoading = true;
        await execDocker(`docker volume rm ${name}`);
        await loadVolumes();
        showConfirmModal = false;
        successMsg = "Volume removed";
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
      browserErrorMsg = `Cannot read volume path: ${formatInvokeError(err)}`;
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
      
      browserEntries = parsed;
    } catch (err: any) {
      browserErrorMsg = `Folder load error: ${formatInvokeError(err)}`;
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

            // Initial syntax check
            browserSyntaxError = validateContent(monaco, browserEditorInstance.getModel(), name);

            browserEditorInstance.onDidChangeModelContent(() => {
              browserSyntaxError = validateContent(monaco, browserEditorInstance.getModel(), name);
            });
          });
        }
      }, 100);
    } catch (err: any) {
      browserErrorMsg = `File read error: ${formatInvokeError(err)}`;
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
      
      successMsg = "File saved successfully";
      setTimeout(() => successMsg = '', 3000);
      closeVolumeEditor();
      await loadVolumeDirectory();
    } catch (err: any) {
      browserErrorMsg = `Compose save error: ${formatInvokeError(err)}`;
    } finally {
      browserEditorSaving = false;
    }
  }

  function closeVolumeEditor() {
    browserEditingFile = null;
    browserSyntaxError = null;
    if (browserEditorInstance) {
      browserEditorInstance.dispose();
      browserEditorInstance = null;
    }
  }

  // ----------------------------------------------------------
  // Container Recreation (Portainer-like inline editor)
  // ----------------------------------------------------------

  // Convert a byte count to a compact human string for editing (e.g. 536870912 -> "512m").
  function bytesToHuman(bytes: number): string {
    if (!bytes || bytes <= 0) return '';
    const g = bytes / (1024 * 1024 * 1024);
    if (Number.isInteger(g)) return `${g}g`;
    const m = bytes / (1024 * 1024);
    if (Number.isInteger(m)) return `${m}m`;
    const k = bytes / 1024;
    if (Number.isInteger(k)) return `${k}k`;
    return String(bytes);
  }

  // Validate a human size like "512m" / "2g" used directly as a docker flag value.
  function isValidSize(s: string): boolean {
    return /^\d+(\.\d+)?\s*[bkmg]?$/i.test(s.trim());
  }

  async function openModifyContainer(id: string, section: EditSection = 'command') {
    modifyContainerId = id;
    modifyLoading = true;
    modifyPorts = [];
    modifyVolumes = [];
    modifyEnv = [];
    modifyLabels = [];
    modifyNetworks = [];
    modifyCmd = '';
    modifyEntrypoint = '';
    modifyWorkingDir = '';
    modifyUser = '';
    modifyTty = false;
    modifyInteractive = false;
    modifyLogDriver = '';
    modifyLogOpts = [];
    modifyHostname = '';
    modifyDns = [];
    modifyMemory = '';
    modifyMemoryReservation = '';
    modifyCpus = '';
    modifyShmSize = '';
    modifyPrivileged = false;
    modifyCapAdd = [];
    modifyCapDrop = [];
    modifyIsCompose = false;
    editSection = section;

    try {
      const inspectOut = await execDocker(`docker inspect ${id}`);
      const inspect = JSON.parse(inspectOut)[0];
      const cfg = inspect.Config || {};
      const host = inspect.HostConfig || {};

      modifyName = (inspect.Name || '').replace(/^\//, '');
      modifyImage = cfg.Image || '';

      const portBindings = host.PortBindings || {};
      for (const cPortProto of Object.keys(portBindings)) {
        const bindings = portBindings[cPortProto] || [];
        if (bindings.length > 0) {
          const parts = cPortProto.split('/');
          const container = parts[0];
          const proto = (parts[1] || 'tcp') as 'tcp' | 'udp';
          const hostPort = bindings[0].HostPort || '';
          modifyPorts.push({ host: hostPort, container, proto });
        }
      }

      const binds = host.Binds || [];
      for (const bind of binds) {
        const parts = bind.split(':');
        if (parts.length >= 2) {
          modifyVolumes.push({ host: parts[0], container: parts[1], ro: parts[2] === 'ro' });
        }
      }

      for (const env of (cfg.Env || [])) {
        const idx = env.indexOf('=');
        if (idx !== -1) {
          modifyEnv.push({ key: env.substring(0, idx), value: env.substring(idx + 1) });
        }
      }

      modifyLabels = Object.entries(cfg.Labels || {}).map(([key, value]) => ({ key, value: String(value) }));
      modifyIsCompose = !!(cfg.Labels && cfg.Labels['com.docker.compose.project']);

      modifyNetworks = Object.keys(inspect.NetworkSettings?.Networks || {});
      if (modifyNetworks.length === 0) modifyNetworks = ['bridge'];
      if (networks.length === 0) loadNetworks();
      modifyRestartPolicy = host.RestartPolicy?.Name || 'no';

      modifyCmd = (cfg.Cmd || []).join(' ');
      modifyEntrypoint = (cfg.Entrypoint || []).join(' ');

      // Command & logging
      modifyWorkingDir = cfg.WorkingDir || '';
      modifyUser = cfg.User || '';
      modifyTty = !!cfg.Tty;
      modifyInteractive = !!cfg.OpenStdin;
      modifyLogDriver = host.LogConfig?.Type || '';
      modifyLogOpts = Object.entries(host.LogConfig?.Config || {}).map(([key, value]) => ({ key, value: String(value) }));

      // Network extras
      modifyHostname = cfg.Hostname || '';
      modifyDns = [...(host.Dns || [])];

      // Runtime & resources
      modifyMemory = bytesToHuman(host.Memory || 0);
      modifyMemoryReservation = bytesToHuman(host.MemoryReservation || 0);
      modifyCpus = host.NanoCpus ? String(host.NanoCpus / 1e9) : '';
      modifyShmSize = bytesToHuman(host.ShmSize || 0);
      modifyPrivileged = !!host.Privileged;

      // Capabilities
      modifyCapAdd = [...(host.CapAdd || [])];
      modifyCapDrop = [...(host.CapDrop || [])];

      detailMode = 'edit';
    } catch (err: any) {
      if (isSudoPasswordRequired(err)) {
        pendingAction = () => openModifyContainer(id, section);
        showSudoModal = true;
      } else {
        errorMsg = `Inspect error: ${formatInvokeError(err)}`;
      }
    } finally {
      modifyLoading = false;
    }
  }

  function cancelEdit() {
    detailMode = 'view';
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
  function addModifyLabel() {
    modifyLabels = [...modifyLabels, { key: '', value: '' }];
  }
  function removeModifyLabel(idx: number) {
    modifyLabels = modifyLabels.filter((_, i) => i !== idx);
  }
  function addModifyLogOpt() {
    modifyLogOpts = [...modifyLogOpts, { key: '', value: '' }];
  }
  function removeModifyLogOpt(idx: number) {
    modifyLogOpts = modifyLogOpts.filter((_, i) => i !== idx);
  }
  function addModifyDns() {
    modifyDns = [...modifyDns, ''];
  }
  function removeModifyDns(idx: number) {
    modifyDns = modifyDns.filter((_, i) => i !== idx);
  }
  function toggleCap(cap: string, list: 'add' | 'drop') {
    if (list === 'add') {
      modifyCapAdd = modifyCapAdd.includes(cap)
        ? modifyCapAdd.filter(c => c !== cap)
        : [...modifyCapAdd, cap];
    } else {
      modifyCapDrop = modifyCapDrop.includes(cap)
        ? modifyCapDrop.filter(c => c !== cap)
        : [...modifyCapDrop, cap];
    }
  }

  // Single-quote-escape a value for safe inclusion in a shell command.
  function sq(v: string): string {
    return `'${v.replace(/'/g, "'\\''")}'`;
  }

  async function saveModifiedContainer() {
    if (!modifyName.trim() || !modifyImage.trim()) {
      errorMsg = 'Name and image are required.';
      return;
    }
    if (modifyMemory && !isValidSize(modifyMemory)) { errorMsg = 'Invalid memory limit (e.g. 512m, 2g).'; return; }
    if (modifyMemoryReservation && !isValidSize(modifyMemoryReservation)) { errorMsg = 'Invalid memory reservation (e.g. 256m).'; return; }
    if (modifyShmSize && !isValidSize(modifyShmSize)) { errorMsg = 'Invalid shm size (e.g. 64m).'; return; }
    if (modifyCpus && !/^\d+(\.\d+)?$/.test(modifyCpus.trim())) { errorMsg = 'Invalid CPU limit (e.g. 0.5, 2).'; return; }

    modifyLoading = true;
    errorMsg = '';

    let runCmd = `docker run -d --name ${sq(modifyName.trim())}`;

    if (modifyRestartPolicy) runCmd += ` --restart ${modifyRestartPolicy}`;
    if (modifyHostname.trim()) runCmd += ` --hostname ${sq(modifyHostname.trim())}`;
    if (modifyWorkingDir.trim()) runCmd += ` -w ${sq(modifyWorkingDir.trim())}`;
    if (modifyUser.trim()) runCmd += ` -u ${sq(modifyUser.trim())}`;
    if (modifyTty) runCmd += ` -t`;
    if (modifyInteractive) runCmd += ` -i`;
    if (modifyPrivileged) runCmd += ` --privileged`;

    if (modifyLogDriver.trim()) {
      runCmd += ` --log-driver ${sq(modifyLogDriver.trim())}`;
      for (const o of modifyLogOpts) {
        if (o.key.trim()) runCmd += ` --log-opt ${sq(`${o.key.trim()}=${o.value}`)}`;
      }
    }

    for (const p of modifyPorts) {
      if (p.host && p.container) runCmd += ` -p ${p.host}:${p.container}/${p.proto}`;
    }
    for (const v of modifyVolumes) {
      if (v.host && v.container) runCmd += ` -v ${sq(`${v.host}:${v.container}${v.ro ? ':ro' : ''}`)}`;
    }
    for (const e of modifyEnv) {
      if (e.key.trim()) runCmd += ` -e ${sq(`${e.key.trim()}=${e.value}`)}`;
    }
    for (const l of modifyLabels) {
      if (l.key.trim()) runCmd += ` --label ${sq(`${l.key.trim()}=${l.value}`)}`;
    }
    for (const d of modifyDns) {
      if (d.trim()) runCmd += ` --dns ${sq(d.trim())}`;
    }

    if (modifyMemory.trim()) runCmd += ` --memory ${modifyMemory.trim()}`;
    if (modifyMemoryReservation.trim()) runCmd += ` --memory-reservation ${modifyMemoryReservation.trim()}`;
    if (modifyCpus.trim()) runCmd += ` --cpus ${modifyCpus.trim()}`;
    if (modifyShmSize.trim()) runCmd += ` --shm-size ${modifyShmSize.trim()}`;

    for (const c of modifyCapAdd) runCmd += ` --cap-add ${c}`;
    for (const c of modifyCapDrop) runCmd += ` --cap-drop ${c}`;

    const primaryNet = modifyNetworks[0] || 'bridge';
    runCmd += ` --network ${sq(primaryNet)}`;

    if (modifyEntrypoint.trim()) runCmd += ` --entrypoint ${sq(modifyEntrypoint.trim())}`;

    runCmd += ` ${sq(modifyImage.trim())}`;

    if (modifyCmd.trim()) runCmd += ` ${modifyCmd}`;

    await handleWithSudo(async () => {
      try {
        await execDocker(`docker stop ${modifyContainerId}`);
        await execDocker(`docker rm ${modifyContainerId}`);
        const newContainerId = (await execDocker(runCmd)).trim();

        for (let i = 1; i < modifyNetworks.length; i++) {
          await execDocker(`docker network connect ${sq(modifyNetworks[i])} ${newContainerId}`);
        }

        successMsg = "Container recreated successfully!";
        setTimeout(() => successMsg = '', 3000);
        detailMode = 'view';
        closeDetail();
        await loadContainers();
      } catch (err: any) {
        errorMsg = `Recreate error: ${formatInvokeError(err)}`;
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
      
      successMsg = "Compose project network updated!";
      setTimeout(() => successMsg = '', 3000);
      showComposeNetworkModal = false;
      await loadComposeProjects();
    } catch (err: any) {
      errorMsg = `Compose network change error: ${formatInvokeError(err)}`;
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

<div class="docker-manager manager-shell fade-in">
  {#if !dockerInstalled}
    <!-- Docker not installed -->
    <div class="not-installed-card glass">
      <div class="not-installed-icon">
        <Container size={48} />
      </div>
      <h2>Docker is not installed</h2>
      <p class="not-installed-desc">
        Docker Engine was not found on this server. Install Docker to use containerization.
      </p>
      <a href="https://docs.docker.com/engine/install/" target="_blank" rel="noopener" class="install-link">
        Docker installation guide →
      </a>
    </div>
  {:else}
    <!-- Compact status bar -->
    <div class="docker-top-bar glass">
      <h1 class="page-title">Docker</h1>
      <div class="status-item">
        <Container size={14} class="status-icon" />
        <span class="status-value mono-val tabular-nums">{dockerVersion || '—'}</span>
      </div>
      <div class="stats-row">
        <div class="stat-chip running">
          <Play size={11} />
          <span class="mono-val tabular-nums">{runningCount}</span>
          <span class="stat-label">running</span>
        </div>
        <div class="stat-chip stopped">
          <Square size={11} />
          <span class="mono-val tabular-nums">{stoppedCount}</span>
          <span class="stat-label">stopped</span>
        </div>
        <div class="stat-chip neutral">
          <Box size={11} />
          <span class="mono-val tabular-nums">{totalImages}</span>
          <span class="stat-label">images</span>
        </div>
        <div class="stat-chip neutral">
          <Network size={11} />
          <span class="mono-val tabular-nums">{totalNetworks}</span>
          <span class="stat-label">networks</span>
        </div>
      </div>
    </div>

    <!-- Sub-tabs -->
    <div class="tabs-bar glass">
      <button class="tab-btn {dockerTab === 'containers' ? 'active' : ''}" onclick={() => { dockerTab = 'containers'; loadContainers(); }}>
        <Container size={16} /> Containers
      </button>
      <button class="tab-btn {dockerTab === 'images' ? 'active' : ''}" onclick={() => { dockerTab = 'images'; loadImages(); }}>
        <Box size={16} /> Images
      </button>
      <button class="tab-btn {dockerTab === 'networks' ? 'active' : ''}" onclick={() => { dockerTab = 'networks'; loadNetworks(); }}>
        <Network size={16} /> Networks
      </button>
      <button class="tab-btn {dockerTab === 'compose' ? 'active' : ''}" onclick={() => { dockerTab = 'compose'; loadComposeProjects(); }}>
        <Layers size={16} /> Compose
      </button>
      <button class="tab-btn {dockerTab === 'volumes' ? 'active' : ''}" onclick={() => { dockerTab = 'volumes'; loadVolumes(); }}>
        <Database size={16} /> Volumes
      </button>
      <button class="tab-btn {dockerTab === 'stats' ? 'active' : ''}" onclick={() => { dockerTab = 'stats'; loadStats(); }}>
        <Activity size={16} /> Live Stats
      </button>
    </div>

    <!-- Tab content -->
    <div class="tab-content">
      {#if dockerTab === 'containers' && detailContainerId}
        <!-- ======== CONTAINER DETAIL VIEW ======== -->
        <div class="detail-breadcrumb glass">
          <button class="link-btn" onclick={closeDetail}>
            <ChevronLeft size={14} /> Containers
          </button>
          <ChevronRight size={14} class="dir-chevron" />
          {#if detailMode === 'edit'}
            <button class="link-btn" onclick={cancelEdit}>{detailContainerName}</button>
            <ChevronRight size={14} class="dir-chevron" />
            <span class="breadcrumb-current">Edit</span>
          {:else}
            <span class="breadcrumb-current mono-val">{detailContainerName}</span>
          {/if}
        </div>

        {#if detailLoading && !detailData}
          <div class="loading-state glass">
            <Loader2 class="spin" size={36} />
            <p>Loading container details…</p>
          </div>
        {:else if detailData && detailMode === 'view'}
          <div class="detail-scroll">
            <!-- Actions -->
            <div class="detail-card glass">
              <div class="detail-card-header"><Activity size={16} /> Actions</div>
              <div class="detail-actions">
                <button class="secondary btn-sm" onclick={() => detailAction('start')} disabled={detailData.running}>
                  <Play size={14} /> Start
                </button>
                <button class="secondary btn-sm" onclick={() => detailAction('stop')} disabled={!detailData.running}>
                  <Square size={14} /> Stop
                </button>
                <button class="secondary btn-sm" onclick={() => detailAction('kill')} disabled={!detailData.running}>
                  <Square size={14} /> Kill
                </button>
                <button class="secondary btn-sm" onclick={() => detailAction('restart')} disabled={!detailData.running}>
                  <RotateCw size={14} /> Restart
                </button>
                <button class="secondary btn-sm" onclick={() => detailAction('pause')} disabled={!detailData.running || detailData.paused}>
                  <Pause size={14} /> Pause
                </button>
                <button class="secondary btn-sm" onclick={() => detailAction('unpause')} disabled={!detailData.paused}>
                  <Play size={14} /> Resume
                </button>
                <button class="danger btn-sm" onclick={detailRemove}>
                  <Trash2 size={14} /> Remove
                </button>
                <button class="secondary btn-sm" onclick={() => openModifyContainer(detailContainerId)}>
                  <Edit size={14} /> Edit / Recreate
                </button>
              </div>
            </div>

            <!-- Container status -->
            <div class="detail-card glass">
              <div class="detail-card-header"><Box size={16} /> Container status</div>
              <div class="detail-kv">
                <div class="kv-label">ID</div>
                <div class="kv-value mono-val">{detailData.id.substring(0, 64)}</div>

                <div class="kv-label">Name</div>
                <div class="kv-value">
                  {#if detailRenaming}
                    <div class="inline-edit">
                      <input type="text" bind:value={detailNewName} onkeydown={(e) => e.key === 'Enter' && renameContainer()} />
                      <button class="primary btn-sm" onclick={renameContainer}><Save size={13} /> Save</button>
                      <button class="secondary btn-sm" onclick={() => { detailRenaming = false; detailNewName = detailData.name; }}>Cancel</button>
                    </div>
                  {:else}
                    <span class="mono-val">{detailData.name}</span>
                    <button class="btn-action" onclick={() => detailRenaming = true} title="Rename"><Edit size={13} /></button>
                  {/if}
                </div>

                <div class="kv-label">Status</div>
                <div class="kv-value">
                  <span class="badge {getStatusBadge(detailData.state)}">{detailData.state}{detailData.paused ? ' (paused)' : ''}</span>
                </div>

                <div class="kv-label">Created</div>
                <div class="kv-value mono-val">{detailData.created || '—'}</div>

                <div class="kv-label">Start time</div>
                <div class="kv-value mono-val">{detailData.startedAt || '—'}</div>
              </div>
              <div class="detail-quick-links">
                <button class="link-btn" onclick={() => openLogs(detailContainerId, detailData.name)}><FileText size={14} /> Logs</button>
                <button class="link-btn" onclick={() => { detailShowRaw = !detailShowRaw; }}><Info size={14} /> Inspect</button>
                {#if detailData.running}
                  <button class="link-btn" onclick={() => openInteractiveShell(detailContainerId, detailData.name)}><Terminal size={14} /> Console</button>
                {/if}
              </div>
              {#if detailShowRaw}
                <pre class="inspect-json detail-raw">{detailRawData}</pre>
              {/if}
            </div>

            <!-- Container details -->
            <div class="detail-card glass">
              <div class="detail-card-header">
                <Container size={16} /> Container details
                <button class="secondary btn-sm header-edit-btn" onclick={() => openModifyContainer(detailContainerId, 'command')}><Edit size={12} /> Edit</button>
              </div>
              <div class="detail-kv">
                <div class="kv-label">Image</div>
                <div class="kv-value mono-val wrap">{detailData.image}</div>

                {#if detailData.cmd}
                  <div class="kv-label">CMD</div>
                  <div class="kv-value"><code class="code-chip">{detailData.cmd}</code></div>
                {/if}

                {#if detailData.entrypoint}
                  <div class="kv-label">ENTRYPOINT</div>
                  <div class="kv-value"><code class="code-chip">{detailData.entrypoint}</code></div>
                {/if}

                <div class="kv-label">Restart policy</div>
                <div class="kv-value">
                  <div class="inline-edit">
                    <select bind:value={detailRestartPolicy}>
                      <option value="no">no</option>
                      <option value="always">always</option>
                      <option value="unless-stopped">unless-stopped</option>
                      <option value="on-failure">on-failure</option>
                    </select>
                    <button class="secondary btn-sm" onclick={updateDetailRestartPolicy} disabled={detailRestartPolicy === detailData.restartPolicy}>
                      Update
                    </button>
                  </div>
                </div>
              </div>

              <!-- Environment variables -->
              <div class="detail-subheader">
                <span>Environment variables ({detailData.env.length})</span>
                <button class="secondary btn-sm" onclick={() => openModifyContainer(detailContainerId, 'env')}><Edit size={12} /> Edit</button>
              </div>
              {#if detailData.env.length > 0}
                <div class="detail-table-wrap">
                  <table class="data-table compact">
                    <thead><tr><th style="width: 35%;">Name</th><th>Value</th></tr></thead>
                    <tbody>
                      {#each detailData.env as e}
                        <tr><td class="mono-val">{e.key}</td><td class="mono-val wrap">{e.value}</td></tr>
                      {/each}
                    </tbody>
                  </table>
                </div>
              {:else}
                <p class="detail-empty">No environment variables</p>
              {/if}

              <!-- Ports -->
              {#if detailData.ports.length > 0}
                <div class="detail-subheader"><span>Published ports ({detailData.ports.length})</span></div>
                <div class="detail-table-wrap">
                  <table class="data-table compact">
                    <thead><tr><th>Host</th><th>Container</th><th>Protocol</th></tr></thead>
                    <tbody>
                      {#each detailData.ports as p}
                        <tr><td class="mono-val">{p.host || '—'}</td><td class="mono-val">{p.container}</td><td>{p.proto}</td></tr>
                      {/each}
                    </tbody>
                  </table>
                </div>
              {/if}

              <!-- Labels -->
              {#if detailData.labels.length > 0}
                <div class="detail-subheader">
                  <span>Labels ({detailData.labels.length})</span>
                  <button class="secondary btn-sm" onclick={() => openModifyContainer(detailContainerId, 'labels')}><Edit size={12} /> Edit</button>
                </div>
                <div class="detail-table-wrap">
                  <table class="data-table compact">
                    <thead><tr><th style="width: 40%;">Key</th><th>Value</th></tr></thead>
                    <tbody>
                      {#each detailData.labels as l}
                        <tr><td class="mono-val wrap">{l.key}</td><td class="mono-val wrap">{l.value}</td></tr>
                      {/each}
                    </tbody>
                  </table>
                </div>
              {/if}
            </div>

            <!-- Volumes -->
            <div class="detail-card glass">
              <div class="detail-card-header">
                <Database size={16} /> Volumes
                <button class="secondary btn-sm header-edit-btn" onclick={() => openModifyContainer(detailContainerId, 'volumes')}><Edit size={12} /> Edit</button>
              </div>
              {#if detailData.mounts.length > 0}
                <div class="detail-table-wrap">
                  <table class="data-table compact">
                    <thead><tr><th>Host/volume</th><th>Path in container</th><th style="width: 12%;">Mode</th></tr></thead>
                    <tbody>
                      {#each detailData.mounts as m}
                        <tr>
                          <td class="mono-val wrap">{m.source || '—'}</td>
                          <td class="mono-val wrap">{m.destination}</td>
                          <td>{m.rw ? 'rw' : 'ro'}</td>
                        </tr>
                      {/each}
                    </tbody>
                  </table>
                </div>
              {:else}
                <p class="detail-empty">No volumes mounted</p>
              {/if}
            </div>

            <!-- Connected networks -->
            <div class="detail-card glass">
              <div class="detail-card-header"><Network size={16} /> Connected networks</div>
              <div class="detail-join-row">
                <label for="detail-join-net">Join a network</label>
                <select id="detail-join-net" bind:value={detailJoinNetwork}>
                  <option value="">Select a network…</option>
                  {#each networks.filter(n => !detailData.networks.some((dn: any) => dn.name === n.Name)) as n}
                    <option value={n.Name}>{n.Name} ({n.Driver})</option>
                  {/each}
                </select>
                <button class="primary btn-sm" onclick={joinDetailNetwork} disabled={!detailJoinNetwork}>Join network</button>
              </div>
              {#if detailData.networks.length > 0}
                <div class="detail-table-wrap">
                  <table class="data-table compact">
                    <thead><tr><th>Network</th><th>IP Address</th><th>Gateway</th><th>MAC Address</th><th style="text-align: right;">Actions</th></tr></thead>
                    <tbody>
                      {#each detailData.networks as n}
                        <tr>
                          <td class="mono-val">{n.name}</td>
                          <td class="mono-val">{n.ipAddress || '—'}</td>
                          <td class="mono-val">{n.gateway || '—'}</td>
                          <td class="mono-val">{n.macAddress || '—'}</td>
                          <td style="text-align: right;">
                            <button class="danger btn-sm" onclick={() => leaveDetailNetwork(n.name)}>Leave network</button>
                          </td>
                        </tr>
                      {/each}
                    </tbody>
                  </table>
                </div>
              {:else}
                <p class="detail-empty">Not connected to any network</p>
              {/if}
            </div>
          </div>
        {:else if detailData && detailMode === 'edit'}
          <!-- ======== CONTAINER EDIT VIEW ======== -->
          <div class="detail-scroll">
            <!-- Basics -->
            <div class="detail-card glass">
              <div class="detail-card-header"><Edit size={16} /> Edit container — recreate with new settings</div>
              <div class="form-row">
                <div class="form-group">
                  <label for="edit-name">Name</label>
                  <input id="edit-name" type="text" bind:value={modifyName} />
                </div>
                <div class="form-group">
                  <label for="edit-image">Image</label>
                  <input id="edit-image" type="text" bind:value={modifyImage} class="mono-val" />
                </div>
              </div>
            </div>

            <!-- Section tabs -->
            <div class="tabs-bar glass edit-section-tabs">
              <button class="tab-btn {editSection === 'command' ? 'active' : ''}" onclick={() => editSection = 'command'}>Command &amp; logging</button>
              <button class="tab-btn {editSection === 'volumes' ? 'active' : ''}" onclick={() => editSection = 'volumes'}>Volumes</button>
              <button class="tab-btn {editSection === 'network' ? 'active' : ''}" onclick={() => editSection = 'network'}>Network</button>
              <button class="tab-btn {editSection === 'env' ? 'active' : ''}" onclick={() => editSection = 'env'}>Env</button>
              <button class="tab-btn {editSection === 'labels' ? 'active' : ''}" onclick={() => editSection = 'labels'}>Labels</button>
              <button class="tab-btn {editSection === 'restart' ? 'active' : ''}" onclick={() => editSection = 'restart'}>Restart policy</button>
              <button class="tab-btn {editSection === 'runtime' ? 'active' : ''}" onclick={() => editSection = 'runtime'}>Runtime &amp; resources</button>
              <button class="tab-btn {editSection === 'caps' ? 'active' : ''}" onclick={() => editSection = 'caps'}>Capabilities</button>
            </div>

            <div class="detail-card glass">
              {#if editSection === 'command'}
                <div class="form-group">
                  <label for="edit-cmd">Command (CMD)</label>
                  <input id="edit-cmd" type="text" bind:value={modifyCmd} class="mono-val" placeholder="e.g. nginx -g 'daemon off;'" />
                </div>
                <div class="form-group">
                  <label for="edit-entry">Entrypoint</label>
                  <input id="edit-entry" type="text" bind:value={modifyEntrypoint} class="mono-val" />
                </div>
                <div class="form-row">
                  <div class="form-group">
                    <label for="edit-wd">Working directory</label>
                    <input id="edit-wd" type="text" bind:value={modifyWorkingDir} class="mono-val" placeholder="/app" />
                  </div>
                  <div class="form-group">
                    <label for="edit-user">User</label>
                    <input id="edit-user" type="text" bind:value={modifyUser} class="mono-val" placeholder="root or 1000:1000" />
                  </div>
                </div>
                <div class="checkbox-row">
                  <label class="ro-label"><input type="checkbox" bind:checked={modifyInteractive} /> Interactive (-i)</label>
                  <label class="ro-label"><input type="checkbox" bind:checked={modifyTty} /> TTY (-t)</label>
                </div>
                <div class="form-group">
                  <label for="edit-logdriver">Log driver</label>
                  <input id="edit-logdriver" type="text" bind:value={modifyLogDriver} class="mono-val" placeholder="json-file, local, journald…" />
                </div>
                <div class="modify-section-header">
                  <span>Log options</span>
                  <button class="secondary btn-sm" onclick={addModifyLogOpt}><Plus size={12} /> Add</button>
                </div>
                {#each modifyLogOpts as opt, i}
                  <div class="modify-row">
                    <input type="text" placeholder="Key (e.g. max-size)" bind:value={opt.key} class="mono-val" />
                    <span>=</span>
                    <input type="text" placeholder="Value (e.g. 10m)" bind:value={opt.value} class="mono-val" />
                    <button class="btn-action danger-text" onclick={() => removeModifyLogOpt(i)}><X size={14} /></button>
                  </div>
                {/each}
                {#if modifyLogOpts.length === 0}
                  <div class="modify-row empty-example">
                    <input type="text" placeholder="Key (e.g. max-size)" class="mono-val" disabled />
                    <span>=</span>
                    <input type="text" placeholder="Value (e.g. 10m)" class="mono-val" disabled />
                  </div>
                {/if}

              {:else if editSection === 'volumes'}
                <div class="modify-section-header">
                  <span>Volumes / mounts</span>
                  <button class="secondary btn-sm" onclick={addModifyVolume}><Plus size={12} /> Add</button>
                </div>
                {#each modifyVolumes as vol, i}
                  <div class="modify-row">
                    <PathAutocomplete placeholder="Host path" bind:value={vol.host} class="mono-val" onlyDirs={true} />
                    <span>→</span>
                    <input type="text" placeholder="Container path" bind:value={vol.container} class="mono-val" />
                    <label class="ro-label"><input type="checkbox" bind:checked={vol.ro} /> RO</label>
                    <button class="btn-action danger-text" onclick={() => removeModifyVolume(i)}><X size={14} /></button>
                  </div>
                {/each}
                {#if modifyVolumes.length === 0}
                  <div class="modify-row empty-example">
                    <input type="text" placeholder="Host path (e.g. /data/myapp)" class="mono-val" disabled />
                    <span>→</span>
                    <input type="text" placeholder="Container path (e.g. /app/data)" class="mono-val" disabled />
                    <label class="ro-label"><input type="checkbox" disabled /> RO</label>
                  </div>
                {/if}

              {:else if editSection === 'network'}
                <div class="form-group">
                  <label for="edit-net">Primary network</label>
                  <select id="edit-net" bind:value={modifyNetworks[0]}>
                    {#if !networks.some(n => n.Name === modifyNetworks[0])}
                      <option value={modifyNetworks[0]}>{modifyNetworks[0]}</option>
                    {/if}
                    {#each networks as n}
                      <option value={n.Name}>{n.Name} ({n.Driver})</option>
                    {/each}
                  </select>
                </div>
                {#if modifyNetworks.length > 1}
                  <div class="form-group">
                    <span class="form-sublabel">Additional networks (reconnected after recreate)</span>
                    <div class="network-tags">
                      {#each modifyNetworks.slice(1) as net}
                        <span class="badge warning">{net}</span>
                      {/each}
                    </div>
                  </div>
                {/if}
                <div class="form-group">
                  <label for="edit-hostname">Hostname</label>
                  <input id="edit-hostname" type="text" bind:value={modifyHostname} class="mono-val" />
                </div>
                <div class="modify-section-header" style="margin-top: 16px;">
                  <span>Port mapping</span>
                  <button class="secondary btn-sm" onclick={addModifyPort}><Plus size={12} /> Add</button>
                </div>
                {#each modifyPorts as port, i}
                  <div class="modify-row">
                    <input type="text" placeholder="Host port (e.g. 8080)" bind:value={port.host} class="mono-val" />
                    <span>→</span>
                    <input type="text" placeholder="Container port (e.g. 80)" bind:value={port.container} class="mono-val" />
                    <select bind:value={port.proto}>
                      <option value="tcp">TCP</option>
                      <option value="udp">UDP</option>
                    </select>
                    <button class="btn-action danger-text" onclick={() => removeModifyPort(i)}><X size={14} /></button>
                  </div>
                {/each}
                {#if modifyPorts.length === 0}
                  <div class="modify-row empty-example">
                    <input type="text" placeholder="Host port (e.g. 8080)" class="mono-val" disabled />
                    <span>→</span>
                    <input type="text" placeholder="Container port (e.g. 80)" class="mono-val" disabled />
                    <select disabled><option>TCP</option></select>
                  </div>
                {/if}
                <div class="modify-section-header" style="margin-top: 16px;">
                  <span>DNS servers</span>
                  <button class="secondary btn-sm" onclick={addModifyDns}><Plus size={12} /> Add</button>
                </div>
                {#each modifyDns as _, i}
                  <div class="modify-row">
                    <input type="text" placeholder="e.g. 1.1.1.1" bind:value={modifyDns[i]} class="mono-val" />
                    <button class="btn-action danger-text" onclick={() => removeModifyDns(i)}><X size={14} /></button>
                  </div>
                {/each}
                {#if modifyDns.length === 0}
                  <div class="modify-row empty-example">
                    <input type="text" placeholder="e.g. 1.1.1.1" class="mono-val" disabled />
                  </div>
                {/if}

              {:else if editSection === 'env'}
                <div class="modify-section-header">
                  <span>Environment variables</span>
                  <button class="secondary btn-sm" onclick={addModifyEnv}><Plus size={12} /> Add</button>
                </div>
                {#each modifyEnv as env, i}
                  <div class="modify-row">
                    <input type="text" placeholder="Key" bind:value={env.key} class="mono-val" />
                    <span>=</span>
                    <input type="text" placeholder="Value" bind:value={env.value} class="mono-val" />
                    <button class="btn-action danger-text" onclick={() => removeModifyEnv(i)}><X size={14} /></button>
                  </div>
                {/each}
                {#if modifyEnv.length === 0}
                  <div class="modify-row empty-example">
                    <input type="text" placeholder="KEY" class="mono-val" disabled />
                    <span>=</span>
                    <input type="text" placeholder="value" class="mono-val" disabled />
                  </div>
                {/if}

              {:else if editSection === 'labels'}
                <div class="modify-section-header">
                  <span>Labels</span>
                  <button class="secondary btn-sm" onclick={addModifyLabel}><Plus size={12} /> Add</button>
                </div>
                {#each modifyLabels as label, i}
                  <div class="modify-row">
                    <input type="text" placeholder="Key" bind:value={label.key} class="mono-val" />
                    <span>=</span>
                    <input type="text" placeholder="Value" bind:value={label.value} class="mono-val" />
                    <button class="btn-action danger-text" onclick={() => removeModifyLabel(i)}><X size={14} /></button>
                  </div>
                {/each}
                {#if modifyLabels.length === 0}
                  <div class="modify-row empty-example">
                    <input type="text" placeholder="com.example.key" class="mono-val" disabled />
                    <span>=</span>
                    <input type="text" placeholder="value" class="mono-val" disabled />
                  </div>
                {/if}

              {:else if editSection === 'restart'}
                <div class="form-group">
                  <label for="edit-restart">Restart policy</label>
                  <select id="edit-restart" bind:value={modifyRestartPolicy}>
                    <option value="no">no</option>
                    <option value="always">always</option>
                    <option value="unless-stopped">unless-stopped</option>
                    <option value="on-failure">on-failure</option>
                  </select>
                </div>

              {:else if editSection === 'runtime'}
                <div class="form-row">
                  <div class="form-group">
                    <label for="edit-mem">Memory limit</label>
                    <input id="edit-mem" type="text" bind:value={modifyMemory} class="mono-val" placeholder="e.g. 512m, 2g (empty = unlimited)" />
                  </div>
                  <div class="form-group">
                    <label for="edit-memres">Memory reservation</label>
                    <input id="edit-memres" type="text" bind:value={modifyMemoryReservation} class="mono-val" placeholder="e.g. 256m" />
                  </div>
                </div>
                <div class="form-row">
                  <div class="form-group">
                    <label for="edit-cpus">CPU limit (cores)</label>
                    <input id="edit-cpus" type="text" bind:value={modifyCpus} class="mono-val" placeholder="e.g. 0.5, 2" />
                  </div>
                  <div class="form-group">
                    <label for="edit-shm">Shared memory size</label>
                    <input id="edit-shm" type="text" bind:value={modifyShmSize} class="mono-val" placeholder="e.g. 64m" />
                  </div>
                </div>
                <div class="checkbox-row">
                  <label class="ro-label"><input type="checkbox" bind:checked={modifyPrivileged} /> Privileged</label>
                </div>

              {:else if editSection === 'caps'}
                <p class="detail-empty">Add or drop Linux capabilities. Leave unchecked to use the image defaults.</p>
                <div class="caps-grid">
                  <div class="caps-head"><span>Capability</span><span>Add</span><span>Drop</span></div>
                  {#each CAP_LIST as cap}
                    <div class="caps-row">
                      <span class="mono-val">{cap}</span>
                      <input type="checkbox" checked={modifyCapAdd.includes(cap)} onchange={() => toggleCap(cap, 'add')} />
                      <input type="checkbox" checked={modifyCapDrop.includes(cap)} onchange={() => toggleCap(cap, 'drop')} />
                    </div>
                  {/each}
                </div>
              {/if}
            </div>

            <!-- Footer -->
            <div class="detail-card glass edit-footer">
              {#if modifyIsCompose}
                <div class="edit-warning">
                  <AlertCircle size={14} />
                  <span>This container is managed by Docker Compose. Recreating it here turns it into a standalone container that may drift from its compose file.</span>
                </div>
              {/if}
              <div class="edit-footer-actions">
                <button class="primary" onclick={saveModifiedContainer} disabled={modifyLoading}>
                  {#if modifyLoading}
                    <Loader2 size={16} class="spin" /> Recreating…
                  {:else}
                    <RefreshCw size={16} /> Save &amp; Recreate
                  {/if}
                </button>
                <button class="secondary" onclick={cancelEdit} disabled={modifyLoading}>Cancel</button>
              </div>
            </div>
          </div>
        {/if}

      {:else if dockerTab === 'containers'}
        <!-- ======== CONTAINERS TAB ======== -->
        <div class="ops-bar glass">
          <div class="search-bar">
            <span class="search-icon-wrapper"><Search size={16} /></span>
            <input type="text" placeholder="Search containers…" bind:value={containerSearch} />
          </div>
          {#if selectedContainers.length > 0}
            <div class="bulk-actions">
              <span class="bulk-count">{`Selected: ${String(selectedContainers.length)}`}</span>
              <button class="secondary btn-sm" onclick={() => runBulkContainerAction('start')}><Play size={12} /> Start</button>
              <button class="secondary btn-sm" onclick={() => runBulkContainerAction('stop')}><Square size={12} /> Stop</button>
              <button class="secondary btn-sm" onclick={() => runBulkContainerAction('restart')}><RotateCw size={12} /> Restart</button>
              <button class="danger btn-sm" onclick={() => runBulkContainerAction('rm -f')}><Trash2 size={12} /> Remove</button>
            </div>
          {/if}
          <button class="secondary" onclick={loadContainers} disabled={isLoading}>
            <RefreshCw size={16} class={isLoading ? 'spin' : ''} /> Refresh
          </button>
        </div>

        <div class="table-container glass">
          {#if containersLoading}
            <div class="loading-state">
              <Loader2 class="spin" size={36} />
              <p>Loading containers…</p>
            </div>
          {:else}
            <table class="data-table">
              <thead>
                <tr>
                  <th style="width: 5%;"><input type="checkbox" checked={selectedContainers.length > 0 && selectedContainers.length === getFilteredContainers().length} onchange={toggleSelectAllContainers} /></th>
                  <SortableTh label="Name" column="name" activeColumn={containerSort.column} direction={containerSort.direction} onsort={(c) => containerSort = nextSort(containerSort, c)} width="20%" />
                  <SortableTh label="Image" column="image" activeColumn={containerSort.column} direction={containerSort.direction} onsort={(c) => containerSort = nextSort(containerSort, c)} width="18%" />
                  <SortableTh label="Status" column="status" activeColumn={containerSort.column} direction={containerSort.direction} onsort={(c) => containerSort = nextSort(containerSort, c)} width="12%" />
                  <SortableTh label="Ports" column="ports" activeColumn={containerSort.column} direction={containerSort.direction} onsort={(c) => containerSort = nextSort(containerSort, c)} width="15%" />
                  <SortableTh label="Created" column="created" activeColumn={containerSort.column} direction={containerSort.direction} onsort={(c) => containerSort = nextSort(containerSort, c)} width="13%" />
                  <th style="width: 17%; text-align: right;">Operations</th>
                </tr>
              </thead>
              <tbody>
                {#each getSortedContainers() as container}
                  <tr>
                    <td><input type="checkbox" checked={selectedContainers.includes(container.ID)} onchange={() => toggleSelectContainer(container.ID)} /></td>
                    <td class="mono-val">
                      <button class="container-name-link" onclick={() => viewContainer(container.ID, container.Names)} title="View details">
                        <strong>{container.Names}</strong>
                      </button>
                    </td>
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
                        <button class="btn-action danger-text" onclick={() => containerAction('stop', container.ID)} title="Stop">
                          <Square size={14} />
                        </button>
                        <button class="btn-action" onclick={() => containerAction('restart', container.ID)} title="Restart">
                          <RotateCw size={14} />
                        </button>
                      {:else}
                        <button class="btn-action success-text" onclick={() => containerAction('start', container.ID)} title="Start">
                          <Play size={14} />
                        </button>
                      {/if}
                      <button class="btn-action" onclick={() => openModifyContainer(container.ID)} title="Modify (Portainer-like)">
                        <Edit size={14} class="accent-amber-text" />
                      </button>
                      <button class="btn-action" onclick={() => inspectContainer(container.ID)} title="Inspect">
                        <Eye size={14} />
                      </button>
                      <button class="btn-action" onclick={() => openLogs(container.ID, container.Names)} title="Logs">
                        <FileText size={14} />
                      </button>
                      {#if container.State === 'running'}
                        <button class="btn-action amber-text" onclick={() => openInteractiveShell(container.ID, container.Names)} title="Interactive shell">
                          <Terminal size={14} />
                        </button>
                      {/if}
                      <button class="btn-action danger-text" onclick={() => removeContainer(container.ID, container.Names)} title="Remove">
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
                            <span class="inspect-title">{`Inspect: ${container.Names}`}</span>
                            <button class="btn-action" onclick={() => expandedContainer = ''}>
                              <X size={14} />
                            </button>
                          </div>
                          {#if inspectLoading}
                            <div class="inspect-loading"><Loader2 size={20} class="spin" /> Loading…</div>
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
                        No containers. Create one with docker run or Docker Compose.
                      {:else}
                        No results
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
            <span class="search-icon-wrapper"><Search size={16} /></span>
            <input type="text" placeholder="Search images…" bind:value={imageSearch} />
          </div>
          {#if selectedImages.length > 0}
            <div class="bulk-actions">
              <span class="bulk-count">{`Selected: ${String(selectedImages.length)}`}</span>
              <button class="danger btn-sm" onclick={runBulkImageAction}><Trash2 size={12} /> Remove selected</button>
            </div>
          {/if}
          <button class="secondary" onclick={loadImages} disabled={isLoading}>
            <RefreshCw size={16} class={isLoading ? 'spin' : ''} /> Refresh
          </button>
          <button class="secondary" onclick={pruneImages}>
            <Eraser size={16} /> Prune unused images
          </button>
          <button class="primary" onclick={() => { showPullModal = true; pullImageName = ''; pullProgress = ''; }}>
            <Download size={16} /> Pull image
          </button>
        </div>

        <div class="table-container glass">
          {#if isLoading && images.length === 0}
            <div class="loading-state">
              <RefreshCw class="spin" size={32} />
              <p>Loading images…</p>
            </div>
          {:else}
            <table class="data-table">
              <thead>
                <tr>
                  <th style="width: 5%;"><input type="checkbox" checked={selectedImages.length > 0 && selectedImages.length === getFilteredImages().length} onchange={toggleSelectAllImages} /></th>
                  <SortableTh label="Repository" column="repository" activeColumn={imageSort.column} direction={imageSort.direction} onsort={(c) => imageSort = nextSort(imageSort, c)} width="25%" />
                  <SortableTh label="Tag" column="tag" activeColumn={imageSort.column} direction={imageSort.direction} onsort={(c) => imageSort = nextSort(imageSort, c)} width="15%" />
                  <SortableTh label="ID" column="id" activeColumn={imageSort.column} direction={imageSort.direction} onsort={(c) => imageSort = nextSort(imageSort, c)} width="18%" />
                  <SortableTh label="Size" column="size" activeColumn={imageSort.column} direction={imageSort.direction} onsort={(c) => imageSort = nextSort(imageSort, c)} width="12%" />
                  <SortableTh label="Created" column="created" activeColumn={imageSort.column} direction={imageSort.direction} onsort={(c) => imageSort = nextSort(imageSort, c)} width="12%" />
                  <th style="width: 13%; text-align: right;">Operations</th>
                </tr>
              </thead>
              <tbody>
                {#each getSortedImages() as image}
                  <tr>
                    <td><input type="checkbox" checked={selectedImages.includes(image.ID)} onchange={() => toggleSelectImage(image.ID)} /></td>
                    <td class="mono-val">
                      <strong>{image.Repository}</strong>
                      {#if isImageUnused(image)}
                        <span class="badge secondary" style="margin-left: 8px; font-size: 0.7rem; padding: 2px 6px;">Unused</span>
                      {/if}
                    </td>
                    <td><span class="badge warning">{image.Tag}</span></td>
                    <td class="mono-val id-cell">{image.ID}</td>
                    <td class="mono-val">{image.Size}</td>
                    <td class="time-cell">{image.CreatedSince || image.CreatedAt || '—'}</td>
                    <td class="actions-cell">
                      {#if isImageUnused(image)}
                        <button class="btn-action danger-text" onclick={() => removeImage(image.ID, image.Repository + ':' + image.Tag)} title="Remove unused image">
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
                        No containers. Create one with docker run or Docker Compose.
                      {:else}
                        No results
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
              <span class="bulk-count">{`Selected: ${String(selectedNetworks.length)}`}</span>
              <button class="danger btn-sm" onclick={runBulkNetworkAction}><Trash2 size={12} /> Remove selected</button>
            </div>
          {/if}
          <button class="secondary" onclick={loadNetworks} disabled={isLoading}>
            <RefreshCw size={16} class={isLoading ? 'spin' : ''} /> Refresh
          </button>
          <button class="primary" onclick={() => { showCreateNetworkModal = true; newNetworkName = ''; newNetworkDriver = 'bridge'; }}>
            <Plus size={16} /> Create network
          </button>
        </div>

        <div class="table-container glass">
          {#if isLoading && networks.length === 0}
            <div class="loading-state">
              <RefreshCw class="spin" size={32} />
              <p>Loading networks…</p>
            </div>
          {:else}
            <table class="data-table">
              <thead>
                <tr>
                  <th style="width: 5%;"><input type="checkbox" checked={selectedNetworks.length > 0 && selectedNetworks.length === networks.length} onchange={toggleSelectAllNetworks} /></th>
                  <SortableTh label="Name" column="name" activeColumn={networkSort.column} direction={networkSort.direction} onsort={(c) => networkSort = nextSort(networkSort, c)} width="25%" />
                  <SortableTh label="Driver" column="driver" activeColumn={networkSort.column} direction={networkSort.direction} onsort={(c) => networkSort = nextSort(networkSort, c)} width="18%" />
                  <SortableTh label="Scope" column="scope" activeColumn={networkSort.column} direction={networkSort.direction} onsort={(c) => networkSort = nextSort(networkSort, c)} width="15%" />
                  <SortableTh label="Network ID" column="id" activeColumn={networkSort.column} direction={networkSort.direction} onsort={(c) => networkSort = nextSort(networkSort, c)} width="22%" />
                  <th style="width: 15%; text-align: right;">Operations</th>
                </tr>
              </thead>
              <tbody>
                {#each getSortedNetworks() as network}
                  <tr>
                    <td><input type="checkbox" checked={selectedNetworks.includes(network.ID || network.Name)} onchange={() => toggleSelectNetwork(network.ID || network.Name)} /></td>
                    <td class="mono-val"><strong>{network.Name}</strong></td>
                    <td><span class="badge warning">{network.Driver}</span></td>
                    <td>{network.Scope || '—'}</td>
                    <td class="mono-val id-cell">{network.ID?.substring(0, 12) || '—'}</td>
                    <td class="actions-cell">
                      <button class="btn-action" onclick={() => inspectNetwork(network.ID || network.Name)} title="Inspect">
                        <Eye size={14} />
                      </button>
                      {#if network.Name !== 'bridge' && network.Name !== 'host' && network.Name !== 'none'}
                        <button class="btn-action danger-text" onclick={() => removeNetwork(network.ID || network.Name, network.Name)} title="Remove network">
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
                            <span class="inspect-title">Network inspect: {network.Name}</span>
                            <button class="btn-action" onclick={() => expandedNetwork = ''}>
                              <X size={14} />
                            </button>
                          </div>
                          {#if networkInspectLoading}
                            <div class="inspect-loading"><Loader2 size={20} class="spin" /> Loading…</div>
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
                    <td colspan="6" class="empty-state">No Docker networks</td>
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
              <span class="bulk-count">Selected: {selectedCompose.length}</span>
              <button class="secondary btn-sm" onclick={() => runBulkComposeAction('up -d')}><Play size={12} /> Up</button>
              <button class="secondary btn-sm" onclick={() => runBulkComposeAction('down')}><Square size={12} /> Down</button>
              <button class="secondary btn-sm" onclick={() => runBulkComposeAction('restart')}><RotateCw size={12} /> Restart</button>
              <button class="secondary btn-sm" onclick={runBulkComposePull}><Download size={12} /> Pull</button>
            </div>
          {/if}
          <button class="secondary" onclick={loadComposeProjects} disabled={isLoading}>
            <RefreshCw size={16} class={isLoading ? 'spin' : ''} /> Refresh
          </button>
          <button class="primary" onclick={openDirPicker}>
            <FolderPlus size={16} /> New project
          </button>
        </div>

        <div class="table-container glass">
          {#if isLoading && composeProjects.length === 0}
            <div class="loading-state">
              <RefreshCw class="spin" size={32} />
              <p>Loading Compose projects…</p>
            </div>
          {:else}
            <table class="data-table">
              <thead>
                <tr>
                  <th style="width: 5%;"><input type="checkbox" checked={selectedCompose.length > 0 && selectedCompose.length === composeProjects.length} onchange={toggleSelectAllCompose} /></th>
                  <SortableTh label="Project name" column="name" activeColumn={composeSort.column} direction={composeSort.direction} onsort={(c) => composeSort = nextSort(composeSort, c)} width="20%" />
                  <SortableTh label="Status" column="status" activeColumn={composeSort.column} direction={composeSort.direction} onsort={(c) => composeSort = nextSort(composeSort, c)} width="12%" />
                  <SortableTh label="Config file" column="config" activeColumn={composeSort.column} direction={composeSort.direction} onsort={(c) => composeSort = nextSort(composeSort, c)} width="28%" />
                  <th style="width: 35%; text-align: right;">Operations</th>
                </tr>
              </thead>
              <tbody>
                {#each getSortedCompose() as project}
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
                      <button class="secondary btn-sm" onclick={() => openComposePull(projectName, configFiles)} title="Pull images">
                        <Download size={14} /> Pull
                      </button>
                      <button class="btn-action" onclick={() => openComposeEditor(configFiles)} title="Edit compose file">
                        <Edit size={14} />
                      </button>
                      <button class="btn-action" onclick={() => openComposeLogs(projectName, configFiles)} title="Show project logs">
                        <FileText size={14} />
                      </button>
                      <button class="btn-action" onclick={() => openChangeComposeNetwork(project)} title="Change network">
                        <Unplug size={14} />
                      </button>
                    </td>
                  </tr>
                {/each}

                {#if composeProjects.length === 0 && !isLoading}
                  <tr>
                    <td colspan="5" class="empty-state">
                      No Docker Compose projects. Create a new project with the button above.
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
            <span class="search-icon-wrapper"><Search size={16} /></span>
            <input type="text" placeholder="Search volumes…" bind:value={volumeSearch} />
          </div>
          <button class="secondary" onclick={loadVolumes} disabled={isLoading}>
            <RefreshCw size={16} class={isLoading ? 'spin' : ''} /> Refresh
          </button>
        </div>

        <div class="table-container glass">
          {#if isLoading && volumes.length === 0}
            <div class="loading-state">
              <RefreshCw class="spin" size={32} />
              <p>Loading volumes…</p>
            </div>
          {:else}
            <table class="data-table">
              <thead>
                <tr>
                  <SortableTh label="Name" column="name" activeColumn={volumeSort.column} direction={volumeSort.direction} onsort={(c) => volumeSort = nextSort(volumeSort, c)} width="30%" />
                  <SortableTh label="Driver" column="driver" activeColumn={volumeSort.column} direction={volumeSort.direction} onsort={(c) => volumeSort = nextSort(volumeSort, c)} width="20%" />
                  <th style="width: 35%; text-align: right;">Operations</th>
                </tr>
              </thead>
              <tbody>
                {#each getSortedVolumes() as volume}
                  <tr>
                    <td class="mono-val"><strong>{volume.Name}</strong></td>
                    <td><span class="badge warning">{volume.Driver || 'local'}</span></td>
                    <td class="actions-cell">
                      <button class="btn-action" onclick={() => inspectVolume(volume.Name)} title="Inspect">
                        <Eye size={14} />
                      </button>
                      <button class="btn-action" onclick={() => openVolumeBrowser(volume.Name)} title="Browse contents">
                        <FolderOpen size={14} />
                      </button>
                      <button class="btn-action danger-text" onclick={() => removeVolume(volume.Name)} title="Remove volume">
                        <Trash2 size={14} />
                      </button>
                    </td>
                  </tr>

                  {#if expandedVolume === volume.Name}
                    <tr class="inspect-row">
                      <td colspan="3">
                        <div class="inspect-card">
                          <div class="inspect-header">
                            <span class="inspect-title">Volume inspect: {volume.Name}</span>
                            <button class="btn-action" onclick={() => expandedVolume = ''}>
                              <X size={14} />
                            </button>
                          </div>
                          {#if volumeInspectLoading}
                            <div class="inspect-loading"><Loader2 size={20} class="spin" /> Loading…</div>
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
                        No Docker volumes
                      {:else}
                        No results
                      {/if}
                    </td>
                  </tr>
                {/if}
              </tbody>
            </table>
          {/if}
        </div>
      {:else if dockerTab === 'stats'}
        <!-- ======== LIVE STATS TAB ======== -->
        <div class="ops-bar glass">
          <div class="search-bar">
            <span class="search-icon-wrapper"><Search size={16} /></span>
            <input type="text" placeholder="Search containers…" bind:value={statsSearchQuery} />
          </div>
          
          <div style="display: flex; align-items: center; gap: 16px;">
            <label class="toggle-checkbox" style="display: flex; align-items: center; gap: 8px; cursor: pointer; font-size: 0.9rem;">
              <input type="checkbox" bind:checked={autoRefreshStats} />
              <span>Auto-refresh</span>
            </label>

            {#if autoRefreshStats}
              <select bind:value={refreshIntervalSeconds} style="padding: 4px 8px; border-radius: 4px; border: 1px solid var(--border-color); background: var(--surface-1); color: var(--text-primary); font-size: 0.85rem;">
                <option value={1}>1s</option>
                <option value={2}>2s</option>
                <option value={3}>3s</option>
                <option value={5}>5s</option>
                <option value={10}>10s</option>
              </select>
            {/if}

            <button class="secondary" onclick={loadStats} disabled={statsLoading}>
              <RefreshCw size={16} class={statsLoading ? 'spin' : ''} /> Refresh
            </button>
          </div>
        </div>

        <div class="table-container glass">
          {#if statsLoading && statsList.length === 0}
            <div class="loading-state">
              <RefreshCw class="spin" size={32} />
              <p>Loading…</p>
            </div>
          {:else}
            <table class="data-table">
              <thead>
                <tr>
                  <SortableTh label="Container Name" column="name" activeColumn={statsSort.column} direction={statsSort.direction} onsort={(c) => statsSort = nextSort(statsSort, c)} width="25%" />
                  <SortableTh label="CPU Usage" column="cpu" activeColumn={statsSort.column} direction={statsSort.direction} onsort={(c) => statsSort = nextSort(statsSort, c)} width="20%" />
                  <SortableTh label="Memory Usage" column="memory" activeColumn={statsSort.column} direction={statsSort.direction} onsort={(c) => statsSort = nextSort(statsSort, c)} width="25%" />
                  <SortableTh label="Net I/O" column="net" activeColumn={statsSort.column} direction={statsSort.direction} onsort={(c) => statsSort = nextSort(statsSort, c)} width="15%" />
                  <SortableTh label="Block I/O" column="block" activeColumn={statsSort.column} direction={statsSort.direction} onsort={(c) => statsSort = nextSort(statsSort, c)} width="10%" />
                  <SortableTh label="PIDs" column="pids" activeColumn={statsSort.column} direction={statsSort.direction} onsort={(c) => statsSort = nextSort(statsSort, c)} width="5%" align="right" />
                </tr>
              </thead>
              <tbody>
                {#each getSortedStats() as stat}
                  <tr>
                    <td>
                      <div style="display: flex; flex-direction: column; gap: 2px;">
                        <span class="mono-val" style="font-weight: 600;">{stat.Name}</span>
                        <span class="mono-val text-muted" style="font-size: 0.75rem;">{stat.Container}</span>
                      </div>
                    </td>
                    <td>
                      <div style="display: flex; flex-direction: column; gap: 4px; width: 100%;">
                        <div style="display: flex; justify-content: space-between; font-size: 0.8rem; font-family: monospace;">
                          <span>{stat.CPUPerc}</span>
                        </div>
                        <div style="width: 100%; height: 6px; background: rgba(255,255,255,0.05); border-radius: 3px; overflow: hidden;">
                          <div style="width: {Math.min(stat.cpuPercent, 100)}%; height: 100%; background: {stat.cpuPercent > 80 ? 'var(--color-danger, #ef4444)' : stat.cpuPercent > 40 ? 'var(--color-warning, #f59e0b)' : 'var(--color-success, #10b981)'}; border-radius: 3px;"></div>
                        </div>
                      </div>
                    </td>
                    <td>
                      <div style="display: flex; flex-direction: column; gap: 4px; width: 100%;">
                        <div style="display: flex; justify-content: space-between; font-size: 0.8rem; font-family: monospace;">
                          <span>{stat.MemUsage}</span>
                          <span style="color: var(--text-secondary);">{stat.MemPercent || stat.MemPerc}</span>
                        </div>
                        <div style="width: 100%; height: 6px; background: rgba(255,255,255,0.05); border-radius: 3px; overflow: hidden;">
                          <div style="width: {Math.min(stat.memPercent, 100)}%; height: 100%; background: {stat.memPercent > 80 ? 'var(--color-danger, #ef4444)' : stat.memPercent > 55 ? 'var(--color-warning, #f59e0b)' : 'var(--color-success, #10b981)'}; border-radius: 3px;"></div>
                        </div>
                      </div>
                    </td>
                    <td class="mono-val" style="font-size: 0.82rem;">{stat.NetIO}</td>
                    <td class="mono-val" style="font-size: 0.82rem;">{stat.BlockIO}</td>
                    <td class="mono-val" style="font-size: 0.82rem; text-align: right;">{stat.PIDs}</td>
                  </tr>
                {/each}

                {#if statsList.length === 0 && !statsLoading}
                  <tr>
                    <td colspan="6" class="empty-state">
                      No results
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
        <h3>Sudo authentication required</h3>
        <p class="modal-desc">This operation requires root privileges. Enter your user password (sudo):</p>
        <input
          type="password"
          placeholder="Enter password…"
          bind:value={sudoPassword}
          onkeydown={(e) => e.key === 'Enter' && submitSudoPassword()}
        />
        {#if sudoError}
          <span class="error-text">{sudoError}</span>
        {/if}
        <div class="modal-actions">
          <button class="primary" onclick={submitSudoPassword}>Submit</button>
          <button class="secondary" onclick={() => { showSudoModal = false; sudoPassword = ''; pendingAction = null; }}>Cancel</button>
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
        <h3>Confirm operation</h3>
        <p class="modal-desc">{confirmMessage}</p>
        <div class="modal-actions">
          <button class="danger" onclick={() => confirmAction && confirmAction()}>Yes, execute</button>
          <button class="secondary" onclick={() => { showConfirmModal = false; confirmAction = null; }}>Cancel</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Container Logs Modal -->
  {#if showLogsModal}
    <div class="modal-overlay logs-overlay">
      <div class="modal-content glass logs-modal">
        <div class="logs-header">
          <h3>{`Logs: ${logsContainerName}`}</h3>
          <div class="logs-controls">
            <div class="search-bar search-bar-sm">
              <span class="search-icon-wrapper"><Search size={14} /></span>
              <input type="text" placeholder="Filter logs…" bind:value={logSearch} />
            </div>
            <button class="secondary btn-sm" onclick={() => logsPaused = !logsPaused}>
              {#if logsPaused}
                <Play size={14} /> Stream logs
              {:else}
                <Pause size={14} /> Pause
              {/if}
            </button>
            <button class="secondary btn-sm" onclick={() => logLines = []}>
              <Eraser size={14} /> Clear
            </button>
            <button class="secondary btn-sm" onclick={downloadLogs}>
              <Download size={14} /> Download
            </button>
            <button class="secondary btn-sm" onclick={closeLogs}>
              <X size={14} />
            </button>
          </div>
        </div>
        <div class="logs-display" use:stickToBottom>
          <pre class="log-text">{getFilteredLogs().join('\n') || 'Waiting for logs…'}</pre>
        </div>
      </div>
    </div>
  {/if}

  <!-- Container Shell Modal -->
  {#if showShellModal}
    <div class="modal-overlay">
      <div class="modal-content glass exec-modal">
        <h3>Interactive shell: {shellPickContainerName}</h3>
        <p class="modal-desc">Choose a shell available in the container.</p>
        <div class="form-group">
          <label for="shell-select">Shell</label>
          <select id="shell-select" bind:value={selectedShell}>
            {#each shellOptions as opt}
              <option value={opt.value}>{opt.label}</option>
            {/each}
          </select>
        </div>
        <div class="modal-actions">
          <button class="primary" onclick={confirmShellLaunch}>
            <Terminal size={16} /> Open shell
          </button>
          <button class="secondary" onclick={() => showShellModal = false}>Cancel</button>
        </div>
      </div>
    </div>
  {/if}


  <!-- Pull Image Modal -->
  {#if showPullModal}
    <div class="modal-overlay">
      <div class="modal-content glass">
        <h3>Pull Docker image</h3>
        <div class="form-group">
          <label for="pull-name">Image name (e.g. nginx:latest, redis:7-alpine)</label>
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
              <Loader2 size={16} class="spin" /> Pulling…
            {:else}
              <Download size={16} /> Pull
            {/if}
          </button>
          <button class="secondary" onclick={() => { showPullModal = false; pullImageName = ''; pullProgress = ''; }}>Cancel</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Create Network Modal -->
  {#if showCreateNetworkModal}
    <div class="modal-overlay">
      <div class="modal-content glass">
        <h3>Create Docker network</h3>
        <div class="form-group">
          <label for="net-name">Network name</label>
          <input id="net-name" type="text" placeholder="my-network" bind:value={newNetworkName} />
        </div>
        <div class="form-group">
          <label for="net-driver">Network driver</label>
          <select id="net-driver" bind:value={newNetworkDriver}>
            <option value="bridge">Bridge (default)</option>
            <option value="overlay">Overlay</option>
            <option value="host">Host</option>
          </select>
        </div>
        <div class="modal-actions">
          <button class="primary" onclick={createNetwork} disabled={!newNetworkName.trim()}>Create</button>
          <button class="secondary" onclick={() => showCreateNetworkModal = false}>Cancel</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Directory Picker Modal (Compose) -->
  {#if showDirPicker}
    <div class="modal-overlay fullscreen-overlay">
      <div class="modal-content glass fullscreen-modal dir-picker-modal">
        <h3>New Compose project</h3>
        <p class="modal-desc">Choose a directory to create the project in and enter a folder name.</p>

        <!-- Current path -->
        <div class="dir-picker-path glass">
          <FolderOpen size={16} class="accent-amber-text" />
          <span class="mono-val">{dirPickerPath}</span>
        </div>

        <!-- Directory listing -->
        <ListSortBar
          columns={[{ id: 'name', label: 'Name' }]}
          activeColumn={dirPickerSort.column}
          direction={dirPickerSort.direction}
          onsort={(c) => dirPickerSort = nextSort(dirPickerSort, c)}
        />
        <div class="dir-picker-list">
          <button class="dir-entry" onclick={navigateUp}>
            <ChevronLeft size={14} />
            <span>..</span>
          </button>
          {#if dirPickerLoading}
            <div class="dir-picker-loading"><Loader2 size={20} class="spin" /></div>
          {:else}
            {#each getSortedDirPickerEntries() as entry}
              <button class="dir-entry" onclick={() => navigateDir(entry.name || entry.filename)}>
                <FolderOpen size={14} />
                <span>{entry.name || entry.filename}</span>
                <ChevronRight size={14} class="dir-chevron" />
              </button>
            {/each}
            {#if dirPickerEntries.length === 0}
              <div class="dir-picker-empty">No subdirectories in this folder</div>
            {/if}
          {/if}
        </div>

        <!-- New folder name -->
        <div class="form-group">
          <label for="new-folder">New project folder name</label>
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
            New Compose project
          </button>
          <button class="secondary" onclick={() => showDirPicker = false}>Cancel</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Compose Editor Modal -->
  {#if showComposeEditor}
    <div class="modal-overlay fullscreen-overlay">
      <div class="modal-content glass fullscreen-modal compose-editor-modal">
        <div class="compose-editor-header">
          <div style="display: flex; align-items: center; gap: 8px;">
            <h3>Compose editor</h3>
            <span class="mono-val compose-filepath">{composeEditorPath}</span>
          </div>
          <div class="compose-editor-actions" style="display: flex; gap: 8px; align-items: center;">
            <button class="primary btn-sm" onclick={saveComposeFile} disabled={composeEditorSaving}>
              {#if composeEditorSaving}
                <Loader2 size={14} class="spin" /> Saving…
              {:else}
                <Save size={14} /> Save
              {/if}
            </button>
            <button class="secondary btn-sm" onclick={closeComposeEditor}>
              <X size={14} /> Close
            </button>
          </div>
        </div>
        <div bind:this={composeEditorElement} class="compose-editor-container"></div>
      </div>
    </div>
  {/if}

  <!-- Compose Pull Modal -->
  {#if showComposePullModal}
    <div class="modal-overlay logs-overlay">
      <div class="modal-content glass compose-pull-modal">
        <div class="logs-header">
          <h3>{`Pull images: ${composePullProjectName}`}</h3>
          <button class="secondary btn-sm" onclick={closeComposePull}>
            <X size={14} /> Close
          </button>
        </div>

        <div class="pull-progress-section">
          <div class="pull-progress-bar">
            <div class="pull-progress-fill" style="width: {composePullProgress}%"></div>
          </div>
          <span class="pull-progress-label mono-val">{composePullProgress}% — {composePullStatus === 'pulling' ? "Pulling…" : composePullStatus === 'finished' ? "Done" : composePullStatus === 'error' ? "Error" : "Queued"}</span>
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
          <pre class="log-text">{composePullLogs.join('') || 'Waiting for pull data…'}</pre>
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
            <h3 style="display: flex; align-items: center; gap: 0.5rem;">
              Editing: {browserEditingFile.split('/').pop()}
              {#if browserSyntaxError}
                <span class="save-status-badge error" title={browserSyntaxError} style="font-size: 0.75rem; font-weight: normal; margin-left: 0.5rem; display: inline-flex; align-items: center;">
                  ● Syntax error
                </span>
              {/if}
            </h3>
            <div class="volume-editor-actions">
              <button class="primary btn-sm" onclick={saveVolumeFile} disabled={browserEditorSaving}>
                {#if browserEditorSaving}
                  <Loader2 size={14} class="spin" /> Saving…
                {:else}
                  <Save size={14} /> Save
                {/if}
              </button>
              <button class="secondary btn-sm" onclick={closeVolumeEditor}>
                <X size={14} /> Close editor
              </button>
            </div>
          </div>
          <div bind:this={browserEditorElement} class="volume-editor-container"></div>
        {:else}
          <div class="logs-header">
            <h3>Volume: {browserVolumeName}</h3>
            <button class="secondary btn-sm" onclick={() => { showVolumeBrowser = false; closeVolumeEditor(); }}>
              <X size={14} /> Close
            </button>
          </div>
          <div class="volume-path-bar glass">
            <button class="secondary btn-sm" onclick={navigateVolumeUp} disabled={!browserRelativePath}>
              <ArrowUp size={14} /> Go up
            </button>
            <span class="mono-val volume-path">{browserVolumePath}{browserRelativePath}</span>
          </div>
          <ListSortBar
            columns={[
              { id: 'name', label: 'Name' },
              { id: 'size', label: 'Size' },
              { id: 'modified', label: 'Modified' },
            ]}
            activeColumn={browserSort.column}
            direction={browserSort.direction}
            onsort={(c) => browserSort = nextSort(browserSort, c)}
          />
          <div class="volume-file-list">
            {#if browserLoading}
              <div class="loading-state"><Loader2 size={24} class="spin" /></div>
            {:else}
              {#each getSortedBrowserEntries() as entry}
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
                <div class="dir-picker-empty">Directory empty or no access</div>
              {/if}
            {/if}
          </div>
        {/if}
      </div>
    </div>
  {/if}

  <!-- Compose Network Change Modal -->
  {#if showComposeNetworkModal}
    <div class="modal-overlay">
      <div class="modal-content glass">
        <h3>Change project network</h3>
        <p class="modal-desc">
          Project: <strong>{composeNetworkProject?.Name || composeNetworkProject?.name}</strong><br />
          A networks.default section with the selected external network will be added.
        </p>
        <div class="form-group">
          <label for="compose-net-select">External network</label>
          <select id="compose-net-select" bind:value={composeSelectedNetwork}>
            <option value="">— select network —</option>
            {#each networks as network}
              <option value={network.Name}>{network.Name} ({network.Driver})</option>
            {/each}
          </select>
        </div>
        <div class="modal-actions">
          <button class="primary" onclick={saveComposeNetwork} disabled={!composeSelectedNetwork || composeNetworkLoading}>
            {#if composeNetworkLoading}
              <Loader2 size={16} class="spin" /> Saving…
            {:else}
              Apply and start
            {/if}
          </button>
          <button class="secondary" onclick={() => showComposeNetworkModal = false}>Cancel</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Compose Logs Modal -->
  {#if showComposeLogsModal}
    <div class="modal-overlay logs-overlay">
      <div class="modal-content glass logs-modal">
        <div class="logs-header">
          <h3>{`Compose logs: ${composeLogsProjectName}`}</h3>
          <button class="secondary btn-sm" onclick={() => showComposeLogsModal = false}>
            <X size={14} /> Close
          </button>
        </div>
        <div class="logs-display" use:stickToBottom>
          <pre class="log-text">{composeLogs.join('\n') || "(no data)"}</pre>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .docker-manager {
    /* uses .manager-shell from global.css */
  }

  /* ===== Container detail view ===== */
  .container-name-link {
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    color: var(--accent-amber);
    font: inherit;
    text-align: left;
  }
  .container-name-link:hover {
    text-decoration: underline;
  }

  .detail-breadcrumb {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-radius: var(--radius-md);
    flex-shrink: 0;
    font-size: 0.85rem;
  }
  .breadcrumb-current {
    color: var(--text-primary);
    font-weight: 600;
  }

  .detail-scroll {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-right: 4px;
  }

  .detail-card {
    border-radius: var(--radius-md);
    padding: 14px 16px;
  }

  .detail-card-header {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.95rem;
    font-weight: 600;
    color: var(--text-primary);
    margin-bottom: 12px;
    padding-bottom: 10px;
    border-bottom: 1px solid var(--border-color);
  }
  .detail-card-header :global(svg) {
    color: var(--accent-amber);
  }

  .detail-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .detail-kv {
    display: grid;
    grid-template-columns: 160px 1fr;
    gap: 8px 16px;
    align-items: center;
  }
  .kv-label {
    font-size: 0.8rem;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }
  .kv-value {
    font-size: 0.88rem;
    color: var(--text-primary);
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .kv-value.wrap {
    word-break: break-all;
  }

  .inline-edit {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .inline-edit input,
  .inline-edit select {
    min-width: 180px;
  }

  .code-chip {
    background: rgba(0, 0, 0, 0.35);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    padding: 2px 8px;
    font-family: "JetBrains Mono", Consolas, monospace;
    font-size: 0.82rem;
    word-break: break-all;
  }

  .detail-quick-links {
    display: flex;
    flex-wrap: wrap;
    gap: 16px;
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid var(--border-color);
  }
  .detail-quick-links .link-btn,
  .detail-breadcrumb .link-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: none;
    border: none;
    cursor: pointer;
    color: var(--accent-amber);
    font-size: 0.85rem;
    padding: 0;
  }
  .detail-quick-links .link-btn:hover,
  .detail-breadcrumb .link-btn:hover {
    text-decoration: underline;
  }

  .detail-subheader {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin: 16px 0 8px;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--text-secondary);
  }

  .detail-table-wrap {
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    overflow: hidden;
  }
  .data-table.compact th,
  .data-table.compact td {
    padding: 6px 10px;
    font-size: 0.82rem;
  }
  .data-table .wrap {
    word-break: break-all;
    white-space: normal;
  }

  .detail-empty {
    font-size: 0.85rem;
    color: var(--text-muted);
    padding: 6px 0;
  }

  .detail-raw {
    margin-top: 12px;
    max-height: 360px;
    overflow: auto;
  }

  .detail-join-row {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    margin-bottom: 12px;
  }
  .detail-join-row label {
    font-size: 0.85rem;
    color: var(--text-secondary);
  }
  .detail-join-row select {
    min-width: 240px;
  }

  /* Edit-mode (inline) */
  .detail-card-header .header-edit-btn {
    margin-left: auto;
  }

  .empty-example input,
  .empty-example select {
    opacity: 0.35;
    pointer-events: none;
  }
  .empty-example span {
    opacity: 0.35;
  }

  .form-sublabel {
    display: block;
    margin-bottom: 6px;
    font-size: 0.8rem;
    color: var(--text-secondary);
  }

  .edit-section-tabs {
    flex-wrap: wrap;
  }
  .edit-section-tabs .tab-btn {
    flex: 1 1 auto;
    white-space: nowrap;
  }

  .checkbox-row {
    display: flex;
    flex-wrap: wrap;
    gap: 20px;
    margin: 12px 0;
  }

  .caps-grid {
    display: grid;
    grid-template-columns: 1fr;
    gap: 2px;
    margin-top: 10px;
  }
  .caps-head,
  .caps-row {
    display: grid;
    grid-template-columns: 1fr 60px 60px;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
  }
  .caps-head {
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
    border-bottom: 1px solid var(--border-color);
  }
  .caps-head span:not(:first-child),
  .caps-row input {
    justify-self: center;
  }
  .caps-row:nth-child(even) {
    background: rgba(255, 255, 255, 0.02);
  }
  .caps-row .mono-val {
    font-size: 0.82rem;
  }

  .edit-footer {
    position: sticky;
    bottom: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .edit-footer-actions {
    display: flex;
    gap: 10px;
  }
  .edit-warning {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    font-size: 0.82rem;
    color: var(--accent-amber);
    background: rgba(245, 158, 11, 0.08);
    border: 1px solid rgba(245, 158, 11, 0.2);
    border-radius: var(--radius-sm);
    padding: 8px 10px;
  }
  .edit-warning :global(svg) {
    flex-shrink: 0;
    margin-top: 2px;
  }

  .docker-top-bar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 12px;
    border-radius: var(--radius-md);
    flex-shrink: 0;
    flex-wrap: wrap;
  }

  .docker-top-bar .page-title {
    flex-shrink: 0;
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
  .status-item {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .status-icon {
    color: var(--accent-amber);
  }

  .status-value {
    font-size: 0.82rem;
    color: var(--text-secondary);
    font-weight: 500;
  }

  .stats-row {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    margin-left: auto;
  }

  .stat-chip {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 3px 8px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-color);
    font-size: 0.72rem;
  }

  .stat-chip .mono-val {
    font-weight: 600;
    font-size: 0.78rem;
    font-variant-numeric: tabular-nums;
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
    padding: 3px;
    border-radius: var(--radius-md);
    gap: 3px;
    flex-shrink: 0;
  }

  .tab-btn {
    flex: 1;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    padding: 6px 8px;
    cursor: pointer;
    font-size: 0.78rem;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
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
    gap: 8px;
    flex: 1;
    overflow: hidden;
    min-height: 0;
  }

  /* Ops bar */
  .ops-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
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

  .search-icon-wrapper {
    position: absolute;
    left: 10px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--text-muted);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    pointer-events: none;
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
    padding: 8px 12px;
    border-bottom: 1px solid var(--border-color);
  }

  .data-table td {
    font-size: 0.9rem;
  }

  .data-table th {
    font-size: 0.72rem;
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
    padding: 6px 10px 6px 34px;
    font-size: 0.8rem;
  }

  .search-bar-sm .search-icon-wrapper {
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

  .save-status-badge {
    font-size: 0.8rem;
    padding: 4px 8px;
    border-radius: 4px;
    font-weight: 500;
  }
  .save-status-badge.error { color: #ef4444; background: rgba(239, 68, 68, 0.1); }

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
  .form-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
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
