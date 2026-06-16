<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import {
    RefreshCw, Plus, Trash2, Play, Database, FolderArchive, Loader2, Download,
  } from 'lucide-svelte';
  import SudoModal from './SudoModal.svelte';
  import type { BackupTemplate, ProfileExtras } from '$lib/admin/types';
  import { DEFAULT_PROFILE_EXTRAS } from '$lib/admin/types';

  let { profileId = '' } = $props();

  let extras = $state<ProfileExtras>({ ...DEFAULT_PROFILE_EXTRAS });
  let isLoading = $state(false);
  let isRunning = $state(false);
  let errorMsg = $state('');
  let lastBackupMsg = $state('');

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
    } catch (err: any) {
      if (err.toString() === 'SUDO_PASSWORD_REQUIRED') {
        pendingAction = () => withSudo(action);
        showSudoModal = true;
      } else {
        errorMsg = err.toString();
      }
    }
  }

  async function loadExtras() {
    if (!profileId) return;
    isLoading = true;
    try {
      extras = await invoke<ProfileExtras>('get_profile_extras', { profileId });
    } catch (err: any) {
      errorMsg = err.toString();
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
      alert('Podaj nazwę szablonu');
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
    if (!confirm('Usunąć szablon backupu?')) return;
    extras.backup_templates = extras.backup_templates.filter((t) => t.id !== id);
    await saveExtras();
  }

  function buildBackupCmd(t: BackupTemplate, remotePath: string): string {
    const ts = Date.now();
    if (t.backup_type === 'files') {
      const dir = t.source_path.replace(/\/$/, '');
      return `tar czf ${remotePath} -C ${dir} . 2>&1`;
    }
    if (t.backup_type === 'mysql') {
      const db = t.db_name || 'mysql';
      const user = t.db_user || 'root';
      const pass = t.db_password ? `-p'${t.db_password.replace(/'/g, "'\\''")}'` : '';
      if (t.docker_container) {
        return `docker exec ${t.docker_container} sh -c "mysqldump -u ${user} ${pass} ${db}" > ${remotePath} 2>&1`;
      }
      return `mysqldump -u ${user} ${pass} ${db} > ${remotePath} 2>&1`;
    }
    const db = t.db_name || 'postgres';
    const user = t.db_user || 'postgres';
    if (t.docker_container) {
      return `docker exec ${t.docker_container} pg_dump -U ${user} ${db} > ${remotePath} 2>&1`;
    }
    return `pg_dump -U ${user} ${db} > ${remotePath} 2>&1`;
  }

  async function runBackup(t: BackupTemplate) {
    isRunning = true;
    lastBackupMsg = '';
    errorMsg = '';
    const ext = t.backup_type === 'files' ? 'tar.gz' : 'sql';
    const remotePath = `/tmp/jarvis-backup-${Date.now()}.${ext}`;

    await withSudo(async () => {
      try {
        const cmd = buildBackupCmd(t, remotePath);
        const out = await exec(cmd, t.backup_type === 'files');
        lastBackupMsg = `Utworzono backup na serwerze: ${remotePath}\n${out}`;

        const count = await invoke<number>('sftp_start_download_batch', {
          remotePaths: [remotePath],
          localDir: null,
        });
        lastBackupMsg += `\n\nPobieranie ${count} plik(ów) na dysk lokalny...`;
        await exec(`rm -f ${remotePath}`, false).catch(() => {});
      } catch (err: any) {
        errorMsg = 'Błąd backupu: ' + err.toString();
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
    <h1 class="page-title">Backupy</h1>
    <div class="header-actions">
      <button class="secondary btn-compact" disabled={isLoading} onclick={loadExtras}>
        <RefreshCw size={14} /> Odśwież
      </button>
      <button class="primary btn-compact" onclick={openAdd}>
        <Plus size={14} /> Nowy szablon
      </button>
    </div>
  </header>

  {#if errorMsg}
    <div class="error-banner">{errorMsg}</div>
  {/if}

  {#if lastBackupMsg}
    <div class="success-banner">{lastBackupMsg}</div>
  {/if}

  {#if extras.backup_templates.length === 0}
    <div class="empty glass">
      <FolderArchive size={36} class="muted" />
      <p>Brak szablonów backupu. Utwórz szablon dla katalogów www lub baz danych.</p>
      <button class="primary btn-compact" onclick={openAdd}><Plus size={14} /> Dodaj szablon</button>
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
              {tpl.db_name || '—'} @ {tpl.docker_container || 'host'}
            {/if}
          </div>
          <div class="tpl-actions">
            <button class="primary btn-compact" disabled={isRunning} onclick={() => runBackup(tpl)}>
              {#if isRunning}<Loader2 size={14} class="spin" />{:else}<Play size={14} />{/if}
              Uruchom
            </button>
            <button class="secondary btn-compact" onclick={() => openEdit(tpl)}>Edytuj</button>
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
    <p>Backup tworzy archiwum na serwerze, pobiera je do folderu Pobrane (Jarvis-SFTP-*), a następnie usuwa plik tymczasowy.</p>
  </section>
</div>

{#if showAddModal}
  <div class="modal-overlay" role="presentation" onclick={() => (showAddModal = false)}>
    <div class="modal glass" role="dialog" onclick={(e) => e.stopPropagation()}>
      <h3>{editId ? 'Edytuj szablon' : 'Nowy szablon backupu'}</h3>
      <label>Nazwa<input bind:value={formName} placeholder="Backup www" /></label>
      <label>Typ
        <select bind:value={formType}>
          <option value="files">Pliki (tar.gz)</option>
          <option value="mysql">MySQL (mysqldump)</option>
          <option value="postgres">PostgreSQL (pg_dump)</option>
        </select>
      </label>
      {#if formType === 'files'}
        <label>Ścieżka katalogu<input bind:value={formPath} placeholder="/var/www" /></label>
      {:else}
        <label>Kontener Docker (puste = host)<input bind:value={formContainer} placeholder="mysql" /></label>
        <label>Nazwa bazy<input bind:value={formDbName} /></label>
        <label>Użytkownik DB<input bind:value={formDbUser} /></label>
        <label>Hasło DB (opcjonalne)<input type="password" bind:value={formDbPassword} /></label>
      {/if}
      <div class="modal-actions">
        <button class="secondary" onclick={() => (showAddModal = false)}>Anuluj</button>
        <button class="primary" onclick={saveTemplate}>Zapisz</button>
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
  .error-banner, .success-banner { padding: 10px; border-radius: var(--radius-sm); font-size: 0.85rem; white-space: pre-wrap; }
  .error-banner { background: var(--accent-red-glow); color: #ff8585; }
  .success-banner { background: var(--accent-green-glow); color: var(--accent-green); }
  .modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.6); display: flex; align-items: center; justify-content: center; z-index: 1000; }
  .modal { width: 420px; padding: 20px; border-radius: var(--radius-md); display: flex; flex-direction: column; gap: 10px; }
  .modal h3 { color: white; font-size: 1rem; }
  .modal label { display: flex; flex-direction: column; gap: 4px; font-size: 0.8rem; color: var(--text-secondary); }
  .modal-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 8px; }
  .hover-red:hover { color: var(--accent-red) !important; }
  .spin { animation: spin 1s linear infinite; }
  @keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
</style>
