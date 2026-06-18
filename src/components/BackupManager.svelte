<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import {
    RefreshCw, Plus, Trash2, Play, Database, FolderArchive, Loader2, Download,
  } from 'lucide-svelte';
  import SudoModal from './SudoModal.svelte';
  import PathAutocomplete from './ui/PathAutocomplete.svelte';
  import type { BackupTemplate, ProfileExtras } from '$lib/admin/types';
  import { DEFAULT_PROFILE_EXTRAS } from '$lib/admin/types';
  import { get } from 'svelte/store';
  import { LL } from '$lib/i18n/i18n-svelte';
  import { notifications } from '$lib/notifications.svelte';
  import {
    formatInvokeError,
    isSudoPasswordRequired,
  } from '$lib/i18n/backendErrors';

  let { profileId = '' } = $props();

  let extras = $state<ProfileExtras>({ ...DEFAULT_PROFILE_EXTRAS });
  let isLoading = $state(false);
  let isRunning = $state(false);

  let showAddModal = $state(false);
  let editId = $state<string | null>(null);
  let formName = $state('');
  let formType = $state<'files' | 'mysql' | 'postgres'>('files');
  let formPath = $state('/var/www');
  let formContainer = $state('');
  let formDbName = $state('');
  let formDbUser = $state('root');
  let formDbPassword = $state('');

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
    showAddModal = true;
  }

  async function saveTemplate() {
    if (!formName.trim()) {
      alert(get(LL).backup.alertTemplateName());
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
    };
    if (editId) {
      extras.backup_templates = extras.backup_templates.map((t) => (t.id === editId ? tpl : t));
    } else {
      extras.backup_templates = [...extras.backup_templates, tpl];
    }
    await saveExtras();
    showAddModal = false;
  }

  async function deleteTemplate(id: string) {
    if (!confirm(get(LL).backup.confirmDeleteTemplate())) return;
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

  async function runBackup(t: BackupTemplate) {
    isRunning = true;
    const ext = t.backup_type === 'files' ? 'tar.gz' : 'sql';
    const remotePath = `/tmp/jarvis-backup-${Date.now()}.${ext}`;

    await withSudo(async () => {
      try {
        const ll = get(LL);
        const cmd = buildBackupCmd(t, remotePath);
        const out = await exec(cmd, t.backup_type === 'files');
        let msg = String(ll.backup.createdOnServer({ path: remotePath, output: out }));

        const count = await invoke<number>('sftp_start_download_batch', {
          remotePaths: [remotePath],
          localDir: null,
        });
        msg += `\n\n${String(ll.backup.downloading({ count }))}`;
        notifications.success(msg);
        await exec(`rm -f ${remotePath}`, false).catch(() => {});
      } catch (err: unknown) {
        if (isSudoPasswordRequired(err)) {
          throw err;
        }
        notifications.error(get(LL).backup.error({ error: formatInvokeError(err) }));
      } finally {
        isRunning = false;
      }
    });
    isRunning = false;
  }

  onMount(loadExtras);

  $effect(() => {
    if (profileId) loadExtras();
  });
</script>

<div class="backup manager-shell scrollable fade-in">
  <header class="manager-header">
    <h1 class="page-title">{$LL.backup.title()}</h1>
    <div class="header-actions">
      <button class="secondary btn-compact" disabled={isLoading} onclick={loadExtras}>
        <RefreshCw size={14} /> {$LL.common.refresh()}
      </button>
      <button class="primary btn-compact" onclick={openAdd}>
        <Plus size={14} /> {$LL.backup.newTemplate()}
      </button>
    </div>
  </header>

  {#if extras.backup_templates.length === 0}
    <div class="empty glass">
      <FolderArchive size={36} class="muted" />
      <p>{$LL.backup.empty()}</p>
      <button class="primary btn-compact" onclick={openAdd}><Plus size={14} /> {$LL.backup.addTemplate()}</button>
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
          </div>
          <div class="tpl-meta mono-val">
            {#if tpl.backup_type === 'files'}
              {tpl.source_path}
            {:else}
              {tpl.db_name || '—'} @ {tpl.docker_container || $LL.common.host()}
            {/if}
          </div>
          <div class="tpl-actions">
            <button class="primary btn-compact" disabled={isRunning} onclick={() => runBackup(tpl)}>
              {#if isRunning}<Loader2 size={14} class="spin" />{:else}<Play size={14} />{/if}
              {$LL.backup.run()}
            </button>
            <button class="secondary btn-compact" onclick={() => openEdit(tpl)}>{$LL.common.edit()}</button>
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
    <p>{$LL.backup.info()}</p>
  </section>
</div>

{#if showAddModal}
  <div class="modal-overlay" role="presentation" onclick={() => (showAddModal = false)}>
    <div class="modal glass" role="dialog" onclick={(e) => e.stopPropagation()}>
      <h3>{editId ? $LL.backup.editTemplate() : $LL.backup.newBackupTemplate()}</h3>
      <label>{$LL.backup.name()}<input bind:value={formName} placeholder={$LL.backup.namePlaceholder()} /></label>
      <label>{$LL.backup.type()}
        <select bind:value={formType}>
          <option value="files">{$LL.backup.typeFiles()}</option>
          <option value="mysql">{$LL.backup.typeMysql()}</option>
          <option value="postgres">{$LL.backup.typePostgres()}</option>
        </select>
      </label>
      {#if formType === 'files'}
        <label>{$LL.backup.pathLabel()}
          <PathAutocomplete bind:value={formPath} placeholder="/var/www" onlyDirs={true} />
        </label>
      {:else}
        <label>{$LL.backup.dockerContainer()}<input bind:value={formContainer} placeholder={$LL.backup.dockerPlaceholder()} /></label>
        <label>{$LL.backup.dbName()}<input bind:value={formDbName} /></label>
        <label>{$LL.backup.dbUser()}<input bind:value={formDbUser} /></label>
        <label>{$LL.backup.dbPassword()}<input type="password" bind:value={formDbPassword} /></label>
      {/if}
      <div class="modal-actions">
        <button class="secondary" onclick={() => (showAddModal = false)}>{$LL.common.cancel()}</button>
        <button class="primary" onclick={saveTemplate}>{$LL.common.save()}</button>
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
  .spin { animation: spin 1s linear infinite; }
  @keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
</style>
