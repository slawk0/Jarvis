<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { Play, Square, RotateCw, Plus, RefreshCw, Eye, KeyRound, Search, X } from 'lucide-svelte';
  import SortableTh from './ui/SortableTh.svelte';
  import { applySort, nextSort, type SortState } from '$lib/sort/sortUtils';
  import { stickToBottom } from '$lib/stickToBottom';
  import { get } from 'svelte/store';
  import { LL } from '$lib/i18n/i18n-svelte';
  import { notifications } from '$lib/notifications.svelte';
  import {
    formatInvokeError,
    isSudoPasswordIncorrect,
    isSudoPasswordRequired,
  } from '$lib/i18n/backendErrors';

  let services = $state<any[]>([]);
  let filteredServices = $state<any[]>([]);
  type ServiceSortCol = 'name' | 'load' | 'status' | 'desc';
  let serviceSort = $state<SortState<ServiceSortCol>>({ column: 'name', direction: 'asc' });

  const sortedServices = $derived(
    applySort(filteredServices, serviceSort, {
      name: (s) => s.name || '',
      load: (s) => s.load || '',
      status: (s) => s.sub || '',
      desc: (s) => s.desc || '',
    }),
  );

  function setServiceSort(column: string) {
    serviceSort = nextSort(serviceSort, column as ServiceSortCol);
  }
  let searchQuery = $state('');
  let isLoading = $state(false);
  let errorMsg = $state('');

  $effect(() => {
    if (errorMsg) {
      notifications.error(errorMsg);
      errorMsg = '';
    }
  });

  // Sudo authentication management
  let showSudoModal = $state(false);
  let sudoPassword = $state('');
  let pendingAction: (() => Promise<void>) | null = null;
  let sudoError = $state('');

  // Modals and new service creator
  let showCreateModal = $state(false);
  let serviceName = $state('');
  let serviceDesc = $state('');
  let serviceExec = $state('');
  let serviceUser = $state('root');
  let serviceRestart = $state('always');

  // Service status / logs modal
  let showStatusModal = $state(false);
  let statusServiceName = $state('');
  let statusTab = $state<'status' | 'logs'>('status');
  let statusContent = $state('');
  let statusLoading = $state(false);
  let statusSearch = $state('');

  const filteredStatusContent = $derived.by(() => {
    if (!statusSearch) return statusContent;
    const q = statusSearch.toLowerCase();
    return statusContent
      .split('\n')
      .filter((line) => line.toLowerCase().includes(q))
      .join('\n');
  });

  async function loadStatusContent() {
    if (!statusServiceName) return;
    statusLoading = true;
    try {
      const cmd =
        statusTab === 'status'
          ? `systemctl status ${statusServiceName}.service -l --no-pager`
          : `journalctl -u ${statusServiceName}.service -n 200 --no-pager`;
      statusContent = await invoke<string>('exec_custom_command', { cmd, useSudo: false });
    } catch (err: unknown) {
      statusContent = get(LL).common.loadFailed({ error: formatInvokeError(err) });
    } finally {
      statusLoading = false;
    }
  }

  async function openServiceStatus(name: string) {
    statusServiceName = name;
    statusTab = 'status';
    statusSearch = '';
    statusContent = '';
    showStatusModal = true;
    await loadStatusContent();
  }

  async function switchStatusTab(tab: 'status' | 'logs') {
    statusTab = tab;
    statusSearch = '';
    await loadStatusContent();
  }

  function closeStatusModal() {
    showStatusModal = false;
    statusServiceName = '';
    statusContent = '';
    statusSearch = '';
  }

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
        // Elements can contain spaces, we split them matching the first 4 columns
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
    } catch (err: unknown) {
      errorMsg = get(LL).services.loadFailed({ error: formatInvokeError(err) });
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
        await invoke('secure_systemctl', { action, service: serviceName });
        await loadServices();
      } catch (err: unknown) {
        if (isSudoPasswordRequired(err)) {
          pendingAction = run;
          showSudoModal = true;
        } else if (isSudoPasswordIncorrect(err)) {
          errorMsg = get(LL).common.sudoIncorrect();
        } else {
          errorMsg = get(LL).services.actionFailed({ action, error: formatInvokeError(err) });
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
    } catch (err: unknown) {
      sudoError = formatInvokeError(err);
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
      } catch (err: unknown) {
        if (isSudoPasswordRequired(err)) {
          pendingAction = run;
          showSudoModal = true;
        } else {
          errorMsg = get(LL).services.installFailed({ error: formatInvokeError(err) });
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

<div class="services-manager manager-shell fade-in">
  <header class="manager-header">
    <h1 class="page-title">{$LL.services.title()}</h1>
  </header>

  <!-- Pasek operacji -->
  <div class="ops-bar glass">
    <input 
      type="text" 
      placeholder={$LL.services.searchPlaceholder()} 
      class="search-input" 
      bind:value={searchQuery}
      oninput={filterServices}
    />
    <button class="secondary" onclick={loadServices} disabled={isLoading}>
      <RefreshCw size={16} class={isLoading ? 'spin' : ''} /> {$LL.common.refresh()}
    </button>
    <button class="primary" onclick={() => showCreateModal = true}>
      <Plus size={16} /> {$LL.services.newService()}
    </button>
  </div>

  <!-- Services table -->
  <div class="table-container glass">
    {#if isLoading && services.length === 0}
      <div class="loading-state">
        <RefreshCw class="spin" size={32} />
        <p>{$LL.services.loading()}</p>
      </div>
    {:else}
      <table class="services-table">
        <thead>
          <tr>
            <SortableTh label={$LL.services.serviceName()} column="name" activeColumn={serviceSort.column} direction={serviceSort.direction} onsort={setServiceSort} width="25%" />
            <SortableTh label={$LL.services.loadState()} column="load" activeColumn={serviceSort.column} direction={serviceSort.direction} onsort={setServiceSort} width="12%" />
            <SortableTh label={$LL.services.status()} column="status" activeColumn={serviceSort.column} direction={serviceSort.direction} onsort={setServiceSort} width="12%" />
            <SortableTh label={$LL.services.description()} column="desc" activeColumn={serviceSort.column} direction={serviceSort.direction} onsort={setServiceSort} width="31%" />
            <th style="width: 20%; text-align: right; padding: 14px 16px; font-size: 0.8rem; text-transform: uppercase; color: var(--text-muted); font-weight: 600;">{$LL.services.control()}</th>
          </tr>
        </thead>
        <tbody>
          {#each sortedServices as service}
            <tr>
              <td class="service-name-cell mono-val">
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
              <td class="desc-cell" title={service.desc}>{service.desc || $LL.common.noDescription()}</td>
              <td class="actions-cell">
                <button class="btn-action" onclick={() => openServiceStatus(service.name)} title={$LL.services.statusAndLogs()}>
                  <Eye size={14} />
                </button>
                {#if service.active === 'active'}
                  <button class="btn-action danger-text" onclick={() => executeServiceAction('stop', service.name)} title={$LL.services.stop()}>
                    <Square size={14} />
                  </button>
                {:else}
                  <button class="btn-action success-text" onclick={() => executeServiceAction('start', service.name)} title={$LL.services.start()}>
                    <Play size={14} />
                  </button>
                {/if}
                <button class="btn-action" onclick={() => executeServiceAction('restart', service.name)} title={$LL.services.restart()}>
                  <RotateCw size={14} />
                </button>
              </td>
            </tr>
          {/each}

          {#if sortedServices.length === 0 && !isLoading}
            <tr>
              <td colspan="5" class="empty-state">{$LL.services.empty()}</td>
            </tr>
          {/if}
        </tbody>
      </table>
    {/if}
  </div>

  <!-- Service status / logs modal -->
  {#if showStatusModal}
    <div class="modal-overlay status-overlay">
      <div class="modal-content glass status-modal">
        <div class="status-header">
          <h3>{statusServiceName}.service</h3>
          <div class="status-controls">
            <div class="status-tabs">
              <button
                class="status-tab"
                class:active={statusTab === 'status'}
                onclick={() => switchStatusTab('status')}
              >
                {$LL.services.statusTab()}
              </button>
              <button
                class="status-tab"
                class:active={statusTab === 'logs'}
                onclick={() => switchStatusTab('logs')}
              >
                {$LL.services.logsTab()}
              </button>
            </div>
            <div class="search-bar search-bar-sm">
              <span class="search-icon-wrapper"><Search size={14} /></span>
              <input type="text" placeholder={$LL.services.filter()} bind:value={statusSearch} />
            </div>
            <button class="secondary btn-sm" onclick={loadStatusContent} disabled={statusLoading}>
              <RefreshCw size={14} class={statusLoading ? 'spin' : ''} />
            </button>
            <button class="secondary btn-sm" onclick={closeStatusModal}>
              <X size={14} />
            </button>
          </div>
        </div>
        <div class="status-display" use:stickToBottom>
          {#if statusLoading && !statusContent}
            <div class="status-loading">
              <RefreshCw class="spin" size={24} />
              <span>{$LL.common.loading()}</span>
            </div>
          {:else}
            <pre class="status-text">{filteredStatusContent || $LL.common.noData()}</pre>
          {/if}
        </div>
      </div>
    </div>
  {/if}

  <!-- Sudo Password Prompt Modal -->
  {#if showSudoModal}
    <div class="modal-overlay">
      <div class="modal-content glass">
        <div class="modal-header-icon">
          <KeyRound size={32} class="accent-amber-text" />
        </div>
        <h3>{$LL.sudo.authTitle()}</h3>
        <p class="modal-desc">{$LL.sudo.authDesc()}</p>
        <input 
          type="password" 
          placeholder={$LL.sudo.passwordInputPlaceholder()} 
          bind:value={sudoPassword} 
          onkeydown={(e) => e.key === 'Enter' && submitSudoPassword()}
        />
        {#if sudoError}
          <span class="error-text">{sudoError}</span>
        {/if}
        <div class="modal-actions">
          <button class="primary" onclick={submitSudoPassword}>{$LL.common.submit()}</button>
          <button class="secondary" onclick={() => { showSudoModal = false; sudoPassword = ''; pendingAction = null; }}>{$LL.common.cancel()}</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- New Service Creator -->
  {#if showCreateModal}
    <div class="modal-overlay">
      <div class="modal-content glass creator-modal">
        <h3>{$LL.services.creatorTitle()}</h3>
        
        <div class="form-group">
          <label for="srv-name">{$LL.services.nameLabel()}</label>
          <input id="srv-name" type="text" placeholder={$LL.services.namePlaceholder()} bind:value={serviceName} />
        </div>

        <div class="form-group">
          <label for="srv-desc">{$LL.services.descLabel()}</label>
          <input id="srv-desc" type="text" placeholder={$LL.services.descPlaceholder()} bind:value={serviceDesc} />
        </div>

        <div class="form-group">
          <label for="srv-exec">{$LL.services.execLabel()}</label>
          <input id="srv-exec" type="text" placeholder={$LL.services.execPlaceholder()} bind:value={serviceExec} />
        </div>

        <div class="form-row">
          <div class="form-group">
            <label for="srv-user">{$LL.services.runAsUser()}</label>
            <input id="srv-user" type="text" bind:value={serviceUser} />
          </div>
          <div class="form-group">
            <label for="srv-restart">{$LL.services.restartPolicy()}</label>
            <select id="srv-restart" bind:value={serviceRestart}>
              <option value="always">{$LL.services.restartAlways()}</option>
              <option value="on-failure">{$LL.services.restartOnFailure()}</option>
              <option value="no">{$LL.services.restartNo()}</option>
            </select>
          </div>
        </div>

        <div class="modal-actions">
          <button class="primary" onclick={createService} disabled={!serviceName || !serviceExec}>{$LL.services.installService()}</button>
          <button class="secondary" onclick={() => showCreateModal = false}>{$LL.common.cancel()}</button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .services-manager {
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
    padding: 8px 12px;
    border-bottom: 1px solid var(--border-color);
  }

  .services-table td {
    font-size: 0.9rem;
  }

  .services-table th {
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

  /* Status / logs modal */
  .status-overlay {
    z-index: 110;
  }

  .status-modal {
    width: 85vw;
    max-width: 1100px;
    height: 75vh;
    max-height: 700px;
  }

  .status-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-shrink: 0;
    gap: 12px;
    flex-wrap: wrap;
  }

  .status-header h3 {
    font-size: 1rem;
    font-family: var(--font-mono);
    white-space: nowrap;
  }

  .status-controls {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .status-tabs {
    display: flex;
    gap: 2px;
    background: var(--bg-primary);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    padding: 2px;
  }

  .status-tab {
    background: transparent;
    border: none;
    padding: 5px 12px;
    border-radius: 4px;
    font-size: 0.8rem;
    color: var(--text-secondary);
    cursor: pointer;
    transition: var(--transition-fast);
  }

  .status-tab:hover {
    color: var(--text-primary);
  }

  .status-tab.active {
    background: var(--bg-hover);
    color: var(--text-primary);
    font-weight: 500;
  }

  .search-bar-sm {
    position: relative;
    min-width: 160px;
    max-width: 200px;
  }

  .search-bar-sm input {
    width: 100%;
    padding: 6px 10px 6px 34px;
    font-size: 0.8rem;
  }

  .search-bar-sm .search-icon-wrapper {
    position: absolute;
    left: 10px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--text-muted);
    pointer-events: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .status-display {
    flex: 1;
    overflow: auto;
    background: var(--bg-primary);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    padding: 12px;
    min-height: 0;
  }

  .status-loading {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 12px;
    height: 100%;
    color: var(--text-secondary);
    font-size: 0.9rem;
  }

  .status-text {
    font-family: var(--font-mono);
    font-size: 0.75rem;
    line-height: 1.55;
    color: #d1d4db;
    white-space: pre-wrap;
    word-break: break-all;
    user-select: text;
  }
</style>
