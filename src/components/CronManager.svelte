<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { Calendar, Trash2, Plus, Edit, RefreshCw, Play, ShieldAlert, Check, ToggleLeft, ToggleRight } from 'lucide-svelte';
  import SortableTh from './ui/SortableTh.svelte';
  import { applySort, nextSort, type SortState } from '$lib/sort/sortUtils';
  import { get } from 'svelte/store';
  import { LL } from '$lib/i18n/i18n-svelte';
  import { formatInvokeError } from '$lib/i18n/backendErrors';
  import { notifications } from '$lib/notifications.svelte';

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
  
  // Helper tool to build cron expressions
  let presetMinutes = $state('*/5');
  let presetHours = $state('*');
  let presetDays = $state('*');
  let presetMonths = $state('*');
  let presetDayOfWeek = $state('*');

  function updateExprFromPresets() {
    cronExpr = `${presetMinutes} ${presetHours} ${presetDays} ${presetMonths} ${presetDayOfWeek}`;
  }

  async function loadCronJobs() {
    isLoading = true;
    try {
      // crontab -l returns exit code 1 if no crontab exists, we handle it in Rust or here
      const result: string = await invoke('exec_custom_command', {
        cmd: 'crontab -l',
        useSudo: false
      });
      
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
            parsedJobs.push({
              id: crypto.randomUUID(),
              expression: expr,
              command: cmd,
              is_active: false
            });
          } else {
            // regular comment
            parsedJobs.push({
              id: crypto.randomUUID(),
              raw: line,
              is_meta: true
            });
          }
        } else {
          // Active cron job or env definition
          const parts = line.trim().split(/\s+/);
          if (parts.length >= 6 && (
            parts[0] === '*' || /^\d+/.test(parts[0]) || parts[0].startsWith('*/') || parts[0].startsWith('@')
          )) {
            const expr = parts.slice(0, 5).join(' ');
            const cmd = parts.slice(5).join(' ');
            parsedJobs.push({
              id: crypto.randomUUID(),
              expression: expr,
              command: cmd,
              is_active: true
            });
          } else {
            // env definition or something else
            parsedJobs.push({
              id: crypto.randomUUID(),
              raw: line,
              is_meta: true
            });
          }
        }
      }
      
      cronJobs = parsedJobs;
    } catch (err: any) {
      // We treat the absence of crontab as an empty list, not an error
      const errString = formatInvokeError(err);
      const lowerErr = errString.toLowerCase();
      if (lowerErr.includes('no crontab') || lowerErr.includes('kod 1')) {
        cronJobs = [];
      } else {
        notifications.error(get(LL).cron.loadFailed({ error: errString }));
      }
    } finally {
      isLoading = false;
    }
  }

  async function saveCronJobs(jobsList: any[]) {
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
      notifications.error(get(LL).cron.saveFailed({ error: formatInvokeError(err) }));
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
    if (confirm(get(LL).cron.confirmDelete())) {
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
    
    // Try to match presets to the edited task
    const parts = job.expression.split(' ');
    if (parts.length === 5) {
      presetMinutes = parts[0];
      presetHours = parts[1];
      presetDays = parts[2];
      presetMonths = parts[3];
      presetDayOfWeek = parts[4];
    }
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
    <h1 class="page-title">{$LL.cron.title()}</h1>
  </header>

  <!-- Pasek operacyjny -->
  <div class="ops-bar glass">
    <button class="primary" onclick={() => showCreateModal = true}>
      <Plus size={16} /> {$LL.cron.newTask()}
    </button>
  </div>

  <!-- Cron jobs list -->
  <div class="table-container glass">
    {#if isLoading && cronJobs.length === 0}
      <div class="loading-state">
        <RefreshCw class="spin" size={32} />
        <p>{$LL.cron.loading()}</p>
      </div>
    {:else}
      <table class="cron-table">
        <thead>
          <tr>
            <SortableTh label={$LL.cron.active()} column="active" activeColumn={cronSort.column} direction={cronSort.direction} onsort={setCronSort} width="10%" />
            <SortableTh label={$LL.cron.schedule()} column="expression" activeColumn={cronSort.column} direction={cronSort.direction} onsort={setCronSort} width="20%" />
            <SortableTh label={$LL.cron.command()} column="command" activeColumn={cronSort.column} direction={cronSort.direction} onsort={setCronSort} width="50%" />
            <th style="width: 20%; text-align: right; padding: 14px 16px; font-size: 0.8rem; text-transform: uppercase; color: var(--text-muted); font-weight: 600;">{$LL.common.actions()}</th>
          </tr>
        </thead>
        <tbody>
          {#each sortedCronJobs as job, index}
            <tr class={job.is_active ? '' : 'disabled-row'}>
              <td>
                <button class="btn-toggle" onclick={() => toggleCronJob(job)} title={job.is_active ? $LL.cron.toggleDisable() : $LL.cron.toggleEnable()}>
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
                <button class="btn-table" onclick={() => openEditModal(job, index)} title={$LL.cron.editTitle()}>
                  <Edit size={14} />
                </button>
                <button class="btn-table danger-text" onclick={() => deleteCronJob(job.id)} title={$LL.cron.deleteTitle()}>
                  <Trash2 size={14} />
                </button>
              </td>
            </tr>
          {/each}

          {#if sortedCronJobs.length === 0 && !isLoading}
            <tr>
              <td colspan="4" class="empty-state">{$LL.cron.empty()}</td>
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
        <h3>{showCreateModal ? $LL.cron.addTask() : $LL.cron.editTask()}</h3>
        
        <div class="form-group">
          <label for="cron-command">{$LL.cron.commandLabel()}</label>
          <input id="cron-command" type="text" placeholder={$LL.cron.commandPlaceholder()} bind:value={cronCmd} />
        </div>

        <div class="form-group">
          <label for="cron-expression-input">{$LL.cron.expressionLabel()}</label>
          <input id="cron-expression-input" type="text" placeholder={$LL.cron.expressionPlaceholder()} bind:value={cronExpr} />
        </div>

        <!-- Visual Expression Generator -->
        <div class="cron-generator glass">
          <h4>{$LL.cron.generatorTitle()}</h4>
          <div class="generator-grid">
            <div class="form-group">
              <label for="gen-min">{$LL.cron.minute()}</label>
              <select id="gen-min" bind:value={presetMinutes} onchange={updateExprFromPresets}>
                <option value="*">{$LL.cron.everyMinute()}</option>
                <option value="*/5">{$LL.cron.every5Min()}</option>
                <option value="*/15">{$LL.cron.every15Min()}</option>
                <option value="0">{$LL.cron.onTheHour()}</option>
                <option value="30">{$LL.cron.at30()}</option>
              </select>
            </div>
            
            <div class="form-group">
              <label for="gen-hour">{$LL.cron.hour()}</label>
              <select id="gen-hour" bind:value={presetHours} onchange={updateExprFromPresets}>
                <option value="*">{$LL.cron.everyHour()}</option>
                <option value="*/2">{$LL.cron.every2Hours()}</option>
                <option value="0">{$LL.cron.midnight()}</option>
                <option value="12">{$LL.cron.noon()}</option>
                <option value="2">{$LL.cron.twoAm()}</option>
              </select>
            </div>

            <div class="form-group">
              <label for="gen-day">{$LL.cron.dayOfMonth()}</label>
              <select id="gen-day" bind:value={presetDays} onchange={updateExprFromPresets}>
                <option value="*">{$LL.cron.everyDay()}</option>
                <option value="1">{$LL.cron.firstDay()}</option>
                <option value="15">{$LL.cron.midMonth()}</option>
                <option value="*/2">{$LL.cron.everyOtherDay()}</option>
              </select>
            </div>

            <div class="form-group">
              <label for="gen-month">{$LL.cron.month()}</label>
              <select id="gen-month" bind:value={presetMonths} onchange={updateExprFromPresets}>
                <option value="*">{$LL.cron.everyMonth()}</option>
                <option value="1">{$LL.cron.january()}</option>
                <option value="*/3">{$LL.cron.quarterly()}</option>
              </select>
            </div>

            <div class="form-group">
              <label for="gen-dow">{$LL.cron.dayOfWeek()}</label>
              <select id="gen-dow" bind:value={presetDayOfWeek} onchange={updateExprFromPresets}>
                <option value="*">{$LL.cron.everyDay()}</option>
                <option value="1-5">{$LL.cron.weekdays()}</option>
                <option value="0,6">{$LL.cron.weekend()}</option>
                <option value="1">{$LL.cron.monday()}</option>
              </select>
            </div>
          </div>
        </div>

        <div class="modal-actions">
          {#if showCreateModal}
            <button class="primary" onclick={addCronJob} disabled={!cronExpr || !cronCmd}>{$LL.cron.addTask()}</button>
            <button class="secondary" onclick={() => showCreateModal = false}>{$LL.common.cancel()}</button>
          {:else}
            <button class="primary" onclick={editCronJob} disabled={!cronExpr || !cronCmd}>{$LL.common.saveChanges()}</button>
            <button class="secondary" onclick={() => { showEditModal = false; editingIndex = null; }}>{$LL.common.cancel()}</button>
          {/if}
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
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

  /* Visual Generator */
  .cron-generator {
    padding: 16px;
    border-radius: var(--radius-md);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .cron-generator h4 {
    font-size: 0.85rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--accent-amber);
    border-bottom: 1px solid var(--border-color);
    padding-bottom: 6px;
  }

  .generator-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 12px;
  }

  .generator-grid .form-group:nth-child(4),
  .generator-grid .form-group:nth-child(5) {
    grid-column: span 1.5;
  }
</style>
