<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { Calendar, Trash2, Plus, Edit, RefreshCw, Play, ShieldAlert, Check, ToggleLeft, ToggleRight } from 'lucide-svelte';
  import SortableTh from './ui/SortableTh.svelte';
  import { applySort, nextSort, type SortState } from '$lib/sort/sortUtils';
  import { get } from 'svelte/store';
    import { formatInvokeError, isSudoPasswordRequired } from '$lib/backendErrors';
  import { notifications } from '$lib/notifications.svelte';
  import CronExpressionInput from '$lib/CronExpressionInput.svelte';
  import SudoModal from './SudoModal.svelte';

  let { visible = true } = $props();

  export function refresh() { loadCronJobs(); }

  let cronJobs = $state<any[]>([]);
  type CronSortCol = 'active' | 'expression' | 'command';
  let cronSort = $state<SortState<CronSortCol>>({ column: 'expression', direction: 'asc' });

  const sortedCronJobs = $derived(
    applySort(
      cronJobs.filter((j) => !j.is_meta),
      cronSort,
      {
        active: (j) => j.is_active,
        expression: (j) => j.expression || '',
        command: (j) => j.command || '',
      },
    ),
  );

  function setCronSort(column: string) {
    cronSort = nextSort(cronSort, column as CronSortCol);
  }
  let isLoading = $state(false);

  // Modale
  let showCreateModal = $state(false);
  let showEditModal = $state(false);
  let editingIndex = $state<number | null>(null);

  // Form variables
  let cronExpr = $state('*/5 * * * *');
  let cronCmd = $state('');

  // Root crontab viewer (read-only). When on, the table shows root's crontab
  // via sudo — including Jarvis backup schedules — and editing is disabled.
  let rootView = $state(false);
  let showSudoModal = $state(false);
  let pendingAction: (() => Promise<void>) | null = null;

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

  function parseCrontab(result: string) {
    const parsedJobs = [];
    const lines = result.split('\n');
    for (const line of lines) {
      if (!line.trim()) continue;

      if (line.startsWith('#')) {
        // Check if it's a commented-out cron job
        const cleaned = line.replace(/^#\s*/, '').trim();
        const parts = cleaned.split(/\s+/);

        if (parts.length >= 6 && (
          parts[0] === '*' || /^\d+/.test(parts[0]) || parts[0].startsWith('*/') || parts[0].startsWith('@')
        )) {
          const expr = parts.slice(0, 5).join(' ');
          const cmd = parts.slice(5).join(' ');
          parsedJobs.push({ id: crypto.randomUUID(), expression: expr, command: cmd, is_active: false });
        } else {
          parsedJobs.push({ id: crypto.randomUUID(), raw: line, is_meta: true });
        }
      } else {
        const parts = line.trim().split(/\s+/);
        if (parts.length >= 6 && (
          parts[0] === '*' || /^\d+/.test(parts[0]) || parts[0].startsWith('*/') || parts[0].startsWith('@')
        )) {
          const expr = parts.slice(0, 5).join(' ');
          const cmd = parts.slice(5).join(' ');
          parsedJobs.push({ id: crypto.randomUUID(), expression: expr, command: cmd, is_active: true });
        } else {
          parsedJobs.push({ id: crypto.randomUUID(), raw: line, is_meta: true });
        }
      }
    }
    return parsedJobs;
  }

  async function loadCronJobs() {
    isLoading = true;
    try {
      if (rootView) {
        await withSudo(async () => {
          const result: string = await invoke('get_root_crontab');
          cronJobs = parseCrontab(result);
        });
        return;
      }
      // crontab -l returns exit code 1 if no crontab exists, we handle it here
      const result: string = await invoke('exec_custom_command', {
        cmd: 'crontab -l',
        useSudo: false
      });
      cronJobs = parseCrontab(result);
    } catch (err: any) {
      // We treat the absence of crontab as an empty list, not an error
      const errString = formatInvokeError(err);
      const lowerErr = errString.toLowerCase();
      if (lowerErr.includes('no crontab') || lowerErr.includes('kod 1')) {
        cronJobs = [];
      } else {
        notifications.error(`Failed to load cron jobs: ${errString}`);
      }
    } finally {
      isLoading = false;
    }
  }

  function toggleRootView() {
    rootView = !rootView;
    cronJobs = [];
    loadCronJobs();
  }

  async function saveCronJobs(jobsList: any[]) {
    // Root crontab is read-only here; never write root's entries into the user crontab.
    if (rootView) return;
    isLoading = true;

    // Build the crontab file content
    const fileContent = jobsList.map(job => {
      if (job.is_meta) {
        return job.raw; // preserve headers/environment variables
      }
      if (job.is_active) {
        return `${job.expression} ${job.command}`;
      } else {
        return `# ${job.expression} ${job.command}`;
      }
    }).join('\n') + '\n';
    
    const tmpPath = `/tmp/jarvis_cron_${Date.now()}_${Math.random().toString(36).slice(2)}`;
    
    try {
      // 1. Save the updated list to a temporary file
      await invoke('sftp_write', { path: tmpPath, content: fileContent });
      // 2. Load the file into crontab
      await invoke('exec_custom_command', {
        cmd: `crontab ${tmpPath} && rm ${tmpPath}`,
        useSudo: false
      });
      await loadCronJobs();
    } catch (err: unknown) {
      notifications.error(`Failed to save tasks: ${formatInvokeError(err)}`);
    } finally {
      isLoading = false;
    }
  }

  async function addCronJob() {
    if (!cronExpr || !cronCmd) return;
    
    const newJob = {
      id: Date.now(),
      expression: cronExpr,
      command: cronCmd,
      is_active: true
    };
    
    const updated = [...cronJobs, newJob];
    await saveCronJobs(updated);
    showCreateModal = false;
    cronCmd = '';
    cronExpr = '*/5 * * * *';
  }

  async function deleteCronJob(id: number) {
    if (confirm("Are you sure you want to delete this cron task?")) {
      const updated = cronJobs.filter(j => j.id !== id);
      await saveCronJobs(updated);
    }
  }

  async function toggleCronJob(job: any) {
    const updated = cronJobs.map(j => {
      if (j.id === job.id) {
        return { ...j, is_active: !j.is_active };
      }
      return j;
    });
    await saveCronJobs(updated);
  }

  function openEditModal(job: any, index: number) {
    editingIndex = index;
    cronExpr = job.expression;
    cronCmd = job.command;
    showEditModal = true;
  }

  async function editCronJob() {
    if (editingIndex === null || !cronExpr || !cronCmd) return;
    
    const updated = [...cronJobs];
    updated[editingIndex] = {
      ...updated[editingIndex],
      expression: cronExpr,
      command: cronCmd
    };
    
    await saveCronJobs(updated);
    showEditModal = false;
    editingIndex = null;
    cronCmd = '';
  }

  onMount(() => {
    loadCronJobs();
  });
</script>

<div class="cron-manager manager-shell fade-in">
  <header class="manager-header">
    <h1 class="page-title">Scheduled tasks (Cron)</h1>
  </header>

  <!-- Pasek operacyjny -->
  <div class="ops-bar glass">
    <button class="primary" onclick={() => showCreateModal = true} disabled={rootView}>
      <Plus size={16} /> New task
    </button>
    <button class="secondary" class:active={rootView} onclick={toggleRootView} title="View root's crontab (sudo) — includes Jarvis backup schedules">
      <ShieldAlert size={16} /> {rootView ? "Viewing root crontab" : "View root crontab"}
    </button>
    {#if rootView}
      <span class="root-note">Read-only — backup schedules are managed in the Backups tab.</span>
    {/if}
  </div>

  <!-- Cron jobs list -->
  <div class="table-container glass">
    {#if isLoading && cronJobs.length === 0}
      <div class="loading-state">
        <RefreshCw class="spin" size={32} />
        <p>Loading scheduled tasks…</p>
      </div>
    {:else}
      <table class="cron-table">
        <thead>
          <tr>
            <SortableTh label="Active" column="active" activeColumn={cronSort.column} direction={cronSort.direction} onsort={setCronSort} width="10%" />
            <SortableTh label="Schedule (Cron)" column="expression" activeColumn={cronSort.column} direction={cronSort.direction} onsort={setCronSort} width="20%" />
            <SortableTh label="Command" column="command" activeColumn={cronSort.column} direction={cronSort.direction} onsort={setCronSort} width="50%" />
            <th style="width: 20%; text-align: right; padding: 14px 16px; font-size: 0.8rem; text-transform: uppercase; color: var(--text-muted); font-weight: 600;">Actions</th>
          </tr>
        </thead>
        <tbody>
          {#each sortedCronJobs as job, index}
            <tr class={job.is_active ? '' : 'disabled-row'}>
              <td>
                <button class="btn-toggle" onclick={() => toggleCronJob(job)} disabled={rootView} title={job.is_active ? "Disable" : "Enable"}>
                  {#if job.is_active}
                    <ToggleRight size={22} class="toggle-icon active" />
                  {:else}
                    <ToggleLeft size={22} class="toggle-icon" />
                  {/if}
                </button>
              </td>
              <td>
                <code class="cron-code mono-val">{job.expression}</code>
              </td>
              <td class="command-cell" title={job.command}>
                <code class="mono-val">{job.command}</code>
              </td>
              <td class="actions-cell">
                {#if !rootView}
                  <button class="btn-table" onclick={() => openEditModal(job, index)} title="Edit">
                    <Edit size={14} />
                  </button>
                  <button class="btn-table danger-text" onclick={() => deleteCronJob(job.id)} title="Delete">
                    <Trash2 size={14} />
                  </button>
                {/if}
              </td>
            </tr>
          {/each}

          {#if sortedCronJobs.length === 0 && !isLoading}
            <tr>
              <td colspan="4" class="empty-state">No active tasks in crontab file</td>
            </tr>
          {/if}
        </tbody>
      </table>
    {/if}
  </div>

  <!-- Modal Kreator / Edytor Zadania Cron -->
  {#if showCreateModal || showEditModal}
    <div class="modal-overlay">
      <div class="modal-content glass cron-modal">
        <h3>{showCreateModal ? "Add task" : "Edit Cron task"}</h3>
        
        <div class="form-group">
          <label for="cron-command">Command / script to run</label>
          <input id="cron-command" type="text" placeholder="/var/www/scripts/backup.sh >> /var/log/backup.log 2>&1" bind:value={cronCmd} />
        </div>

        <CronExpressionInput bind:value={cronExpr} />

        <div class="modal-actions">
          {#if showCreateModal}
            <button class="primary" onclick={addCronJob} disabled={!cronExpr || !cronCmd}>Add task</button>
            <button class="secondary" onclick={() => showCreateModal = false}>Cancel</button>
          {:else}
            <button class="primary" onclick={editCronJob} disabled={!cronExpr || !cronCmd}>Save changes</button>
            <button class="secondary" onclick={() => { showEditModal = false; editingIndex = null; }}>Cancel</button>
          {/if}
        </div>
      </div>
    </div>
  {/if}
</div>

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
  .root-note { font-size: 0.75rem; color: var(--text-muted); }
  .ops-bar .secondary.active { color: var(--accent-amber); }
  .cron-manager {
    /* uses .manager-shell */
  }

  /* Ops Bar */
  .ops-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border-radius: var(--radius-md);
    flex-shrink: 0;
  }

  /* Table */
  .table-container {
    flex: 1;
    overflow-y: auto;
    border-radius: var(--radius-md);
  }

  .cron-table {
    width: 100%;
    border-collapse: collapse;
    text-align: left;
  }

  .cron-table th, .cron-table td {
    padding: 8px 12px;
    border-bottom: 1px solid var(--border-color);
  }

  .cron-table td {
    font-size: 0.9rem;
  }

  .cron-table th {
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

  .cron-table tr {
    transition: var(--transition-fast);
  }

  .cron-table tr:hover {
    background: rgba(255, 255, 255, 0.02);
  }

  .disabled-row {
    opacity: 0.45;
  }

  .btn-toggle {
    background: transparent;
    border: none;
    cursor: pointer;
    color: var(--text-muted);
    display: flex;
    align-items: center;
  }

  .toggle-icon.active {
    color: var(--accent-amber);
  }

  .cron-code {
    font-family: var(--font-mono);
    color: var(--accent-amber);
    background: var(--accent-amber-glow);
    padding: 4px 8px;
    border-radius: var(--radius-sm);
    font-size: 0.8rem;
    border: 1px solid rgba(245, 158, 11, 0.15);
  }

  .command-cell {
    font-size: 0.85rem;
    max-width: 400px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .command-cell code {
    color: var(--text-primary);
  }

  .actions-cell {
    text-align: right;
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .btn-table {
    background: transparent;
    border: none;
    padding: 6px;
    border-radius: 4px;
    color: var(--text-secondary);
    cursor: pointer;
    transition: var(--transition-fast);
  }

  .btn-table:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .danger-text:hover {
    color: var(--accent-red) !important;
    background: var(--accent-red-glow) !important;
  }

  .loading-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 16px;
    color: var(--text-secondary);
  }

  .empty-state {
    text-align: center;
    color: var(--text-muted);
    font-size: 0.9rem;
    padding: 40px !important;
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
    width: 420px;
    padding: 24px;
    border-radius: var(--radius-md);
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .cron-modal {
    width: 550px;
  }

  .modal-content input {
    width: 100%;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }

</style>
