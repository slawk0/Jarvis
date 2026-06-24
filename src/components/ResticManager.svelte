<script lang="ts">
  import { untrack } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import {
    Plus, Trash2, Play, FolderArchive, Loader2, Download, RefreshCw,
    AlertCircle, CheckCircle2, FolderOpen, ShieldAlert, Terminal, Eye,
    Settings, KeyRound, Check, HelpCircle, X, Search, Globe, HardDrive,
    Copy, ArrowLeft
  } from 'lucide-svelte';
  import SudoModal from './SudoModal.svelte';
  import PathAutocomplete from './ui/PathAutocomplete.svelte';
  import type { ResticRepo, ProfileExtras } from '$lib/admin/types';
  import { DEFAULT_PROFILE_EXTRAS } from '$lib/admin/types';
  import { notifications } from '$lib/notifications.svelte';
  import {
    formatInvokeError,
    isSudoPasswordRequired,
  } from '$lib/backendErrors';
  import { shellQuote, resticEnvPrefix } from '$lib/restic/env';

  let { profileId = '', visible = true } = $props();

  let extras = $state<ProfileExtras>({ ...DEFAULT_PROFILE_EXTRAS });
  let isLoading = $state(false);
  let isChecking = $state(false);
  let isInstalling = $state(false);
  let isResticInstalled = $state(false);
  let resticVersion = $state('');

  // Repositories state
  let selectedRepoId = $state('');
  let activeRepo = $derived(extras.restic_repos.find(r => r.id === selectedRepoId));
  
  // Snapshots state
  let snapshots = $state<any[]>([]);
  let isLoadingSnapshots = $state(false);
  let snapshotError = $state('');

  // File Browser state (rendered inline in place of the snapshots list)
  let isBrowsingFiles = $state(false);
  let selectedSnapshot = $state('');
  let snapshotFiles = $state<any[]>([]);
  let isLoadingFiles = $state(false);
  let searchQuery = $state('');
  const filesCache = new Map<string, any[]>();

  // File Preview & Download state
  let downloadingPaths = $state<string[]>([]);
  let isPreviewingFile = $state(false);
  let previewFileName = $state('');
  let previewFilePath = $state('');
  let previewFileSize = $state(0);
  let previewContent = $state('');
  let isBinaryPreview = $state(false);
  let isCopied = $state(false);

  // Modal / Form state
  let showConfigModal = $state(false);
  let editRepoId = $state<string | null>(null);
  let formName = $state('');
  let formType = $state<'local' | 's3' | 'sftp' | 'b2' | 'rest' | 'rclone'>('local');
  let formPathOrUrl = $state('');
  let formS3Endpoint = $state('');
  let formS3Region = $state('');
  let formS3Bucket = $state('');
  let formUseSudo = $state(false);
  let formPassword = $state('');
  let formAccessKey = $state('');
  let formSecretKey = $state('');
  let formEnvVarsText = $state('');

  // rclone detection state (for the config form)
  let rcloneRemotes = $state<string[]>([]);
  let isLoadingRemotes = $state(false);
  let rcloneError = $state('');

  // Backup Modal state
  let showBackupModal = $state(false);
  let backupPath = $state('');
  let backupTagsText = $state('');
  let backupExcludesText = $state('');
  let backupUseSudo = $state(false);

  // Restore Modal state
  let showRestoreModal = $state(false);
  let restoreSnapshotId = $state('');
  let restoreTargetPath = $state('');
  let restoreUseSudo = $state(false);

  // Streaming console modal state
  let showLogsModal = $state(false);
  let consoleTitle = $state('');
  let consoleOutput = $state('');
  let isConsoleRunning = $state(false);

  let showSudoModal = $state(false);
  let pendingAction: (() => Promise<void>) | null = null;

  async function exec(cmd: string, useSudo = false): Promise<string> {
    return invoke<string>('exec_custom_command', { cmd, useSudo });
  }

  async function withSudo(action: () => Promise<void>) {
    try {
      await action();
    } catch (err: unknown) {
      if (isSudoPasswordRequired(err)) {
        pendingAction = () => withSudo(action);
        showSudoModal = true;
      } else {
        notifications.error(formatInvokeError(err));
      }
    }
  }

  // Redirect restic's stdin from /dev/null so it never blocks waiting for an
  // interactive password prompt (restic/rclone fall back to stdin when the
  // password env var is missing). Without this the SSH command hangs forever.
  function noStdin(cmd: string): string {
    return `${cmd} < /dev/null`;
  }

  // Shared restic env-prefix builder (see $lib/restic/env).
  const buildResticEnvPrefix = resticEnvPrefix;

  async function checkResticInstalled() {
    isChecking = true;
    try {
      await exec('which restic', false);
      isResticInstalled = true;
      try {
        const ver = await exec('restic version', false);
        resticVersion = ver.trim();
      } catch {
        resticVersion = 'Installed';
      }
    } catch {
      isResticInstalled = false;
      resticVersion = '';
    } finally {
      isChecking = false;
    }
  }

  async function installRestic() {
    isInstalling = true;
    const cmd = `
if command -v apt-get >/dev/null 2>&1; then
  sudo apt-get update && sudo apt-get install -y restic
elif command -v yum >/dev/null 2>&1; then
  sudo yum install -y restic
elif command -v dnf >/dev/null 2>&1; then
  sudo dnf install -y restic
elif command -v apk >/dev/null 2>&1; then
  sudo apk add restic
elif command -v pacman >/dev/null 2>&1; then
  sudo pacman -Sy --noconfirm restic
else
  echo "Unsupported package manager. Please install restic manually." >&2
  exit 1
fi
    `.trim();

    await withSudo(async () => {
      try {
        await runResticStreamCommand("Install Restic", cmd, true);
        await checkResticInstalled();
      } catch (err: any) {
        notifications.error(`Failed to install: ${formatInvokeError(err)}`);
      } finally {
        isInstalling = false;
      }
    });
  }

  async function detectRcloneRemotes() {
    isLoadingRemotes = true;
    rcloneError = '';
    rcloneRemotes = [];
    
    await withSudo(async () => {
      isLoadingRemotes = true;
      rcloneError = '';
      try {
        const out = await exec('rclone listremotes', formUseSudo);
        rcloneRemotes = out
          .split('\n')
          .map(l => l.trim().replace(/:$/, ''))
          .filter(Boolean);
        if (rcloneRemotes.length === 0) {
          rcloneError = 'No rclone remotes configured on the server. Run `rclone config` there first.';
        }
      } catch (err: any) {
        if (isSudoPasswordRequired(err)) {
          throw err;
        }
        rcloneError = `Failed to list rclone remotes: ${formatInvokeError(err)}`;
      } finally {
        isLoadingRemotes = false;
      }
    });
  }

  function applyRcloneRemote(remote: string) {
    // Preserve any path the user already typed after the remote.
    const existingPath = formPathOrUrl.replace(/^rclone:/, '').split(':').slice(1).join(':');
    formPathOrUrl = `rclone:${remote}:${existingPath}`;
  }

  async function loadExtras() {
    if (!profileId) return;
    // Guard against concurrent loads: two simultaneous get_profile_extras calls
    // trigger concurrent keyring reads, which can intermittently fail and drop
    // the repository password.
    if (isLoading) return;
    isLoading = true;
    try {
      extras = await invoke<ProfileExtras>('get_profile_extras', { profileId });
      if (extras.restic_repos && extras.restic_repos.length > 0) {
        if (!selectedRepoId || !extras.restic_repos.some(r => r.id === selectedRepoId)) {
          selectedRepoId = extras.restic_repos[0].id;
        }
      } else {
        selectedRepoId = '';
        snapshots = [];
      }
    } catch (err: unknown) {
      notifications.error(formatInvokeError(err));
    } finally {
      isLoading = false;
    }
  }

  async function saveExtras() {
    await invoke('save_profile_extras', { profileId, extras });
  }

  function openAdd() {
    editRepoId = null;
    formName = '';
    formType = 'local';
    formPathOrUrl = '';
    formS3Endpoint = '';
    formS3Region = '';
    formS3Bucket = '';
    formUseSudo = false;
    formPassword = '';
    formAccessKey = '';
    formSecretKey = '';
    formEnvVarsText = '';
    rcloneRemotes = [];
    rcloneError = '';
    showConfigModal = true;
  }

  function openEdit(repo: ResticRepo) {
    editRepoId = repo.id;
    formName = repo.name;
    formType = repo.repo_type;
    formPathOrUrl = repo.path_or_url;
    formS3Endpoint = repo.s3_endpoint || '';
    formS3Region = repo.s3_region || '';
    formS3Bucket = repo.s3_bucket || '';
    formUseSudo = repo.use_sudo;
    formPassword = repo.password || '';
    formAccessKey = repo.access_key || '';
    formSecretKey = repo.secret_key || '';
    
    if (repo.env_vars) {
      formEnvVarsText = Object.entries(repo.env_vars)
        .map(([k, v]) => `${k}=${v}`)
        .join('\n');
    } else {
      formEnvVarsText = '';
    }

    rcloneRemotes = [];
    rcloneError = '';
    showConfigModal = true;

    if (repo.repo_type === 'rclone') {
      detectRcloneRemotes();
    }
  }

  async function saveRepo() {
    if (!formName.trim() || !formPathOrUrl.trim()) {
      notifications.error("Name and path are required");
      return;
    }

    const envVars: Record<string, string> = {};
    if (formEnvVarsText.trim()) {
      for (const line of formEnvVarsText.split('\n')) {
        const parts = line.split('=');
        if (parts.length >= 2) {
          envVars[parts[0].trim()] = parts.slice(1).join('=').trim();
        }
      }
    }

    const repo: ResticRepo = {
      id: editRepoId || Date.now().toString(),
      name: formName.trim(),
      repo_type: formType,
      path_or_url: formPathOrUrl.trim(),
      s3_endpoint: formS3Endpoint.trim() || null,
      s3_region: formS3Region.trim() || null,
      s3_bucket: formS3Bucket.trim() || null,
      use_sudo: formUseSudo,
      password: formPassword || null,
      access_key: formAccessKey || null,
      secret_key: formSecretKey || null,
      env_vars: Object.keys(envVars).length > 0 ? envVars : null
    };

    if (editRepoId) {
      extras.restic_repos = extras.restic_repos.map(r => r.id === editRepoId ? repo : r);
    } else {
      extras.restic_repos = [...extras.restic_repos, repo];
    }

    await saveExtras();
    showConfigModal = false;
    await loadExtras();
  }

  async function deleteRepo(id: string) {
    if (!confirm("Delete this repository configuration? This will not touch the remote data, just the profile settings.")) return;
    extras.restic_repos = extras.restic_repos.filter(r => r.id !== id);
    await saveExtras();
    await loadExtras();
  }

  async function runResticStreamCommand(title: string, cmd: string, useSudo: boolean): Promise<boolean> {
    consoleTitle = title;
    consoleOutput = `$ ${cmd}\n`;
    showLogsModal = true;
    isConsoleRunning = true;

    const eventId = Math.random().toString(36).substring(7);
    const unStdout = await listen<string>(`exec-stdout-${eventId}`, (e) => (consoleOutput += e.payload));
    const unStderr = await listen<string>(`exec-stderr-${eventId}`, (e) => (consoleOutput += e.payload));

    try {
      const out = await invoke<string>('exec_custom_command_stream', { cmd, useSudo, eventId });
      if (out.includes("Command failed")) {
        return false;
      }
      return true;
    } catch (err: any) {
      consoleOutput += `\nError: ${formatInvokeError(err)}`;
      return false;
    } finally {
      unStdout();
      unStderr();
      isConsoleRunning = false;
    }
  }

  async function initRepository() {
    if (!activeRepo) return;
    const env = buildResticEnvPrefix(activeRepo);
    const cmd = noStdin(`${env}restic -r ${shellQuote(activeRepo.path_or_url)} init`);

    await withSudo(async () => {
      const ok = await runResticStreamCommand("Initialize Repository", cmd, activeRepo.use_sudo);
      if (ok) {
        notifications.success("Repository initialized successfully");
        await loadSnapshots();
      }
    });
  }

  async function loadSnapshots() {
    if (!activeRepo) return;
    isLoadingSnapshots = true;
    snapshotError = '';
    snapshots = [];
    
    try {
      const env = buildResticEnvPrefix(activeRepo);
      const cmd = noStdin(`${env}restic -r ${shellQuote(activeRepo.path_or_url)} snapshots --no-lock --json`);
      const out = await exec(cmd, activeRepo.use_sudo);
      
      try {
        snapshots = JSON.parse(out) || [];
        // Sort newest first
        snapshots.sort((a, b) => new Date(b.time).getTime() - new Date(a.time).getTime());
      } catch (err) {
        console.error("Failed to parse snapshots JSON:", out, err);
        snapshotError = "Failed to parse snapshots output. The repository might not be initialized yet.";
      }
    } catch (err: any) {
      const errMsg = formatInvokeError(err);
      if (errMsg.includes("repository does not exist") || errMsg.includes("unable to open config")) {
        snapshotError = "Repository is not initialized or does not exist at the specified path.";
      } else if (errMsg.includes("empty password") || errMsg.includes("password from stdin") || errMsg.includes("wrong password")) {
        snapshotError = "No valid repository password. Edit the repository and set its encryption password.";
      } else {
        snapshotError = errMsg;
      }
    } finally {
      isLoadingSnapshots = false;
    }
  }

  async function checkRepository() {
    if (!activeRepo) return;
    const env = buildResticEnvPrefix(activeRepo);
    const cmd = noStdin(`${env}restic -r ${shellQuote(activeRepo.path_or_url)} check`);
    await withSudo(async () => {
      await runResticStreamCommand("Check Repository Integrity", cmd, activeRepo.use_sudo);
    });
  }

  let currentPath = $state('');

  function getBreadcrumbs(path: string) {
    const parts = path.split('/').filter(Boolean);
    const breadcrumbs = [];
    let current = '';
    for (const part of parts) {
      current += '/' + part;
      breadcrumbs.push({ name: part, path: current });
    }
    return breadcrumbs;
  }

  function navigateTo(path: string) {
    browseFiles(selectedSnapshot, path);
  }

  function isDirectChild(parent: string, child: string): boolean {
    if (!parent) return false;
    if (child === parent) return false;
    
    if (parent === '/') {
      if (!child.startsWith('/')) return false;
      const rel = child.slice(1);
      return rel.length > 0 && !rel.includes('/');
    } else {
      const prefix = parent.endsWith('/') ? parent : parent + '/';
      if (!child.startsWith(prefix)) return false;
      const rel = child.slice(prefix.length);
      return rel.length > 0 && !rel.includes('/');
    }
  }

  async function browseFiles(snapshotId: string, path?: string) {
    if (!activeRepo) return;
    selectedSnapshot = snapshotId;
    isBrowsingFiles = true;
    isPreviewingFile = false;

    const snap = snapshots.find(s => s.id === snapshotId);

    if (path === undefined) {
      searchQuery = '';
      if (snap && snap.paths && snap.paths.length === 1) {
        currentPath = snap.paths[0];
      } else {
        currentPath = '';
      }
    } else {
      currentPath = path;
    }

    const cacheKey = snapshotId;
    if (filesCache.has(cacheKey)) {
      snapshotFiles = filesCache.get(cacheKey) || [];
      isLoadingFiles = false;
      return;
    }

    isLoadingFiles = true;
    snapshotFiles = [];

    try {
      const env = buildResticEnvPrefix(activeRepo);
      const cmd = noStdin(`${env}restic -r ${shellQuote(activeRepo.path_or_url)} ls ${snapshotId} --no-lock --json`);
      const out = await exec(cmd, activeRepo.use_sudo);
      
      const lines = out.split('\n')
        .map(l => l.trim())
        .filter(l => l.startsWith('{'));
      const parsed = JSON.parse('[' + lines.join(',') + ']');

      const validFiles = parsed.filter((f: any) => f.path);

      filesCache.set(cacheKey, validFiles);
      snapshotFiles = validFiles;
    } catch (err: any) {
      notifications.error(`Failed to load file list: ${formatInvokeError(err)}`);
      currentPath = '';
      snapshotFiles = [];
    } finally {
      isLoadingFiles = false;
    }
  }

  function closeBrowse() {
    isBrowsingFiles = false;
    isPreviewingFile = false;
  }

  async function downloadFile(snapshotId: string, filePath: string, size: number) {
    if (!activeRepo) return;
    
    const fileName = filePath.split('/').pop() || 'file';
    notifications.info(`Initiating download for "${fileName}" (${formatBytes(size)})...`);
    
    downloadingPaths = [...downloadingPaths, filePath];

    try {
      const ext = filePath.split('.').pop() || 'tmp';
      const remoteTempPath = `/tmp/jarvis-restic-dl-${Date.now()}.${ext}`;
      const env = buildResticEnvPrefix(activeRepo);
      
      const cmd = noStdin(`${env}restic -r ${shellQuote(activeRepo.path_or_url)} dump ${snapshotId} ${shellQuote(filePath)} > ${remoteTempPath}`);
      await exec(cmd, activeRepo.use_sudo);

      if (activeRepo.use_sudo) {
        await exec(`chmod 644 ${remoteTempPath}`, true).catch(() => {});
      }

      const count = await invoke<number>('sftp_start_download_batch', {
        remotePaths: [remoteTempPath],
        localDir: null,
      });

      notifications.success(`Started downloading "${fileName}". Track progress in Transfer manager.`);

      const cleanCmd = `nohup sh -c 'sleep 300 && rm -f ${remoteTempPath}' >/dev/null 2>&1 &`;
      await exec(cleanCmd, activeRepo.use_sudo).catch(() => {});
    } catch (err: any) {
      notifications.error(`Failed to initiate download: ${formatInvokeError(err)}`);
    } finally {
      downloadingPaths = downloadingPaths.filter(p => p !== filePath);
    }
  }

  async function previewFile(snapshotId: string, filePath: string, size: number) {
    if (!activeRepo) return;

    const fileName = filePath.split('/').pop() || 'file';
    previewFileName = fileName;
    previewFilePath = filePath;
    previewFileSize = size;
    isPreviewingFile = true;
    previewContent = 'Loading file contents...';
    isBinaryPreview = false;
    isCopied = false;

    if (size > 1024 * 1024 * 5) {
      previewContent = `File is too large to preview (${formatBytes(size)}). Please download the file to view its contents.`;
      return;
    }

    try {
      const env = buildResticEnvPrefix(activeRepo);
      // Run restic dump directly without head pipe to avoid SIGPIPE masking
      const cmd = `${env}restic -r ${shellQuote(activeRepo.path_or_url)} dump ${snapshotId} ${shellQuote(filePath)} --no-lock < /dev/null`;
      const out = await exec(cmd, activeRepo.use_sudo);

      const hasNullBytes = out.includes('\u0000') || /[\x00-\x08\x0E-\x1F\x7F]/.test(out.slice(0, 1000));
      if (hasNullBytes) {
        isBinaryPreview = true;
        previewContent = `This file appears to be a binary file (${formatBytes(size)}). Preview is not available. Please download the file to view its contents.`;
      } else {
        previewContent = out;
      }
    } catch (err: any) {
      previewContent = `Failed to load preview:\n${formatInvokeError(err)}`;
    }
  }

  async function copyPreviewToClipboard() {
    try {
      if (navigator.clipboard && navigator.clipboard.writeText) {
        await navigator.clipboard.writeText(previewContent);
      } else {
        const textarea = document.querySelector('.preview-textarea') as HTMLTextAreaElement;
        if (textarea) {
          textarea.select();
          document.execCommand('copy');
        }
      }
      isCopied = true;
      setTimeout(() => { isCopied = false; }, 2000);
      notifications.success("Copied to clipboard!");
    } catch (err) {
      notifications.error("Failed to copy to clipboard");
    }
  }

  function openBackup() {
    backupPath = '';
    backupTagsText = '';
    backupExcludesText = '';
    backupUseSudo = activeRepo?.use_sudo || false;
    showBackupModal = true;
  }

  async function runBackup() {
    if (!activeRepo || !backupPath.trim()) {
      notifications.error("Enter path to backup");
      return;
    }

    const paths = backupPath.split(',').map(p => p.trim()).filter(Boolean);
    const tags = backupTagsText.split(',').map(t => t.trim()).filter(Boolean);
    const excludes = backupExcludesText.split(',').map(e => e.trim()).filter(Boolean);

    const env = buildResticEnvPrefix(activeRepo);
    let cmd = `${env}restic -r ${shellQuote(activeRepo.path_or_url)} backup`;
    
    for (const p of paths) cmd += ` ${shellQuote(p)}`;
    for (const t of tags) cmd += ` --tag ${shellQuote(t)}`;
    for (const e of excludes) cmd += ` --exclude ${shellQuote(e)}`;
    cmd = noStdin(cmd);

    showBackupModal = false;

    await withSudo(async () => {
      const ok = await runResticStreamCommand("Create Backup", cmd, backupUseSudo);
      if (ok) {
        notifications.success("Backup successfully completed");
        await loadSnapshots();
      }
    });
  }

  function openRestore(snapshotId: string) {
    restoreSnapshotId = snapshotId;
    restoreTargetPath = '';
    restoreUseSudo = activeRepo?.use_sudo || false;
    showRestoreModal = true;
  }

  async function runRestore() {
    if (!activeRepo || !restoreTargetPath.trim()) {
      notifications.error("Enter restore target path");
      return;
    }

    const env = buildResticEnvPrefix(activeRepo);
    const cmd = noStdin(`${env}restic -r ${shellQuote(activeRepo.path_or_url)} restore ${restoreSnapshotId} --target ${shellQuote(restoreTargetPath.trim())}`);
    
    showRestoreModal = false;

    await withSudo(async () => {
      const ok = await runResticStreamCommand("Restore Snapshot", cmd, restoreUseSudo);
      if (ok) {
        notifications.success("Restore successfully completed");
      }
    });
  }

  async function forgetSnapshot(snapshotId: string) {
    if (!activeRepo) return;
    if (!confirm(`Are you sure you want to forget and prune snapshot ${snapshotId.slice(0, 8)}?\nThis is a destructive action.`)) return;

    const env = buildResticEnvPrefix(activeRepo);
    const cmd = noStdin(`${env}restic -r ${shellQuote(activeRepo.path_or_url)} forget ${snapshotId} --prune`);
    
    await withSudo(async () => {
      const ok = await runResticStreamCommand("Forget & Prune Snapshot", cmd, activeRepo.use_sudo);
      if (ok) {
        notifications.success("Snapshot removed and repository pruned");
        await loadSnapshots();
      }
    });
  }

  function formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  }

  function formatDate(dStr: string): string {
    return new Date(dStr).toLocaleString();
  }

  let filteredFiles = $derived(
    (() => {
      const search = searchQuery.trim().toLowerCase();

      if (!currentPath) {
        const snap = snapshots.find(s => s.id === selectedSnapshot);
        if (!snap || !snap.paths) return [];
        
        const baseDirs = snap.paths.map((p: string) => ({
          path: p,
          type: 'dir',
          name: p,
          size: 0,
          mode: 0o755,
          mtime: ''
        }));

        if (search) {
          return baseDirs.filter((d: any) => d.name.toLowerCase().includes(search));
        }
        return baseDirs;
      }

      return snapshotFiles
        .filter(f => {
          if (search) {
            if (f.path === currentPath) return false;
            if (currentPath === '/') {
              if (!f.path.startsWith('/')) return false;
            } else {
              const prefix = currentPath.endsWith('/') ? currentPath : currentPath + '/';
              if (!f.path.startsWith(prefix)) return false;
            }
            return (f.name || '').toLowerCase().includes(search);
          } else {
            return isDirectChild(currentPath, f.path);
          }
        })
        .slice(0, 1000);
    })()
  );

  // Note: do NOT also load in onMount — the $effect below already runs on mount.
  // Loading from both caused two concurrent get_profile_extras calls, and the
  // resulting concurrent keyring reads could intermittently fail (returning an
  // empty repository password), which made restic prompt for a password on stdin.
  $effect(() => {
    if (profileId) {
      untrack(() => {
        checkResticInstalled();
        loadExtras();
      });
    }
  });

  $effect(() => {
    if (selectedRepoId) {
      untrack(() => {
        filesCache.clear();
        currentPath = '';
        loadSnapshots();
      });
    }
  });

  export function refresh() {
    checkResticInstalled();
    loadExtras();
    if (selectedRepoId) loadSnapshots();
  }
</script>

<div class="restic manager-shell scrollable fade-in">
  <header class="manager-header">
    <h1 class="page-title">
      Restic Backups
      {#if resticVersion}
        <span class="version-tag">{resticVersion.split('\n')[0]}</span>
      {/if}
    </h1>
    <div class="header-actions">
      <button class="primary btn-compact" onclick={openAdd}>
        <Plus size={14} /> New repository
      </button>
      <button class="secondary btn-compact" onclick={refresh} disabled={isLoading || isChecking}>
        {#if isLoading || isChecking}
          <Loader2 size={14} class="spin" />
        {:else}
          <RefreshCw size={14} />
        {/if}
      </button>
    </div>
  </header>

  {#if !isResticInstalled && !isChecking}
    <div class="empty glass warning-container">
      <ShieldAlert size={48} class="accent-red-text animate-pulse" />
      <h3>Restic is not installed</h3>
      <p>Restic is required on the remote server in order to manage and perform backups.</p>
      <button class="primary" onclick={installRestic} disabled={isInstalling}>
        {#if isInstalling}
          <Loader2 size={14} class="spin" /> Installing...
        {:else}
          <Terminal size={14} /> Install Restic
        {/if}
      </button>
    </div>
  {:else if extras.restic_repos.length === 0}
    <div class="empty glass">
      <FolderArchive size={36} class="muted" />
      <p>No Restic repositories configured. Add one to start managing backups.</p>
      <button class="primary btn-compact" onclick={openAdd}><Plus size={14} /> Add repository</button>
    </div>
  {:else}
    <div class="restic-grid">
      <!-- Sidebar Selector -->
      <div class="repo-selector glass">
        <h4>Repositories</h4>
        <div class="repo-list">
          {#each extras.restic_repos as repo}
            <button 
              class="repo-item" 
              class:active={repo.id === selectedRepoId}
              onclick={() => selectedRepoId = repo.id}
            >
              <div class="repo-item-header">
                {#if repo.repo_type === 'local'}
                  <HardDrive size={14} class="repo-icon text-muted" />
                {:else}
                  <Globe size={14} class="repo-icon text-amber" />
                {/if}
                <span class="repo-name">{repo.name}</span>
              </div>
              <span class="repo-path" title={repo.path_or_url}>{repo.path_or_url}</span>
            </button>
          {/each}
        </div>
      </div>

      <!-- Main Repo Control Dashboard -->
      <div class="repo-dashboard">
        {#if activeRepo}
          <div class="dashboard-header glass">
            <div class="repo-info-card">
              <h3>{activeRepo.name}</h3>
              <div class="repo-meta">
                <span class="badge">{activeRepo.repo_type.toUpperCase()}</span>
                {#if activeRepo.use_sudo}
                  <span class="badge warning">SUDO</span>
                {/if}
                <span class="repo-url mono-val">{activeRepo.path_or_url}</span>
              </div>
            </div>
            <div class="dashboard-actions">
              <button class="primary btn-compact" disabled={isLoadingSnapshots} onclick={openBackup}>
                <Play size={14} /> Backup now
              </button>
              <button class="secondary btn-compact" onclick={() => openEdit(activeRepo)}>
                <Settings size={14} /> Edit config
              </button>
              <button class="secondary btn-compact" onclick={checkRepository}>
                <CheckCircle2 size={14} /> Integrity check
              </button>
              <button class="secondary btn-compact hover-red" onclick={() => deleteRepo(activeRepo.id)}>
                <Trash2 size={14} />
              </button>
            </div>
          </div>

          <!-- Snapshots / inline file browser section -->
          <div class="snapshots-container glass">
            {#if isBrowsingFiles}
              <!-- Inline File Browser (replaces the snapshots list) -->
              <div class="section-title-bar">
                <div class="browse-heading">
                  <button class="secondary btn-compact" onclick={closeBrowse}>
                    <ArrowLeft size={12} /> Snapshots
                  </button>
                  <h4>Files in <span class="mono-val text-amber">{selectedSnapshot.slice(0, 8)}</span></h4>
                </div>
              </div>

              {#if isPreviewingFile}
                <!-- Inline File Preview (replaces the file list) -->
                <div class="inline-preview">
                  <div class="preview-header">
                    <div class="preview-file-info">
                      <button class="secondary btn-compact" onclick={() => isPreviewingFile = false}>
                        <ArrowLeft size={12} /> Files
                      </button>
                      <div class="preview-name-block">
                        <span class="preview-name mono-val text-amber">{previewFileName}</span>
                        <span class="preview-path mono-val" title={previewFilePath}>{previewFilePath} ({formatBytes(previewFileSize)})</span>
                      </div>
                    </div>
                    <div class="preview-actions-inline">
                      {#if !isBinaryPreview && previewContent !== 'Loading file contents...' && !previewContent.startsWith('Failed to load preview')}
                        <button class="secondary btn-compact" onclick={copyPreviewToClipboard}>
                          <Copy size={14} />
                          {isCopied ? 'Copied!' : 'Copy'}
                        </button>
                      {/if}
                      {#if previewContent !== 'Loading file contents...'}
                        <button class="primary btn-compact" onclick={() => { downloadFile(selectedSnapshot, previewFilePath, previewFileSize); }}>
                          <Download size={14} /> Download
                        </button>
                      {/if}
                    </div>
                  </div>
                  <textarea class="preview-textarea inline" readonly bind:value={previewContent}></textarea>
                </div>
              {:else}
                <div class="search-bar-row">
                  <div class="search-input-wrapper">
                    <Search size={14} class="search-icon" />
                    <input bind:value={searchQuery} placeholder="Search files and folders..." />
                  </div>
                </div>

                <div class="breadcrumbs-bar glass">
                  <button class="breadcrumb-item" onclick={() => navigateTo('')}>Root</button>
                  {#if currentPath}
                    {#each getBreadcrumbs(currentPath) as bc}
                      <span class="separator">/</span>
                      <button class="breadcrumb-item" onclick={() => navigateTo(bc.path)}>{bc.name}</button>
                    {/each}
                  {/if}
                </div>

                <div class="files-list-container scrollable inline-files">
                  {#if isLoadingFiles}
                    <div class="loading-state">
                      <Loader2 size={24} class="spin muted" />
                      <p>Scanning snapshot filesystem...</p>
                    </div>
                  {:else if filteredFiles.length === 0}
                    <div class="empty-state">
                      <FolderOpen size={24} class="muted" />
                      <p>No matching files found.</p>
                    </div>
                  {:else}
                    <div class="files-table-wrapper">
                      <table>
                        <thead>
                          <tr>
                            <th>Type</th>
                            <th>Name</th>
                            <th>Size</th>
                            <th>Permissions</th>
                            <th>Modified Time</th>
                            <th>Actions</th>
                          </tr>
                        </thead>
                        <tbody>
                          {#each filteredFiles as f}
                            <tr class:clickable-row={f.type === 'dir'} onclick={() => { if (f.type === 'dir') navigateTo(f.path); }}>
                              <td>
                                <span class="file-type-badge" class:dir={f.type === 'dir'}>
                                  {f.type.toUpperCase()}
                                </span>
                              </td>
                              <td class="mono-val path-val" title={f.path}>
                                {#if f.type === 'dir'}
                                  <FolderOpen size={12} class="dir-icon" />
                                {/if}
                                {f.name}
                              </td>
                              <td class="mono-val text-right">{f.size !== undefined && f.type === 'file' ? formatBytes(f.size) : '—'}</td>
                              <td class="mono-val">{f.mode ? (f.mode & 0o777).toString(8) : '—'}</td>
                              <td>{f.mtime ? formatDate(f.mtime) : '—'}</td>
                              <td>
                                {#if f.type === 'file'}
                                  <div class="actions-cell">
                                    <button
                                      class="icon-btn-compact"
                                      title="Preview file"
                                      onclick={(e) => { e.stopPropagation(); previewFile(selectedSnapshot, f.path, f.size); }}
                                    >
                                      <Eye size={14} />
                                    </button>
                                    <button
                                      class="icon-btn-compact"
                                      title="Download file"
                                      disabled={downloadingPaths.includes(f.path)}
                                      onclick={(e) => { e.stopPropagation(); downloadFile(selectedSnapshot, f.path, f.size); }}
                                    >
                                      {#if downloadingPaths.includes(f.path)}
                                        <Loader2 size={14} class="spin" />
                                      {:else}
                                        <Download size={14} />
                                      {/if}
                                    </button>
                                  </div>
                                {/if}
                              </td>
                            </tr>
                          {/each}
                        </tbody>
                      </table>
                    </div>
                    {#if snapshotFiles.length > 1000}
                      <div class="limit-hint">
                        Showing first 1000 items of {snapshotFiles.length} total elements. Use search to filter.
                      </div>
                    {/if}
                  {/if}
                </div>
              {/if}
            {:else}
            <div class="section-title-bar">
              <h4>Snapshots</h4>
              <button class="secondary btn-compact" onclick={loadSnapshots} disabled={isLoadingSnapshots}>
                {#if isLoadingSnapshots}
                  <Loader2 size={12} class="spin" />
                {:else}
                  <RefreshCw size={12} />
                {/if}
              </button>
            </div>

            {#if isLoadingSnapshots}
              <div class="loading-state">
                <Loader2 size={24} class="spin muted" />
                <p>Querying repository snapshots...</p>
              </div>
            {:else if snapshotError}
              <div class="error-state">
                <AlertCircle size={24} class="accent-red-text" />
                <p>{snapshotError}</p>
                <div class="error-actions">
                  <button class="primary" onclick={initRepository}>
                    <Plus size={14} /> Initialize Repository
                  </button>
                  <button class="secondary" onclick={loadSnapshots}>
                    Try again
                  </button>
                </div>
              </div>
            {:else if snapshots.length === 0}
              <div class="empty-state">
                <FolderOpen size={24} class="muted" />
                <p>No snapshots found in this repository.</p>
                <button class="primary btn-compact" onclick={openBackup}>
                  <Play size={12} /> Take first backup
                </button>
              </div>
            {:else}
              <div class="table-wrapper scrollable-table">
                <table>
                  <thead>
                    <tr>
                      <th>Snapshot ID</th>
                      <th>Time</th>
                      <th>Host</th>
                      <th>Paths</th>
                      <th>Tags</th>
                      <th class="actions-col">Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    {#each snapshots as snap}
                      <tr>
                        <td class="mono-val text-amber">{snap.id.slice(0, 8)}</td>
                        <td>{formatDate(snap.time)}</td>
                        <td>{snap.hostname}</td>
                        <td class="paths-cell">
                          {#each snap.paths as path}
                            <span class="path-tag mono-val" title={path}>{path}</span>
                          {/each}
                        </td>
                        <td>
                          {#if snap.tags && snap.tags.length > 0}
                            {#each snap.tags as tag}
                              <span class="tag-badge">{tag}</span>
                            {/each}
                          {:else}
                            <span class="muted">—</span>
                          {/if}
                        </td>
                        <td class="actions-col">
                          <div class="row-actions">
                            <button class="action-btn" title="Browse files" onclick={() => browseFiles(snap.id)}>
                              <Eye size={12} />
                            </button>
                            <button class="action-btn" title="Restore snapshot" onclick={() => openRestore(snap.id)}>
                              <Download size={12} />
                            </button>
                            <button class="action-btn hover-red" title="Forget snapshot" onclick={() => forgetSnapshot(snap.id)}>
                              <Trash2 size={12} />
                            </button>
                          </div>
                        </td>
                      </tr>
                    {/each}
                  </tbody>
                </table>
              </div>
            {/if}
            {/if}
          </div>
        {/if}
      </div>
    </div>
  {/if}
</div>

<!-- Modal: Add / Edit Repository -->
{#if showConfigModal}
  <div class="modal-overlay" role="presentation" onclick={() => showConfigModal = false}>
    <div class="modal glass" role="dialog" onclick={(e) => e.stopPropagation()}>
      <h3>{editRepoId ? "Edit repository" : "New Restic repository"}</h3>
      
      <label>Name
        <input bind:value={formName} placeholder="Production backup" />
      </label>

      <label>Type
        <select bind:value={formType}>
          <option value="local">Local Directory</option>
          <option value="s3">Amazon S3 or S3-compatible</option>
          <option value="b2">Backblaze B2</option>
          <option value="sftp">SFTP Server</option>
          <option value="rest">REST Server</option>
          <option value="rclone">rclone Remote</option>
        </select>
      </label>

      {#if formType === 'local'}
        <label>Directory path
          <PathAutocomplete bind:value={formPathOrUrl} placeholder="/var/backups/restic" onlyDirs={true} />
        </label>
      {:else if formType === 's3'}
        <label>S3 Repository URL
          <input bind:value={formPathOrUrl} placeholder="s3:https://s3.amazonaws.com/bucket-name" />
        </label>
        <label>S3 Endpoint (optional)
          <input bind:value={formS3Endpoint} placeholder="s3.us-west-002.backblazeb2.com" />
        </label>
        <label>S3 Region (optional)
          <input bind:value={formS3Region} placeholder="us-east-1" />
        </label>
        <label>Access Key ID
          <input bind:value={formAccessKey} placeholder="AKIA..." />
        </label>
        <label>Secret Access Key
          <input type="password" bind:value={formSecretKey} />
        </label>
      {:else if formType === 'b2'}
        <label>B2 Repository URL
          <input bind:value={formPathOrUrl} placeholder="b2:bucket-name:path/to/repo" />
        </label>
        <label>B2 Application Key ID
          <input bind:value={formAccessKey} />
        </label>
        <label>B2 Application Key
          <input type="password" bind:value={formSecretKey} />
        </label>
      {:else if formType === 'sftp'}
        <label>SFTP Repository URL
          <input bind:value={formPathOrUrl} placeholder="sftp:user@host:/path/to/repo" />
        </label>
      {:else if formType === 'rclone'}
        <label>rclone Repository URL
          <input bind:value={formPathOrUrl} placeholder="rclone:remote:path/to/repo" />
        </label>
        <p class="desc-text">
          Uses rclone configured on the remote server. Format is
          <span class="mono-val">rclone:&lt;remote&gt;:&lt;path&gt;</span>.
          Configure remotes there with <span class="mono-val">rclone config</span>.
        </p>
        <div class="rclone-detect-row">
          <button
            type="button"
            class="secondary btn-compact"
            onclick={detectRcloneRemotes}
            disabled={isLoadingRemotes}
          >
            {#if isLoadingRemotes}
              <Loader2 size={12} class="spin" /> Detecting...
            {:else}
              <Search size={12} /> Detect remotes
            {/if}
          </button>
        </div>
        {#if rcloneError}
          <p class="rclone-error"><AlertCircle size={12} /> {rcloneError}</p>
        {:else if rcloneRemotes.length > 0}
          <div class="rclone-remotes">
            {#each rcloneRemotes as remote}
              <button
                type="button"
                class="remote-chip"
                onclick={() => applyRcloneRemote(remote)}
              >
                {remote}:
              </button>
            {/each}
          </div>
        {/if}
      {:else}
        <label>REST Server URL
          <input bind:value={formPathOrUrl} placeholder="rest:http://user:pass@host:8000/" />
        </label>
      {/if}

      <label>Repository Password
        <input type="password" bind:value={formPassword} placeholder="Enter encryption key" />
      </label>

      <div class="row-checkbox">
        <input type="checkbox" id="form-use-sudo" bind:checked={formUseSudo} />
        <label for="form-use-sudo">Run commands with sudo (root permissions)</label>
      </div>

      <div class="advanced-section">
        <label>Custom Environment Variables (Key=Value, one per line)
          <textarea bind:value={formEnvVarsText} rows="3" placeholder="RESTIC_COMPRESSION=max"></textarea>
        </label>
      </div>

      <div class="modal-actions">
        <button class="secondary" onclick={() => showConfigModal = false}>Cancel</button>
        <button class="primary" onclick={saveRepo}>Save</button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal: Create Backup -->
{#if showBackupModal}
  <div class="modal-overlay" role="presentation" onclick={() => showBackupModal = false}>
    <div class="modal glass" role="dialog" onclick={(e) => e.stopPropagation()}>
      <h3>Create Backup Snapshot</h3>
      <p class="desc-text">Perform a manual backup of remote paths into this repository.</p>
      
      <label>Paths to Backup (comma-separated list)
        <PathAutocomplete bind:value={backupPath} placeholder="/var/www, /etc/nginx" onlyDirs={false} />
      </label>

      <label>Tags (optional, comma-separated)
        <input bind:value={backupTagsText} placeholder="web, manual" />
      </label>

      <label>Exclude Paths (optional, comma-separated)
        <input bind:value={backupExcludesText} placeholder="*.log, /var/www/cache" />
      </label>

      <div class="row-checkbox">
        <input type="checkbox" id="backup-use-sudo" bind:checked={backupUseSudo} />
        <label for="backup-use-sudo">Run backup with sudo</label>
      </div>

      <div class="modal-actions">
        <button class="secondary" onclick={() => showBackupModal = false}>Cancel</button>
        <button class="primary" onclick={runBackup}>Run Backup</button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal: Restore Snapshot -->
{#if showRestoreModal}
  <div class="modal-overlay" role="presentation" onclick={() => showRestoreModal = false}>
    <div class="modal glass" role="dialog" onclick={(e) => e.stopPropagation()}>
      <h3>Restore Snapshot</h3>
      <p class="desc-text">Restore snapshot <span class="mono-val text-amber">{restoreSnapshotId.slice(0, 8)}</span> files to the remote server.</p>
      
      <label>Destination Target Path (remote directory)
        <PathAutocomplete bind:value={restoreTargetPath} placeholder="/tmp/restore-target" onlyDirs={true} />
      </label>

      <div class="row-checkbox">
        <input type="checkbox" id="restore-use-sudo" bind:checked={restoreUseSudo} />
        <label for="restore-use-sudo">Run restore with sudo</label>
      </div>

      <div class="modal-actions">
        <button class="secondary" onclick={() => showRestoreModal = false}>Cancel</button>
        <button class="primary" onclick={runRestore}>Run Restore</button>
      </div>
    </div>
  </div>
{/if}

<!-- Modal: Stream Logs Console -->
{#if showLogsModal}
  <div class="modal-overlay console-overlay" role="presentation" onclick={() => { if (!isConsoleRunning) showLogsModal = false; }}>
    <div class="modal glass console-modal" role="dialog" onclick={(e) => e.stopPropagation()}>
      <div class="modal-header-row">
        <h3>{consoleTitle}</h3>
        <button class="close-btn" disabled={isConsoleRunning} onclick={() => showLogsModal = false}>
          <X size={16} />
        </button>
      </div>

      <div class="console-box">
        <pre class="scrollable"><code>{consoleOutput}</code></pre>
      </div>

      <div class="modal-actions-row">
        {#if isConsoleRunning}
          <span class="status-running">
            <Loader2 size={12} class="spin" /> Executing command...
          </span>
        {:else}
          <button class="primary" onclick={() => showLogsModal = false}>Close Console</button>
        {/if}
      </div>
    </div>
  </div>
{/if}

<SudoModal
  bind:open={showSudoModal}
  onSuccess={() => {
    if (pendingAction) {
      const action = pendingAction;
      pendingAction = null;
      action();
    }
  }}
/>

<style>
  .version-tag {
    font-size: 0.72rem;
    font-weight: normal;
    color: var(--text-muted);
    background: rgba(255, 255, 255, 0.05);
    padding: 2px 6px;
    border-radius: var(--radius-sm);
    margin-left: 8px;
    font-family: var(--font-mono);
  }

  .warning-container {
    padding: 40px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
    text-align: center;
    border-radius: var(--radius-md);
    margin-top: 20px;
  }

  .warning-container h3 {
    margin: 0;
    color: white;
  }

  .restic-grid {
    display: grid;
    grid-template-columns: 240px 1fr;
    gap: 16px;
    margin-top: 10px;
    height: calc(100vh - 120px);
  }

  .repo-selector {
    padding: 16px;
    border-radius: var(--radius-md);
    display: flex;
    flex-direction: column;
    gap: 12px;
    overflow: hidden;
  }

  .repo-selector h4 {
    margin: 0 0 4px 0;
    color: white;
    font-size: 0.85rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .repo-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    overflow-y: auto;
    flex: 1;
  }

  .repo-item {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: var(--radius-sm);
    padding: 10px;
    text-align: left;
    cursor: pointer;
    transition: all 0.15s ease;
    display: flex;
    flex-direction: column;
    gap: 4px;
    width: 100%;
  }

  .repo-item:hover {
    background: var(--bg-hover);
    border-color: rgba(255, 255, 255, 0.1);
  }

  .repo-item.active {
    background: var(--accent-muted);
    border-color: var(--accent-primary);
  }

  .repo-item-header {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .repo-icon {
    flex-shrink: 0;
  }

  .repo-name {
    font-weight: 600;
    color: white;
    font-size: 0.82rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .repo-path {
    font-family: var(--font-mono);
    font-size: 0.65rem;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .repo-dashboard {
    display: flex;
    flex-direction: column;
    gap: 16px;
    overflow: hidden;
  }

  .dashboard-header {
    padding: 16px;
    border-radius: var(--radius-md);
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
  }

  .repo-info-card h3 {
    margin: 0 0 6px 0;
    color: white;
    font-size: 1.1rem;
  }

  .repo-meta {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .repo-url {
    font-size: 0.72rem;
    color: var(--text-secondary);
  }

  .dashboard-actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .snapshots-container {
    flex: 1;
    border-radius: var(--radius-md);
    padding: 16px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .section-title-bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
  }

  .section-title-bar h4 {
    margin: 0;
    color: white;
    font-size: 0.9rem;
    font-weight: 600;
  }

  .loading-state, .empty-state, .error-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    text-align: center;
    padding: 40px 20px;
    background: rgba(0, 0, 0, 0.1);
    border-radius: var(--radius-sm);
  }

  .error-state p {
    color: var(--accent-red);
    font-size: 0.85rem;
    max-width: 400px;
    word-break: break-word;
  }

  .error-actions {
    display: flex;
    gap: 8px;
    margin-top: 4px;
  }

  .empty-state p {
    color: var(--text-muted);
    font-size: 0.85rem;
  }

  .table-wrapper {
    flex: 1;
    overflow: auto;
    background: rgba(0, 0, 0, 0.15);
    border-radius: var(--radius-sm);
    border: 1px solid rgba(255, 255, 255, 0.05);
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.8rem;
    text-align: left;
  }

  th, td {
    padding: 10px 12px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }

  th {
    background: rgba(0, 0, 0, 0.2);
    color: white;
    font-weight: 600;
    position: sticky;
    top: 0;
    z-index: 10;
  }

  tr:hover td {
    background: rgba(255, 255, 255, 0.01);
  }

  .paths-cell {
    max-width: 250px;
    overflow: hidden;
  }

  .path-tag {
    display: inline-block;
    font-size: 0.7rem;
    background: rgba(255, 255, 255, 0.05);
    padding: 2px 6px;
    border-radius: var(--radius-sm);
    margin: 1px;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    vertical-align: middle;
  }

  .tag-badge {
    display: inline-block;
    font-size: 0.65rem;
    background: var(--accent-muted);
    color: var(--accent-primary);
    border: 1px solid rgba(139, 92, 246, 0.2);
    padding: 1px 5px;
    border-radius: var(--radius-sm);
    margin: 1px;
  }

  .actions-col {
    width: 100px;
    text-align: right;
  }

  .row-actions {
    display: flex;
    justify-content: flex-end;
    gap: 4px;
  }

  .action-btn {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: var(--radius-sm);
    width: 26px;
    height: 26px;
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--text-secondary);
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .action-btn:hover {
    background: var(--bg-hover);
    color: white;
  }

  .action-btn.hover-red:hover {
    background: var(--accent-red-glow);
    border-color: rgba(239, 68, 68, 0.2);
    color: var(--accent-red) !important;
  }

  /* Modals */
  .modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.65);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .modal {
    width: 460px;
    max-height: 86vh;
    overflow-y: auto;
    padding: 20px;
    border-radius: var(--radius-md);
    display: flex;
    flex-direction: column;
    gap: 12px;
    box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.5);
  }

  .modal h3 {
    color: white;
    font-size: 1.05rem;
    margin: 0 0 4px 0;
  }

  .desc-text {
    font-size: 0.78rem;
    color: var(--text-secondary);
    margin: -4px 0 4px 0;
  }

  .modal label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 0.8rem;
    color: var(--text-secondary);
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 8px;
  }

  .row-checkbox {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 2px 0;
  }

  .row-checkbox label {
    font-size: 0.78rem;
    cursor: pointer;
    user-select: none;
  }

  .advanced-section {
    margin-top: 4px;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    padding-top: 10px;
  }

  .advanced-section textarea {
    font-family: var(--font-mono);
    font-size: 0.72rem;
    line-height: 1.4;
  }

  .modal-header-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .close-btn {
    background: transparent;
    border: none;
    cursor: pointer;
    color: var(--text-muted);
    padding: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
  }

  .close-btn:hover {
    background: rgba(255, 255, 255, 0.05);
    color: white;
  }

  .search-bar-row {
    margin: 4px 0 8px 0;
  }

  .search-input-wrapper {
    position: relative;
    display: flex;
    align-items: center;
  }

  .search-icon {
    position: absolute;
    left: 10px;
    color: var(--text-muted);
    pointer-events: none;
  }

  .search-input-wrapper input {
    width: 100%;
    padding-left: 32px;
  }

  .files-list-container {
    height: 400px;
    background: rgba(0, 0, 0, 0.15);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: var(--radius-sm);
    display: flex;
    flex-direction: column;
    position: relative;
  }

  /* When the browser is rendered inline it fills the snapshots panel. */
  .files-list-container.inline-files {
    height: auto;
    flex: 1;
    min-height: 0;
  }

  .browse-heading {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .browse-heading h4 {
    margin: 0;
    color: white;
    font-size: 0.9rem;
    font-weight: 600;
  }

  /* Inline file preview */
  .inline-preview {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .preview-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
  }

  .preview-file-info {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
  }

  .preview-name-block {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .preview-name {
    font-size: 0.82rem;
    font-weight: 600;
  }

  .preview-path {
    font-size: 0.68rem;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .preview-actions-inline {
    display: flex;
    gap: 8px;
    flex-shrink: 0;
  }

  .preview-textarea.inline {
    flex: 1;
    min-height: 0;
    height: auto;
  }

  .files-table-wrapper {
    flex: 1;
    overflow: auto;
  }

  .file-type-badge {
    font-size: 0.6rem;
    font-weight: 800;
    padding: 1px 4px;
    border-radius: var(--radius-sm);
    background: rgba(255, 255, 255, 0.08);
    color: var(--text-secondary);
  }

  .file-type-badge.dir {
    background: rgba(245, 158, 11, 0.15);
    color: var(--accent-amber);
  }

  .path-val {
    word-break: break-all;
    font-size: 0.72rem;
  }

  .breadcrumbs-bar {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 6px 12px;
    margin-bottom: 12px;
    border-radius: var(--radius-md);
    font-size: 0.8rem;
    overflow-x: auto;
    white-space: nowrap;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.05);
  }
  
  .breadcrumb-item {
    background: none;
    border: none;
    color: var(--accent-color, #3b82f6);
    cursor: pointer;
    padding: 2px 6px;
    border-radius: var(--radius-sm);
    font-weight: 500;
  }
  
  .breadcrumb-item:hover {
    background: rgba(255, 255, 255, 0.05);
    text-decoration: underline;
  }
  
  .separator {
    color: var(--text-muted);
    user-select: none;
  }

  .clickable-row {
    cursor: pointer;
  }
  
  .clickable-row:hover {
    background: rgba(255, 255, 255, 0.03) !important;
  }

  .dir-icon {
    color: var(--accent-amber);
    margin-right: 6px;
    display: inline-block;
    vertical-align: middle;
  }

  .text-right {
    text-align: right;
  }

  .limit-hint {
    padding: 8px 12px;
    font-size: 0.7rem;
    color: var(--text-muted);
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    background: rgba(0, 0, 0, 0.1);
  }

  /* Console Modal Styles */
  .console-overlay {
    background: rgba(0, 0, 0, 0.75);
  }

  .console-modal {
    width: 720px;
    max-width: 90vw;
  }

  .console-box {
    background: rgba(0, 0, 0, 0.9);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: var(--radius-sm);
    padding: 14px;
    height: 350px;
    overflow: hidden;
  }

  .console-box pre {
    height: 100%;
    margin: 0;
    overflow-y: auto;
    font-family: var(--font-mono);
    font-size: 0.75rem;
    line-height: 1.5;
    color: #34d399; /* Green prompt color */
    white-space: pre-wrap;
    word-break: break-all;
  }

  .modal-actions-row {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    margin-top: 6px;
  }

  .status-running {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 0.75rem;
    color: var(--accent-amber);
  }

  .rclone-detect-row {
    display: flex;
    justify-content: flex-start;
  }

  .rclone-error {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.72rem;
    color: var(--accent-red);
    margin: 0;
  }

  .rclone-remotes {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .remote-chip {
    font-family: var(--font-mono);
    font-size: 0.72rem;
    background: var(--accent-muted);
    color: var(--accent-primary);
    border: 1px solid rgba(139, 92, 246, 0.2);
    padding: 3px 8px;
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .remote-chip:hover {
    background: var(--bg-hover);
    color: white;
  }

  .actions-cell {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  
  .preview-textarea {
    width: 100%;
    height: 400px;
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: var(--radius-sm);
    color: #e5e7eb;
    font-family: monospace;
    font-size: 0.8rem;
    padding: 12px;
    resize: none;
    overflow-y: auto;
    white-space: pre;
  }
</style>
