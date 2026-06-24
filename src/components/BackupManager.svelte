<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import {
    Plus, Trash2, Play, Database, FolderArchive, Loader2, Download, Clock, FileText, X,
  } from 'lucide-svelte';
  import SudoModal from './SudoModal.svelte';
  import PathAutocomplete from './ui/PathAutocomplete.svelte';
  import CronExpressionInput from '$lib/CronExpressionInput.svelte';
  import type { BackupTemplate, ProfileExtras, ResticRepo } from '$lib/admin/types';
  import { DEFAULT_PROFILE_EXTRAS } from '$lib/admin/types';
  import { shellQuote as q, resticEnvExports } from '$lib/restic/env';
  import { get } from 'svelte/store';
    import { notifications } from '$lib/notifications.svelte';
  import {
    formatInvokeError,
    isSudoPasswordRequired,
  } from '$lib/backendErrors';

  let { profileId = '', visible = true } = $props();

  let extras = $state<ProfileExtras>({ ...DEFAULT_PROFILE_EXTRAS });
  let isLoading = $state(false);
  let isRunning = $state(false);

  export function refresh() { loadExtras(); }

  let showAddModal = $state(false);
  let editId = $state<string | null>(null);
  let formName = $state('');
  let formType = $state<'files' | 'mysql' | 'postgres'>('files');
  let formPath = $state('/var/www');
  let formContainer = $state('');
  let formDbName = $state('');
  let formDbUser = $state('root');
  let formDbPassword = $state('');
  // Off-site destination
  let formDestination = $state<'download' | 's3' | 'sftp' | 'restic'>('download');
  let formDestEndpoint = $state('');
  let formDestRegion = $state('');
  let formDestBucket = $state('');
  let formDestPath = $state('');
  let formDestHost = $state('');
  let formDestPort = $state('22');
  let formDestUser = $state('');
  let formDestAccessKey = $state('');
  let formDestSecretKey = $state('');
  // Restic engine + scheduling + retention
  let formResticRepoId = $state('');
  let formScheduleEnabled = $state(false);
  let formScheduleCron = $state('0 2 * * *');
  let formKeepLast = $state('');
  let formKeepDaily = $state('');
  let formKeepWeekly = $state('');
  let formKeepMonthly = $state('');
  let formRetentionDays = $state('');

  // Log viewer (schedule logs)
  let showLogModal = $state(false);
  let logContent = $state('');
  let logTitle = $state('');

  // Live console for manual runs
  let showConsole = $state(false);
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

  async function loadExtras() {
    if (!profileId) return;
    isLoading = true;
    try {
      extras = await invoke<ProfileExtras>('get_profile_extras', { profileId });
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
    editId = null;
    formName = '';
    formType = 'files';
    formPath = '/var/www';
    formContainer = '';
    formDbName = '';
    formDbUser = 'root';
    formDbPassword = '';
    formDestination = 'download';
    formDestEndpoint = '';
    formDestRegion = '';
    formDestBucket = '';
    formDestPath = '';
    formDestHost = '';
    formDestPort = '22';
    formDestUser = '';
    formDestAccessKey = '';
    formDestSecretKey = '';
    formResticRepoId = extras.restic_repos[0]?.id || '';
    formScheduleEnabled = false;
    formScheduleCron = '0 2 * * *';
    formKeepLast = '';
    formKeepDaily = '';
    formKeepWeekly = '';
    formKeepMonthly = '';
    formRetentionDays = '';
    showAddModal = true;
  }

  function openEdit(t: BackupTemplate) {
    editId = t.id;
    formName = t.name;
    formType = t.backup_type;
    formPath = t.source_path;
    formContainer = t.docker_container || '';
    formDbName = t.db_name || '';
    formDbUser = t.db_user || 'root';
    formDbPassword = t.db_password || '';
    formDestination = (t.destination as any) || 'download';
    formDestEndpoint = t.dest_endpoint || '';
    formDestRegion = t.dest_region || '';
    formDestBucket = t.dest_bucket || '';
    formDestPath = t.dest_path || '';
    formDestHost = t.dest_host || '';
    formDestPort = t.dest_port || '22';
    formDestUser = t.dest_user || '';
    formDestAccessKey = t.dest_access_key || '';
    formDestSecretKey = t.dest_secret_key || '';
    formResticRepoId = t.restic_repo_id || extras.restic_repos[0]?.id || '';
    formScheduleEnabled = !!t.schedule_enabled;
    formScheduleCron = t.schedule_cron || '0 2 * * *';
    formKeepLast = t.keep_last != null ? String(t.keep_last) : '';
    formKeepDaily = t.keep_daily != null ? String(t.keep_daily) : '';
    formKeepWeekly = t.keep_weekly != null ? String(t.keep_weekly) : '';
    formKeepMonthly = t.keep_monthly != null ? String(t.keep_monthly) : '';
    formRetentionDays = t.retention_days != null ? String(t.retention_days) : '';
    showAddModal = true;
  }

  function toInt(s: string): number | null {
    const n = parseInt(s.trim(), 10);
    return Number.isFinite(n) && n > 0 ? n : null;
  }

  async function saveTemplate() {
    if (!formName.trim()) {
      alert("Enter template name");
      return;
    }
    // A cronjob runs while the desktop app is closed, so it cannot download to
    // this computer — scheduled backups need a server-side/offsite destination.
    if (formScheduleEnabled && formDestination === 'download') {
      alert("Scheduled backups can't download to this computer. Pick S3, SFTP or a Restic repository.");
      return;
    }
    if (formDestination === 'restic' && !formResticRepoId) {
      alert("Select a Restic repository (configure one in the Restic tab first).");
      return;
    }
    const tpl: BackupTemplate = {
      id: editId || Date.now().toString(),
      name: formName.trim(),
      backup_type: formType,
      source_path: formPath.trim(),
      docker_container: formContainer.trim() || null,
      db_name: formDbName.trim() || null,
      db_user: formDbUser.trim() || null,
      db_password: formDbPassword.trim() || null,
      destination: formDestination,
      dest_endpoint: formDestEndpoint.trim() || null,
      dest_region: formDestRegion.trim() || null,
      dest_bucket: formDestBucket.trim() || null,
      dest_path: formDestPath.trim() || null,
      dest_host: formDestHost.trim() || null,
      dest_port: formDestPort.trim() || null,
      dest_user: formDestUser.trim() || null,
      dest_access_key: formDestAccessKey.trim() || null,
      dest_secret_key: formDestSecretKey.trim() || null,
      restic_repo_id: formDestination === 'restic' ? formResticRepoId : null,
      schedule_enabled: formScheduleEnabled,
      schedule_cron: formScheduleEnabled ? formScheduleCron.trim() : null,
      keep_last: toInt(formKeepLast),
      keep_daily: toInt(formKeepDaily),
      keep_weekly: toInt(formKeepWeekly),
      keep_monthly: toInt(formKeepMonthly),
      retention_days: toInt(formRetentionDays),
    };
    const prev = extras.backup_templates.find((t) => t.id === editId);
    if (editId) {
      extras.backup_templates = extras.backup_templates.map((t) => (t.id === editId ? tpl : t));
    } else {
      extras.backup_templates = [...extras.backup_templates, tpl];
    }
    await saveExtras();
    showAddModal = false;
    await syncSchedule(tpl, !!prev?.schedule_enabled);
  }

  // Install or remove the remote cronjob to match the template's schedule.
  async function syncSchedule(tpl: BackupTemplate, wasScheduled: boolean) {
    if (tpl.schedule_enabled) {
      const repo = tpl.destination === 'restic'
        ? extras.restic_repos.find((r) => r.id === tpl.restic_repo_id)
        : undefined;
      const scriptBody = buildScriptBody(tpl, repo, false);
      const envContent = buildEnvContent(tpl, repo);
      await withSudo(async () => {
        await invoke('install_backup_schedule', {
          templateId: tpl.id,
          cron: tpl.schedule_cron,
          scriptBody,
          envContent,
        });
        notifications.success(`Schedule installed (${tpl.schedule_cron}).`);
      });
    } else if (wasScheduled) {
      await withSudo(async () => {
        await invoke('uninstall_backup_schedule', { templateId: tpl.id });
        notifications.success("Schedule removed.");
      });
    }
  }

  async function deleteTemplate(id: string) {
    if (!confirm("Delete backup template?")) return;
    const tpl = extras.backup_templates.find((t) => t.id === id);
    if (tpl?.schedule_enabled) {
      await withSudo(async () => {
        await invoke('uninstall_backup_schedule', { templateId: id });
      });
    }
    extras.backup_templates = extras.backup_templates.filter((t) => t.id !== id);
    await saveExtras();
  }

  function shellQuote(s: string): string {
    return "'" + s.replace(/'/g, "'\\''") + "'";
  }

  function buildBackupCmd(t: BackupTemplate, remotePath: string): string {
    if (t.backup_type === 'files') {
      const dir = t.source_path.replace(/\/$/, '');
      return `tar czf ${shellQuote(remotePath)} -C ${shellQuote(dir)} . 2>&1`;
    }
    if (t.backup_type === 'mysql') {
      const db = t.db_name || 'mysql';
      const user = t.db_user || 'root';
      const pass = t.db_password ? `-p${shellQuote(t.db_password)}` : '';
      if (t.docker_container) {
        return `docker exec ${shellQuote(t.docker_container)} sh -c ${shellQuote(`mysqldump -u ${user} ${pass} ${db}`)} > ${shellQuote(remotePath)} 2>&1`;
      }
      return `mysqldump -u ${shellQuote(user)} ${pass} ${shellQuote(db)} > ${shellQuote(remotePath)} 2>&1`;
    }
    const db = t.db_name || 'postgres';
    const user = t.db_user || 'postgres';
    if (t.docker_container) {
      return `docker exec ${shellQuote(t.docker_container)} pg_dump -U ${shellQuote(user)} ${shellQuote(db)} > ${shellQuote(remotePath)} 2>&1`;
    }
    return `pg_dump -U ${shellQuote(user)} ${shellQuote(db)} > ${shellQuote(remotePath)} 2>&1`;
  }

  function buildRclonePush(t: BackupTemplate, remotePath: string, destFile: string): string {
    const dest = t.destination || 'download';
    if (dest === 's3') {
      const provider = t.dest_endpoint ? 'Other' : 'AWS';
      const envs = [
        `RCLONE_S3_PROVIDER=${shellQuote(provider)}`,
        `RCLONE_S3_ACCESS_KEY_ID=${shellQuote(t.dest_access_key || '')}`,
        `RCLONE_S3_SECRET_ACCESS_KEY=${shellQuote(t.dest_secret_key || '')}`,
      ];
      if (t.dest_endpoint) envs.push(`RCLONE_S3_ENDPOINT=${shellQuote(t.dest_endpoint)}`);
      if (t.dest_region) envs.push(`RCLONE_S3_REGION=${shellQuote(t.dest_region)}`);
      const sub = (t.dest_path || '').replace(/^\/+|\/+$/g, '');
      const target = `:s3:${t.dest_bucket}${sub ? '/' + sub : ''}/${destFile}`;
      return `${envs.join(' ')} rclone copyto ${shellQuote(remotePath)} ${shellQuote(target)} 2>&1`;
    }
    // sftp
    const envs = [
      `RCLONE_SFTP_HOST=${shellQuote(t.dest_host || '')}`,
      `RCLONE_SFTP_USER=${shellQuote(t.dest_user || '')}`,
      `RCLONE_SFTP_PORT=${shellQuote(t.dest_port || '22')}`,
      `RCLONE_SFTP_PASS="$(rclone obscure ${shellQuote(t.dest_secret_key || '')})"`,
    ];
    const sub = (t.dest_path || '').replace(/^\/+$/, '');
    const target = `:sftp:${sub ? sub.replace(/\/+$/, '') + '/' : ''}${destFile}`;
    return `${envs.join(' ')} rclone copyto ${shellQuote(remotePath)} ${shellQuote(target)} 2>&1`;
  }

  function safeName(t: BackupTemplate): string {
    return (t.name || 'backup').replace(/[^a-zA-Z0-9_-]/g, '_');
  }

  // Dump command for a script, writing into the shell variable "$TMP".
  function buildDumpToVar(t: BackupTemplate): string {
    if (t.backup_type === 'files') {
      const dir = t.source_path.replace(/\/$/, '');
      return `tar czf "$TMP" -C ${q(dir)} .`;
    }
    if (t.backup_type === 'mysql') {
      const db = t.db_name || 'mysql';
      const user = t.db_user || 'root';
      const pass = t.db_password ? `-p${q(t.db_password)}` : '';
      if (t.docker_container) {
        return `docker exec ${q(t.docker_container)} sh -c ${q(`mysqldump -u ${user} ${pass} ${db}`)} > "$TMP"`;
      }
      return `mysqldump -u ${q(user)} ${pass} ${q(db)} > "$TMP"`;
    }
    const db = t.db_name || 'postgres';
    const user = t.db_user || 'postgres';
    if (t.docker_container) {
      return `docker exec ${q(t.docker_container)} pg_dump -U ${q(user)} ${q(db)} > "$TMP"`;
    }
    return `pg_dump -U ${q(user)} ${q(db)} > "$TMP"`;
  }

  // Native restic retention policy → `forget --keep-* --prune` (empty if unset).
  function buildResticForget(t: BackupTemplate, repo: ResticRepo): string {
    const flags: string[] = [];
    if (t.keep_last) flags.push(`--keep-last ${t.keep_last}`);
    if (t.keep_daily) flags.push(`--keep-daily ${t.keep_daily}`);
    if (t.keep_weekly) flags.push(`--keep-weekly ${t.keep_weekly}`);
    if (t.keep_monthly) flags.push(`--keep-monthly ${t.keep_monthly}`);
    if (!flags.length) return '';
    return `restic -r ${q(repo.path_or_url)} forget ${flags.join(' ')} --tag jarvis --prune`;
  }

  // Secrets env content (restic password / S3 / SFTP creds) as `export` lines.
  function buildEnvContent(t: BackupTemplate, repo?: ResticRepo): string {
    if (t.destination === 'restic') {
      return repo ? resticEnvExports(repo) + '\n' : '';
    }
    if (t.destination === 's3') {
      const provider = t.dest_endpoint ? 'Other' : 'AWS';
      const lines = [
        `export RCLONE_S3_PROVIDER=${q(provider)}`,
        `export RCLONE_S3_ACCESS_KEY_ID=${q(t.dest_access_key || '')}`,
        `export RCLONE_S3_SECRET_ACCESS_KEY=${q(t.dest_secret_key || '')}`,
      ];
      if (t.dest_endpoint) lines.push(`export RCLONE_S3_ENDPOINT=${q(t.dest_endpoint)}`);
      if (t.dest_region) lines.push(`export RCLONE_S3_REGION=${q(t.dest_region)}`);
      return lines.join('\n') + '\n';
    }
    if (t.destination === 'sftp') {
      const lines = [
        `export RCLONE_SFTP_HOST=${q(t.dest_host || '')}`,
        `export RCLONE_SFTP_USER=${q(t.dest_user || '')}`,
        `export RCLONE_SFTP_PORT=${q(t.dest_port || '22')}`,
        `export RCLONE_SFTP_PASS="$(rclone obscure ${q(t.dest_secret_key || '')})"`,
      ];
      return lines.join('\n') + '\n';
    }
    return '';
  }

  // Backup commands assuming the env vars above are already in the environment.
  function buildScriptCore(t: BackupTemplate, repo?: ResticRepo): string {
    const name = safeName(t);
    if (t.destination === 'restic') {
      if (!repo) return 'echo "Restic repository not found" >&2; exit 1';
      const r = q(repo.path_or_url);
      const lines: string[] = [];
      if (t.backup_type === 'files') {
        const dir = t.source_path.replace(/\/$/, '');
        lines.push(`restic -r ${r} backup ${q(dir)} --tag jarvis`);
      } else {
        lines.push(`TMP="/tmp/jarvis-backup-$(date +%s).sql"`);
        lines.push(buildDumpToVar(t));
        lines.push(`restic -r ${r} backup "$TMP" --tag jarvis`);
        lines.push(`rm -f "$TMP"`);
      }
      const forget = buildResticForget(t, repo);
      if (forget) lines.push(forget);
      return lines.join('\n');
    }
    // s3 / sftp via rclone
    const ext = t.backup_type === 'files' ? 'tar.gz' : 'sql';
    const lines: string[] = [];
    lines.push(`TS="$(date +%Y%m%d-%H%M%S)"`);
    lines.push(`TMP="/tmp/jarvis-backup-$TS.${ext}"`);
    lines.push(buildDumpToVar(t));
    const sub = (t.dest_path || '').replace(/^\/+|\/+$/g, '');
    const destBase = t.destination === 's3'
      ? `:s3:${t.dest_bucket}${sub ? '/' + sub : ''}`
      : `:sftp:${sub}`;
    lines.push(`DEST=${q(destBase)}`);
    lines.push(`rclone copyto "$TMP" "$DEST/${name}-$TS.${ext}"`);
    lines.push(`rm -f "$TMP"`);
    // Age-based retention. Skip for SFTP without an explicit sub-path so we
    // never run a delete against the whole login home directory.
    if (t.retention_days && (t.destination === 's3' || sub)) {
      lines.push(`rclone delete "$DEST" --min-age ${t.retention_days}d`);
    }
    return lines.join('\n');
  }

  // Full script. `inlineEnv` embeds secrets (manual run); otherwise the script
  // sources the installed /etc/jarvis-backups/<id>.env file (scheduled run).
  function buildScriptBody(t: BackupTemplate, repo: ResticRepo | undefined, inlineEnv: boolean): string {
    const lines = ['#!/bin/sh', 'set -e'];
    if (inlineEnv) {
      const env = buildEnvContent(t, repo);
      if (env.trim()) lines.push(env.trimEnd());
    } else {
      lines.push(`. /etc/jarvis-backups/${t.id}.env`);
    }
    lines.push(buildScriptCore(t, repo));
    return lines.join('\n') + '\n';
  }

  // Run a command on the server, streaming stdout/stderr into the live console.
  // Returns true on success. Re-throws SUDO_PASSWORD_REQUIRED so the caller's
  // withSudo() wrapper can prompt and retry.
  async function runStream(title: string, cmd: string, useSudo: boolean): Promise<boolean> {
    consoleTitle = title;
    consoleOutput = '';
    showConsole = true;
    isConsoleRunning = true;
    const eventId = Math.random().toString(36).slice(2);
    const unStdout = await listen<string>(`exec-stdout-${eventId}`, (e) => (consoleOutput += e.payload));
    const unStderr = await listen<string>(`exec-stderr-${eventId}`, (e) => (consoleOutput += e.payload));
    try {
      await invoke('exec_custom_command_stream', { cmd, useSudo, eventId });
      consoleOutput += '\n✓ Done.\n';
      return true;
    } catch (err: unknown) {
      if (isSudoPasswordRequired(err)) {
        showConsole = false;
        throw err;
      }
      consoleOutput += `\n✗ Error: ${formatInvokeError(err)}\n`;
      return false;
    } finally {
      unStdout();
      unStderr();
      isConsoleRunning = false;
    }
  }

  async function runBackup(t: BackupTemplate) {
    isRunning = true;
    const dest = t.destination || 'download';

    // Download is interactive-only: archive on server, then pull to this PC.
    if (dest === 'download') {
      const ext = t.backup_type === 'files' ? 'tar.gz' : 'sql';
      const isFiles = t.backup_type === 'files';
      const remotePath = `/tmp/jarvis-backup-${Date.now()}.${ext}`;
      await withSudo(async () => {
        try {
          const ok = await runStream(`Backup: ${t.name}`, buildBackupCmd(t, remotePath), isFiles);
          if (!ok) return;
          if (isFiles) await exec(`chmod 644 ${remotePath}`, true).catch(() => {});
          const count = await invoke<number>('sftp_start_download_batch', {
            remotePaths: [remotePath],
            localDir: null,
          });
          consoleOutput += `\nStarted downloading ${count} file(s) to Downloads.\n`;
          await exec(`rm -f ${remotePath}`, isFiles).catch(() => {});
        } catch (err: unknown) {
          if (isSudoPasswordRequired(err)) throw err;
          notifications.error(`Backup error: ${formatInvokeError(err)}`);
        } finally {
          isRunning = false;
        }
      });
      isRunning = false;
      return;
    }

    // Server-side destinations: run the same script the scheduler would install.
    const repo = dest === 'restic'
      ? extras.restic_repos.find((r) => r.id === t.restic_repo_id)
      : undefined;
    if (dest === 'restic' && !repo) {
      notifications.error("Restic repository not found.");
      isRunning = false;
      return;
    }
    const needsSudo = t.backup_type === 'files' || (dest === 'restic' && !!repo?.use_sudo);
    const body = buildScriptBody(t, repo, true);
    await withSudo(async () => {
      try {
        const ok = await runStream(`Backup: ${t.name}`, body, needsSudo);
        if (ok) notifications.success(`Backup completed (${dest.toUpperCase()}).`);
      } catch (err: unknown) {
        if (isSudoPasswordRequired(err)) throw err;
        notifications.error(`Backup error: ${formatInvokeError(err)}`);
      } finally {
        isRunning = false;
      }
    });
    isRunning = false;
  }

  async function viewLog(t: BackupTemplate) {
    logTitle = t.name;
    logContent = 'Loading…';
    showLogModal = true;
    await withSudo(async () => {
      try {
        const out = await exec(`tail -n 200 /var/log/jarvis-backup-${t.id}.log 2>/dev/null || echo "(no log yet)"`, true);
        logContent = out.trim() || '(empty)';
      } catch (err: unknown) {
        if (isSudoPasswordRequired(err)) throw err;
        logContent = formatInvokeError(err);
      }
    });
  }

  onMount(loadExtras);

  $effect(() => {
    if (profileId) loadExtras();
  });
</script>

<div class="backup manager-shell scrollable fade-in">
  <header class="manager-header">
    <h1 class="page-title">Backups</h1>
    <div class="header-actions">
      <button class="primary btn-compact" onclick={openAdd}>
        <Plus size={14} /> New template
      </button>
    </div>
  </header>

  {#if extras.backup_templates.length === 0}
    <div class="empty glass">
      <FolderArchive size={36} class="muted" />
      <p>No backup templates. Create one for www directories or databases.</p>
      <button class="primary btn-compact" onclick={openAdd}><Plus size={14} /> Add template</button>
    </div>
  {:else}
    <div class="templates-grid">
      {#each extras.backup_templates as tpl}
        <div class="tpl-card glass">
          <div class="tpl-header">
            {#if tpl.backup_type === 'files'}
              <FolderArchive size={18} class="accent-amber-text" />
            {:else}
              <Database size={18} class="accent-green-text" />
            {/if}
            <span class="tpl-name">{tpl.name}</span>
            <span class="badge">{tpl.backup_type}</span>
            {#if tpl.destination && tpl.destination !== 'download'}
              <span class="badge success">{tpl.destination.toUpperCase()}</span>
            {/if}
            {#if tpl.schedule_enabled}
              <span class="badge sched"><Clock size={11} /> {tpl.schedule_cron}</span>
            {/if}
          </div>
          <div class="tpl-meta mono-val">
            {#if tpl.backup_type === 'files'}
              {tpl.source_path}
            {:else}
              {tpl.db_name || '—'} @ {tpl.docker_container || "host"}
            {/if}
          </div>
          <div class="tpl-actions">
            <button class="primary btn-compact" disabled={isRunning} onclick={() => runBackup(tpl)}>
              {#if isRunning}<Loader2 size={14} class="spin" />{:else}<Play size={14} />{/if}
              Run
            </button>
            <button class="secondary btn-compact" onclick={() => openEdit(tpl)}>Edit</button>
            {#if tpl.schedule_enabled}
              <button class="secondary btn-compact" onclick={() => viewLog(tpl)} title="View schedule log">
                <FileText size={14} />
              </button>
            {/if}
            <button class="secondary btn-compact hover-red" onclick={() => deleteTemplate(tpl.id)}>
              <Trash2 size={14} />
            </button>
          </div>
        </div>
      {/each}
    </div>
  {/if}

  <section class="info glass">
    <Download size={16} />
    <p>Backups can download here, push offsite (S3/SFTP), or snapshot to a Restic repo. Enable a schedule to run periodically via a server cronjob with automatic retention.</p>
  </section>
</div>

{#if showAddModal}
  <div class="modal-overlay" role="presentation" onclick={() => (showAddModal = false)}>
    <div class="modal glass" role="dialog" onclick={(e) => e.stopPropagation()}>
      <h3>{editId ? "Edit template" : "New backup template"}</h3>
      <label>Name<input bind:value={formName} placeholder="WWW backup" /></label>
      <label>Type
        <select bind:value={formType}>
          <option value="files">Files (tar.gz)</option>
          <option value="mysql">MySQL (mysqldump)</option>
          <option value="postgres">PostgreSQL (pg_dump)</option>
        </select>
      </label>
      {#if formType === 'files'}
        <label>Directory path
          <PathAutocomplete bind:value={formPath} placeholder="/var/www" onlyDirs={true} />
        </label>
      {:else}
        <label>Docker container (empty = host)<input bind:value={formContainer} placeholder="mysql" /></label>
        <label>Database name<input bind:value={formDbName} /></label>
        <label>DB user<input bind:value={formDbUser} /></label>
        <label>DB password (optional)<input type="password" bind:value={formDbPassword} /></label>
      {/if}

      <div class="dest-divider">Destination</div>
      <label>Send backup to
        <select bind:value={formDestination}>
          <option value="download">Download to this computer</option>
          <option value="s3">S3 / Backblaze B2 (S3-compatible)</option>
          <option value="sftp">Remote SFTP server</option>
          <option value="restic">Restic repository</option>
        </select>
      </label>
      {#if formDestination === 'restic'}
        {#if extras.restic_repos.length === 0}
          <p class="dest-hint">No Restic repositories configured. Add one in the Restic tab first.</p>
        {:else}
          <label>Restic repository
            <select bind:value={formResticRepoId}>
              {#each extras.restic_repos as r}
                <option value={r.id}>{r.name} ({r.repo_type})</option>
              {/each}
            </select>
          </label>
          <p class="dest-hint">Snapshots are written to the selected repository (tagged <code>jarvis</code>). Credentials come from the Restic tab's keyring.</p>
        {/if}
      {/if}
      {#if formDestination === 's3'}
        <label>Bucket<input bind:value={formDestBucket} placeholder="my-bucket" /></label>
        <label>Path / prefix<input bind:value={formDestPath} placeholder="backups/db" /></label>
        <label>Endpoint (leave empty for AWS S3)<input bind:value={formDestEndpoint} placeholder="s3.us-west-002.backblazeb2.com" /></label>
        <label>Region<input bind:value={formDestRegion} placeholder="us-east-1" /></label>
        <label>Access key ID<input bind:value={formDestAccessKey} /></label>
        <label>Secret access key<input type="password" bind:value={formDestSecretKey} /></label>
        <p class="dest-hint">Requires rclone installed on the server. Credentials are stored in the OS keyring.</p>
      {:else if formDestination === 'sftp'}
        <label>SFTP host<input bind:value={formDestHost} placeholder="backup.example.com" /></label>
        <label>Port<input bind:value={formDestPort} placeholder="22" /></label>
        <label>SFTP user<input bind:value={formDestUser} /></label>
        <label>DB password (optional)<input type="password" bind:value={formDestSecretKey} /></label>
        <label>Path / prefix<input bind:value={formDestPath} placeholder="/backups" /></label>
        <p class="dest-hint">Requires rclone installed on the server. Credentials are stored in the OS keyring.</p>
      {/if}

      <div class="dest-divider">Schedule</div>
      <label class="row-check">
        <input type="checkbox" bind:checked={formScheduleEnabled} disabled={formDestination === 'download'} />
        Run periodically via cronjob
      </label>
      {#if formDestination === 'download'}
        <p class="dest-hint">Scheduling needs a server-side destination (S3, SFTP or Restic) — a cronjob can't reach this computer.</p>
      {:else if formScheduleEnabled}
        <CronExpressionInput bind:value={formScheduleCron} />
        <p class="dest-hint">Installs a root cronjob and a 600-perm secrets file on the server.</p>
      {/if}

      <div class="dest-divider">Retention (auto-delete old backups)</div>
      {#if formDestination === 'restic'}
        <div class="retention-grid">
          <label>Keep last<input type="number" min="0" bind:value={formKeepLast} placeholder="—" /></label>
          <label>Keep daily<input type="number" min="0" bind:value={formKeepDaily} placeholder="—" /></label>
          <label>Keep weekly<input type="number" min="0" bind:value={formKeepWeekly} placeholder="—" /></label>
          <label>Keep monthly<input type="number" min="0" bind:value={formKeepMonthly} placeholder="—" /></label>
        </div>
        <p class="dest-hint">Applied via <code>restic forget --prune</code> after each backup. Leave empty to keep everything.</p>
      {:else if formDestination === 'download'}
        <p class="dest-hint">Retention isn't available for downloads.</p>
      {:else}
        <label>Delete backups older than (days)
          <input type="number" min="0" bind:value={formRetentionDays} placeholder="leave empty to keep all" />
        </label>
        <p class="dest-hint">Runs <code>rclone delete --min-age</code> on the destination after each backup.</p>
      {/if}

      <div class="modal-actions">
        <button class="secondary" onclick={() => (showAddModal = false)}>Cancel</button>
        <button class="primary" onclick={saveTemplate}>Save</button>
      </div>
    </div>
  </div>
{/if}

{#if showConsole}
  <div class="modal-overlay" role="presentation" onclick={() => { if (!isConsoleRunning) showConsole = false; }}>
    <div class="modal glass console-modal" role="dialog" onclick={(e) => e.stopPropagation()}>
      <div class="console-head">
        <h3>{consoleTitle}</h3>
        <button class="icon-btn" disabled={isConsoleRunning} onclick={() => (showConsole = false)}><X size={16} /></button>
      </div>
      <pre class="log-pre mono-val">{consoleOutput || '…'}</pre>
      <div class="modal-actions">
        {#if isConsoleRunning}
          <span class="run-status"><Loader2 size={13} class="spin" /> Running…</span>
        {:else}
          <button class="primary" onclick={() => (showConsole = false)}>Close</button>
        {/if}
      </div>
    </div>
  </div>
{/if}

{#if showLogModal}
  <div class="modal-overlay" role="presentation" onclick={() => (showLogModal = false)}>
    <div class="modal glass log-modal" role="dialog" onclick={(e) => e.stopPropagation()}>
      <h3>Schedule log — {logTitle}</h3>
      <pre class="log-pre mono-val">{logContent}</pre>
      <div class="modal-actions">
        <button class="secondary" onclick={() => (showLogModal = false)}>Close</button>
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
      isRunning = true;
      action();
    }
  }}
/>

<style>
  .header-actions { display: flex; gap: 8px; }
  .templates-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 10px; }
  .tpl-card { padding: 14px; border-radius: var(--radius-md); display: flex; flex-direction: column; gap: 10px; }
  .tpl-header { display: flex; align-items: center; gap: 8px; }
  .tpl-name { font-weight: 600; color: white; flex: 1; }
  .tpl-meta { font-size: 0.75rem; color: var(--text-muted); word-break: break-all; }
  .tpl-actions { display: flex; gap: 6px; flex-wrap: wrap; }
  .empty { padding: 40px; text-align: center; display: flex; flex-direction: column; align-items: center; gap: 12px; border-radius: var(--radius-md); }
  .empty .muted { color: var(--text-muted); }
  .info { padding: 12px; display: flex; gap: 10px; align-items: flex-start; font-size: 0.8rem; color: var(--text-secondary); border-radius: var(--radius-md); }
  .modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.6); display: flex; align-items: center; justify-content: center; z-index: 1000; }
  .modal { width: 420px; padding: 20px; border-radius: var(--radius-md); display: flex; flex-direction: column; gap: 10px; }
  .modal h3 { color: white; font-size: 1rem; }
  .modal label { display: flex; flex-direction: column; gap: 4px; font-size: 0.8rem; color: var(--text-secondary); }
  .modal-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 8px; }
  .hover-red:hover { color: var(--accent-red) !important; }
  .dest-divider { margin-top: 6px; padding-top: 10px; border-top: 1px solid var(--border-color); font-size: 0.72rem; text-transform: uppercase; letter-spacing: 0.05em; color: var(--text-muted); }
  .dest-hint { font-size: 0.72rem; color: var(--text-muted); margin: 2px 0; }
  .dest-hint code { font-family: var(--font-mono, monospace); }
  .modal { max-height: 86vh; overflow-y: auto; }
  .row-check { flex-direction: row !important; align-items: center; gap: 8px; }
  .row-check input { width: auto; }
  .retention-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; }
  .retention-grid label { min-width: 0; }
  .retention-grid input { width: 100%; box-sizing: border-box; min-width: 0; }
  .badge.sched { display: inline-flex; align-items: center; gap: 3px; background: rgba(99,102,241,0.18); color: #c7d2fe; }
  .log-modal, .console-modal { width: 640px; }
  .log-pre { max-height: 60vh; overflow: auto; white-space: pre-wrap; word-break: break-word; font-size: 0.75rem; background: rgba(0,0,0,0.35); padding: 10px; border-radius: var(--radius-sm); color: var(--text-secondary); }
  .console-head { display: flex; align-items: center; justify-content: space-between; }
  .icon-btn { background: none; border: none; color: var(--text-muted); cursor: pointer; padding: 2px; }
  .icon-btn:disabled { opacity: 0.4; cursor: default; }
  .run-status { display: inline-flex; align-items: center; gap: 6px; font-size: 0.8rem; color: var(--text-muted); }
</style>
