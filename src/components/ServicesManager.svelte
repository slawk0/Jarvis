<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { Play, Square, RotateCw, Plus, RefreshCw, Eye, ShieldAlert, KeyRound } from 'lucide-svelte';

  let services = $state<any[]>([]);
  let filteredServices = $state<any[]>([]);
  let searchQuery = $state('');
  let isLoading = $state(false);
  let errorMsg = $state('');

  // Zarządzanie uwierzytelnieniem Sudo
  let showSudoModal = $state(false);
  let sudoPassword = $state('');
  let pendingAction: (() => Promise<void>) | null = null;
  let sudoError = $state('');

  // Modale i kreator nowej usługi
  let showCreateModal = $state(false);
  let serviceName = $state('');
  let serviceDesc = $state('');
  let serviceExec = $state('');
  let serviceUser = $state('root');
  let serviceRestart = $state('always');

  async function loadServices() {
    isLoading = true;
    errorMsg = '';
    try {
      const output: string = await invoke('exec_custom_command', {
        cmd: 'systemctl list-units --type=service --all --no-legend --no-pager',
        useSudo: false
      });
      
      const lines = output.trim().split('\n');
      const parsed = lines.map(line => {
        // systemctl list-units zwraca: UNIT LOAD ACTIVE SUB DESCRIPTION
        // Elementy mogą zawierać spacje, rozdzielamy je dopasowując pierwsze 4 kolumny
        const parts = line.trim().split(/\s+/);
        if (parts.length < 4) return null;
        
        const unit = parts[0];
        const load = parts[1];
        const active = parts[2];
        const sub = parts[3];
        const desc = parts.slice(4).join(' ');
        
        return {
          name: unit.replace('.service', ''),
          load,
          active,
          sub,
          desc
        };
      }).filter(Boolean);

      services = parsed;
      filterServices();
    } catch (err: any) {
      errorMsg = 'Nie udało się wczytać usług: ' + err.toString();
    } finally {
      isLoading = false;
    }
  }

  function filterServices() {
    if (!searchQuery) {
      filteredServices = services;
    } else {
      const q = searchQuery.toLowerCase();
      filteredServices = services.filter(s => 
        s.name.toLowerCase().includes(q) || 
        s.desc.toLowerCase().includes(q)
      );
    }
  }

  async function executeServiceAction(action: string, serviceName: string) {
    const run = async () => {
      isLoading = true;
      errorMsg = '';
      try {
        await invoke('exec_custom_command', {
          cmd: `systemctl ${action} ${serviceName}.service`,
          useSudo: true
        });
        await loadServices();
      } catch (err: any) {
        if (err.toString() === 'SUDO_PASSWORD_REQUIRED') {
          pendingAction = run;
          showSudoModal = true;
        } else if (err.toString() === 'SUDO_PASSWORD_INCORRECT') {
          errorMsg = 'Niepoprawne hasło sudo. Spróbuj ponownie.';
        } else {
          errorMsg = `Błąd wykonania akcji ${action}: ` + err.toString();
        }
      } finally {
        isLoading = false;
      }
    };
    await run();
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

  async function createService() {
    if (!serviceName || !serviceExec) return;
    
    const run = async () => {
      isLoading = true;
      errorMsg = '';
      
      const unitContent = `[Unit]
Description=${serviceDesc}
After=network.target

[Service]
Type=simple
ExecStart=${serviceExec}
User=${serviceUser}
Restart=${serviceRestart}

[Install]
WantedBy=multi-user.target
`;
      
      const tmpPath = `/tmp/${serviceName}.service`;
      const finalPath = `/etc/systemd/system/${serviceName}.service`;

      try {
        // 1. Zapis pliku przez SFTP do /tmp
        await invoke('sftp_write', { path: tmpPath, content: unitContent });
        
        // 2. Przeniesienie pliku i rejestracja z sudo
        await invoke('exec_custom_command', {
          cmd: `mv ${tmpPath} ${finalPath} && systemctl daemon-reload && systemctl enable ${serviceName}.service`,
          useSudo: true
        });

        showCreateModal = false;
        // Reset formularza
        serviceName = '';
        serviceDesc = '';
        serviceExec = '';
        serviceUser = 'root';
        serviceRestart = 'always';

        await loadServices();
      } catch (err: any) {
        if (err.toString() === 'SUDO_PASSWORD_REQUIRED') {
          pendingAction = run;
          showSudoModal = true;
        } else {
          errorMsg = 'Błąd instalacji usługi: ' + err.toString();
        }
      } finally {
        isLoading = false;
      }
    };
    await run();
  }

  onMount(() => {
    loadServices();
  });
</script>

<div class="services-manager fade-in">
  <header class="sm-header">
    <div class="title-area">
      <h1>Usługi Systemd</h1>
      <p class="subtitle">Zarządzaj i twórz demony systemowe Linux</p>
    </div>
    {#if errorMsg}
      <div class="error-badge">{errorMsg}</div>
    {/if}
  </header>

  <!-- Pasek operacji -->
  <div class="ops-bar glass">
    <input 
      type="text" 
      placeholder="Szukaj usług..." 
      class="search-input" 
      bind:value={searchQuery}
      oninput={filterServices}
    />
    <button class="secondary" onclick={loadServices} disabled={isLoading}>
      <RefreshCw size={16} class={isLoading ? 'spin' : ''} /> Odśwież
    </button>
    <button class="primary" onclick={() => showCreateModal = true}>
      <Plus size={16} /> Nowa Usługa
    </button>
  </div>

  <!-- Tabela usług -->
  <div class="table-container glass">
    {#if isLoading && services.length === 0}
      <div class="loading-state">
        <RefreshCw class="spin" size={32} />
        <p>Wczytywanie listy usług...</p>
      </div>
    {:else}
      <table class="services-table">
        <thead>
          <tr>
            <th width="25%">Nazwa Usługi</th>
            <th width="12%">Stan Wczytania</th>
            <th width="12%">Status</th>
            <th width="31%">Opis</th>
            <th width="20%" style="text-align: right;">Sterowanie</th>
          </tr>
        </thead>
        <tbody>
          {#each filteredServices as service}
            <tr>
              <td class="service-name-cell">
                <strong>{service.name}</strong>
              </td>
              <td>
                <span class="badge {service.load === 'loaded' ? 'success' : 'warning'}">
                  {service.load}
                </span>
              </td>
              <td>
                <span class="badge {service.active === 'active' ? 'success' : 'danger'}">
                  {service.sub}
                </span>
              </td>
              <td class="desc-cell" title={service.desc}>{service.desc || '(brak opisu)'}</td>
              <td class="actions-cell">
                {#if service.active === 'active'}
                  <button class="btn-action danger-text" onclick={() => executeServiceAction('stop', service.name)} title="Zatrzymaj">
                    <Square size={14} />
                  </button>
                {:else}
                  <button class="btn-action success-text" onclick={() => executeServiceAction('start', service.name)} title="Uruchom">
                    <Play size={14} />
                  </button>
                {/if}
                <button class="btn-action" onclick={() => executeServiceAction('restart', service.name)} title="Restartuj">
                  <RotateCw size={14} />
                </button>
              </td>
            </tr>
          {/each}

          {#if filteredServices.length === 0 && !isLoading}
            <tr>
              <td colspan="5" class="empty-state">Brak dopasowanych usług</td>
            </tr>
          {/if}
        </tbody>
      </table>
    {/if}
  </div>

  <!-- Sudo Password Prompt Modal -->
  {#if showSudoModal}
    <div class="modal-overlay">
      <div class="modal-content glass">
        <div class="modal-header-icon">
          <KeyRound size={32} class="accent-purple-text" />
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

  <!-- Kreator Nowej Usługi -->
  {#if showCreateModal}
    <div class="modal-overlay">
      <div class="modal-content glass creator-modal">
        <h3>Kreator Usługi Systemd</h3>
        
        <div class="form-group">
          <label for="srv-name">Nazwa usługi (np. myapp)</label>
          <input id="srv-name" type="text" placeholder="myapp" bind:value={serviceName} />
        </div>

        <div class="form-group">
          <label for="srv-desc">Opis usługi</label>
          <input id="srv-desc" type="text" placeholder="Opis działania programu..." bind:value={serviceDesc} />
        </div>

        <div class="form-group">
          <label for="srv-exec">Polecenie do uruchomienia (ExecStart)</label>
          <input id="srv-exec" type="text" placeholder="/usr/bin/node /var/www/app.js" bind:value={serviceExec} />
        </div>

        <div class="form-row">
          <div class="form-group">
            <label for="srv-user">Użytkownik uruchamiający</label>
            <input id="srv-user" type="text" bind:value={serviceUser} />
          </div>
          <div class="form-group">
            <label for="srv-restart">Restart (Restart policy)</label>
            <select id="srv-restart" bind:value={serviceRestart}>
              <option value="always">Always (Zawsze)</option>
              <option value="on-failure">On-Failure (Tylko błędy)</option>
              <option value="no">No (Nigdy)</option>
            </select>
          </div>
        </div>

        <div class="modal-actions">
          <button class="primary" onclick={createService} disabled={!serviceName || !serviceExec}>Zainstaluj usługę</button>
          <button class="secondary" onclick={() => showCreateModal = false}>Anuluj</button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .services-manager {
    padding: 30px;
    display: flex;
    flex-direction: column;
    gap: 24px;
    height: 100%;
    overflow: hidden;
  }

  .sm-header {
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

  .search-input {
    flex: 1;
  }

  /* Table */
  .table-container {
    flex: 1;
    overflow-y: auto;
    border-radius: var(--radius-md);
  }

  .services-table {
    width: 100%;
    border-collapse: collapse;
    text-align: left;
  }

  .services-table th, .services-table td {
    padding: 14px 20px;
    border-bottom: 1px solid var(--border-color);
  }

  .services-table th {
    font-size: 0.8rem;
    text-transform: uppercase;
    color: var(--text-muted);
    font-weight: 600;
    letter-spacing: 0.05em;
    position: sticky;
    top: 0;
    background: #0d121f;
    z-index: 1;
  }

  .services-table tr {
    transition: var(--transition-fast);
  }

  .services-table tr:hover {
    background: rgba(255, 255, 255, 0.02);
  }

  .service-name-cell strong {
    color: white;
    font-family: var(--font-sans);
  }

  .desc-cell {
    color: var(--text-secondary);
    font-size: 0.9rem;
    max-width: 300px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .actions-cell {
    text-align: right;
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .btn-action {
    background: transparent;
    border: none;
    padding: 6px;
    border-radius: 4px;
    color: var(--text-secondary);
    cursor: pointer;
    transition: var(--transition-fast);
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

  .creator-modal {
    width: 500px;
  }

  .modal-header-icon {
    display: flex;
    justify-content: center;
  }

  .modal-desc {
    font-size: 0.9rem;
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
    margin-top: 8px;
  }

  /* Form elements for Creator */
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

  .form-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
  }

  select {
    width: 100%;
  }
</style>
