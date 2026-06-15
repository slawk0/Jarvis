<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { Calendar, Trash2, Plus, Edit, RefreshCw, Play, ShieldAlert, Check, ToggleLeft, ToggleRight } from 'lucide-svelte';

  let cronJobs = $state<any[]>([]);
  let isLoading = $state(false);
  let errorMsg = $state('');

  // Modale
  let showCreateModal = $state(false);
  let showEditModal = $state(false);
  let editingIndex = $state<number | null>(null);

  // Zmienne formularza
  let cronExpr = $state('*/5 * * * *');
  let cronCmd = $state('');
  
  // Narzędzie ułatwiające budowanie wyrazów cron
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
    errorMsg = '';
    try {
      // crontab -l zwraca błąd o kodzie 1 jeśli nie ma crontaba, obsłużymy to w Rust lub tutaj
      const result: string = await invoke('exec_custom_command', {
        cmd: 'crontab -l',
        useSudo: false
      });
      
      const lines = result.trim().split('\n');
      let parsedJobs = [];
      let idCounter = 0;
      
      for (const line of lines) {
        const trimmed = line.trim();
        if (trimmed === '') continue;
        
        let isCommented = trimmed.startsWith('#');
        let cleanLine = isCommented ? trimmed.substring(1).trim() : trimmed;
        
        // Wyrażenie cron ma 5 kolumn oddzielonych spacjami, reszta to komenda
        const parts = cleanLine.split(/\s+/);
        if (parts.length >= 6) {
          const expression = parts.slice(0, 5).join(' ');
          const command = parts.slice(5).join(' ');
          
          parsedJobs.push({
            id: idCounter++,
            expression,
            command,
            is_active: !isCommented,
            raw: trimmed
          });
        } else {
          // Inne linie (np. komentarze nagłówkowe lub zmienne środowiskowe jak MAILTO)
          parsedJobs.push({
            id: idCounter++,
            expression: '',
            command: cleanLine,
            is_active: false,
            raw: trimmed,
            is_meta: true
          });
        }
      }
      
      cronJobs = parsedJobs;
    } catch (err: any) {
      // Uznajemy, że brak crontaba to pusta lista, a nie błąd
      if (err.toString().includes('no crontab') || err.toString().includes('kod 1')) {
        cronJobs = [];
      } else {
        errorMsg = 'Nie udało się wczytać zadań cron: ' + err.toString();
      }
    } finally {
      isLoading = false;
    }
  }

  async function saveCronJobs(jobsList: any[]) {
    isLoading = true;
    errorMsg = '';
    
    // Budujemy zawartość pliku crontab
    const fileContent = jobsList.map(job => {
      if (job.is_meta) {
        return job.raw; // zachowaj nagłówki/zmienne środowiskowe
      }
      if (job.is_active) {
        return `${job.expression} ${job.command}`;
      } else {
        return `# ${job.expression} ${job.command}`;
      }
    }).join('\n') + '\n';
    
    const tmpPath = `/tmp/cron_temp`;
    
    try {
      // 1. Zapisujemy zaktualizowaną listę do pliku tymczasowego
      await invoke('sftp_write', { path: tmpPath, content: fileContent });
      // 2. Ładujemy plik do crontab
      await invoke('exec_custom_command', {
        cmd: `crontab ${tmpPath} && rm ${tmpPath}`,
        useSudo: false
      });
      await loadCronJobs();
    } catch (err: any) {
      errorMsg = 'Nie udało się zapisać zadań: ' + err.toString();
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
    if (confirm('Czy na pewno chcesz usunąć to zadanie cron?')) {
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
    
    // Spróbuj dopasować presety do edytowanego zadania
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

<div class="cron-manager fade-in">
  <header class="cm-header">
    <div class="title-area">
      <h1>Zadania Harmonogramu (Cron)</h1>
      <p class="subtitle">Zautomatyzuj uruchamianie skryptów i poleceń w tle</p>
    </div>
    {#if errorMsg}
      <div class="error-badge">{errorMsg}</div>
    {/if}
  </header>

  <!-- Pasek operacyjny -->
  <div class="ops-bar glass">
    <button class="secondary" onclick={loadCronJobs} disabled={isLoading}>
      <RefreshCw size={16} class={isLoading ? 'spin' : ''} /> Odśwież
    </button>
    <button class="primary" onclick={() => showCreateModal = true}>
      <Plus size={16} /> Nowe Zadanie
    </button>
  </div>

  <!-- Lista zadań cron -->
  <div class="table-container glass">
    {#if isLoading && cronJobs.length === 0}
      <div class="loading-state">
        <RefreshCw class="spin" size={32} />
        <p>Wczytywanie zadań harmonogramu...</p>
      </div>
    {:else}
      <table class="cron-table">
        <thead>
          <tr>
            <th style="width: 10%;">Aktywny</th>
            <th style="width: 20%;">Harmonogram (Cron)</th>
            <th style="width: 50%;">Komenda</th>
            <th style="width: 20%; text-align: right;">Akcje</th>
          </tr>
        </thead>
        <tbody>
          {#each cronJobs.filter(j => !j.is_meta) as job, index}
            <tr class={job.is_active ? '' : 'disabled-row'}>
              <td>
                <button class="btn-toggle" onclick={() => toggleCronJob(job)} title={job.is_active ? 'Wyłącz' : 'Włącz'}>
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
                <button class="btn-table" onclick={() => openEditModal(job, index)} title="Edytuj">
                  <Edit size={14} />
                </button>
                <button class="btn-table danger-text" onclick={() => deleteCronJob(job.id)} title="Usuń">
                  <Trash2 size={14} />
                </button>
              </td>
            </tr>
          {/each}

          {#if cronJobs.filter(j => !j.is_meta).length === 0 && !isLoading}
            <tr>
              <td colspan="4" class="empty-state">Brak aktywnych zadań w pliku crontab</td>
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
        <h3>{showCreateModal ? 'Dodaj nowe zadanie Cron' : 'Edytuj zadanie Cron'}</h3>
        
        <div class="form-group">
          <label for="cron-command">Polecenie / Skrypt do uruchomienia</label>
          <input id="cron-command" type="text" placeholder="/var/www/scripts/backup.sh >> /var/log/backup.log 2>&1" bind:value={cronCmd} />
        </div>

        <div class="form-group">
          <label for="cron-expression-input">Wyrażenie Cron (5 pól)</label>
          <input id="cron-expression-input" type="text" placeholder="* * * * *" bind:value={cronExpr} />
        </div>

        <!-- Wizualny Generator Wyrażeń -->
        <div class="cron-generator glass">
          <h4>Wizualny Kreator Harmonogramu</h4>
          <div class="generator-grid">
            <div class="form-group">
              <label for="gen-min">Minuta</label>
              <select id="gen-min" bind:value={presetMinutes} onchange={updateExprFromPresets}>
                <option value="*">Każda (*)</option>
                <option value="*/5">Co 5 minut (*/5)</option>
                <option value="*/15">Co 15 minut (*/15)</option>
                <option value="0">O pełnej godzinie (0)</option>
                <option value="30">W 30. minucie (30)</option>
              </select>
            </div>
            
            <div class="form-group">
              <label for="gen-hour">Godzina</label>
              <select id="gen-hour" bind:value={presetHours} onchange={updateExprFromPresets}>
                <option value="*">Każda godzina (*)</option>
                <option value="*/2">Co 2 godziny (*/2)</option>
                <option value="0">Północ (00:00)</option>
                <option value="12">Południe (12:00)</option>
                <option value="2">Druga w nocy (02:00)</option>
              </select>
            </div>

            <div class="form-group">
              <label for="gen-day">Dzień miesiąca</label>
              <select id="gen-day" bind:value={presetDays} onchange={updateExprFromPresets}>
                <option value="*">Każdy dzień (*)</option>
                <option value="1">Pierwszy dzień (1)</option>
                <option value="15">Połowa miesiąca (15)</option>
                <option value="*/2">Co drugi dzień (*/2)</option>
              </select>
            </div>

            <div class="form-group">
              <label for="gen-month">Miesiąc</label>
              <select id="gen-month" bind:value={presetMonths} onchange={updateExprFromPresets}>
                <option value="*">Każdy miesiąc (*)</option>
                <option value="1">Styczeń (1)</option>
                <option value="*/3">Kwartalnie (*/3)</option>
              </select>
            </div>

            <div class="form-group">
              <label for="gen-dow">Dzień tygodnia</label>
              <select id="gen-dow" bind:value={presetDayOfWeek} onchange={updateExprFromPresets}>
                <option value="*">Każdy dzień (*)</option>
                <option value="1-5">Dni robocze (Pon-Pt)</option>
                <option value="0,6">Weekend (Sob-Nie)</option>
                <option value="1">Poniedziałek (1)</option>
              </select>
            </div>
          </div>
        </div>

        <div class="modal-actions">
          {#if showCreateModal}
            <button class="primary" onclick={addCronJob} disabled={!cronExpr || !cronCmd}>Dodaj zadanie</button>
            <button class="secondary" onclick={() => showCreateModal = false}>Anuluj</button>
          {:else}
            <button class="primary" onclick={editCronJob} disabled={!cronExpr || !cronCmd}>Zapisz zmiany</button>
            <button class="secondary" onclick={() => { showEditModal = false; editingIndex = null; }}>Anuluj</button>
          {/if}
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .cron-manager {
    padding: 30px;
    display: flex;
    flex-direction: column;
    gap: 24px;
    height: 100%;
    overflow: hidden;
  }

  .cm-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-shrink: 0;
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
    font-size: 0.85rem;
  }

  /* Ops Bar */
  .ops-bar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
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
    padding: 14px 20px;
    border-bottom: 1px solid var(--border-color);
  }

  .cron-table th {
    font-size: 0.8rem;
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

  .spin {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
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
