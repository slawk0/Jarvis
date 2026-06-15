<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { Shield, ShieldOff, Plus, Trash2, RefreshCw, KeyRound, Check, ShieldAlert } from 'lucide-svelte';

  let ufwActive = $state(false);
  let rules = $state<any[]>([]);
  let isLoading = $state(false);
  let errorMsg = $state('');

  // Sudo auth state
  let showSudoModal = $state(false);
  let sudoPassword = $state('');
  let pendingAction: (() => Promise<void>) | null = null;
  let sudoError = $state('');

  // Formularz nowej reguły
  let showAddModal = $state(false);
  let ruleAction = $state('allow');
  let rulePort = $state('');
  let ruleProto = $state('any');
  let ruleSource = $state('Anywhere');

  async function loadUfwStatus() {
    isLoading = true;
    errorMsg = '';
    try {
      // UFW wymaga sudo do statusu
      const statusOut: string = await invoke('exec_custom_command', {
        cmd: 'ufw status numbered',
        useSudo: true
      });

      if (statusOut.includes('inactive') || statusOut.includes('nieaktywny')) {
        ufwActive = false;
        rules = [];
      } else {
        ufwActive = true;
        
        // Parsowanie reguł ufw status numbered
        // Przykład: [ 1] 22/tcp                     ALLOW IN    Anywhere
        const lines = statusOut.trim().split('\n');
        let parsedRules = [];
        
        for (const line of lines) {
          const match = line.match(/^\s*\[\s*(\d+)\]\s+(.*?)\s+(ALLOW|DENY|REJECT)\s+(IN|OUT)?\s+(.*?)$/i);
          if (match) {
            parsedRules.push({
              num: parseInt(match[1]),
              to: match[2].trim(),
              action: match[3].trim(),
              direction: match[4] ? match[4].trim() : 'IN',
              from: match[5].trim()
            });
          }
        }
        
        rules = parsedRules;
      }
    } catch (err: any) {
      if (err.toString() === 'SUDO_PASSWORD_REQUIRED') {
        pendingAction = loadUfwStatus;
        showSudoModal = true;
      } else {
        errorMsg = 'Błąd wczytywania zapory UFW: ' + err.toString() + '. Upewnij się, że pakiet `ufw` jest zainstalowany na serwerze.';
      }
    } finally {
      isLoading = false;
    }
  }

  async function handleActionWithSudo(action: () => Promise<void>) {
    const run = async () => {
      try {
        await action();
      } catch (err: any) {
        if (err.toString() === 'SUDO_PASSWORD_REQUIRED') {
          pendingAction = run;
          showSudoModal = true;
        } else {
          errorMsg = 'Błąd wykonania akcji: ' + err.toString();
        }
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

  async function toggleUfw() {
    const action = async () => {
      isLoading = true;
      errorMsg = '';
      const cmd = ufwActive ? 'ufw disable' : 'ufw --force enable';
      await invoke('exec_custom_command', { cmd, useSudo: true });
      await loadUfwStatus();
    };
    await handleActionWithSudo(action);
  }

  async function addRule() {
    if (!rulePort) return;
    
    const action = async () => {
      isLoading = true;
      errorMsg = '';
      
      // Konstruowanie komendy ufw
      // ufw allow proto tcp from 192.168.1.1 to any port 80
      let cmd = `ufw ${ruleAction}`;
      if (ruleProto !== 'any') {
        cmd += ` proto ${ruleProto}`;
      }
      if (ruleSource !== 'Anywhere' && ruleSource !== '') {
        cmd += ` from ${ruleSource}`;
      }
      cmd += ` to any port ${rulePort}`;
      
      await invoke('exec_custom_command', { cmd, useSudo: true });
      showAddModal = false;
      rulePort = '';
      ruleSource = 'Anywhere';
      ruleProto = 'any';
      await loadUfwStatus();
    };
    
    await handleActionWithSudo(action);
  }

  async function deleteRule(num: number) {
    if (confirm(`Czy na pewno chcesz usunąć regułę o numerze ${num}?`)) {
      const action = async () => {
        isLoading = true;
        errorMsg = '';
        await invoke('exec_custom_command', {
          cmd: `ufw --force delete ${num}`,
          useSudo: true
        });
        await loadUfwStatus();
      };
      await handleActionWithSudo(action);
    }
  }

  onMount(() => {
    loadUfwStatus();
  });
</script>

<div class="firewall-manager fade-in">
  <header class="fm-header">
    <div class="title-area">
      <h1>Zapora Sieciowa (UFW)</h1>
      <p class="subtitle">Zabezpiecz serwer blokując lub zezwalając na ruch na portach</p>
    </div>
    {#if errorMsg}
      <div class="error-badge">{errorMsg}</div>
    {/if}
  </header>

  <!-- Pasek stanu zapory -->
  <div class="status-bar glass">
    <div class="status-indicator">
      {#if ufwActive}
        <Shield size={24} class="shield-icon active" />
        <div>
          <span class="status-title">Zapora jest AKTYWNA</span>
          <span class="status-desc">Ruch na serwerze jest filtrowany według poniższych reguł</span>
        </div>
      {:else}
        <ShieldOff size={24} class="shield-icon inactive" />
        <div>
          <span class="status-title">Zapora jest NIEAKTYWNA</span>
          <span class="status-desc">Wszystkie porty wejściowe i wyjściowe są otwarte</span>
        </div>
      {/if}
    </div>

    <div class="status-actions">
      <button class="secondary" onclick={loadUfwStatus} disabled={isLoading}>
        <RefreshCw size={16} class={isLoading ? 'spin' : ''} /> Odśwież
      </button>
      <button class={ufwActive ? 'danger' : 'primary'} onclick={toggleUfw} disabled={isLoading}>
        {ufwActive ? 'Wyłącz zaporę' : 'Włącz zaporę'}
      </button>
    </div>
  </div>

  {#if ufwActive}
    <!-- Sekcja operacyjna dla reguł -->
    <div class="rules-header">
      <h2>Aktywne Reguły Zapory</h2>
      <button class="primary" onclick={() => showAddModal = true}>
        <Plus size={16} /> Dodaj Regułę
      </button>
    </div>

    <div class="table-container glass">
      <table class="rules-table">
        <thead>
          <tr>
            <th style="width: 10%;">Nr</th>
            <th style="width: 25%;">Port / Usługa</th>
            <th style="width: 20%;">Akcja</th>
            <th style="width: 25%;">Z adresu IP</th>
            <th style="width: 20%; text-align: right;">Usuń</th>
          </tr>
        </thead>
        <tbody>
          {#each rules as rule}
            <tr>
              <td><span class="badge warning mono-val">{rule.num}</span></td>
              <td class="mono-val"><strong>{rule.to}</strong></td>
              <td>
                <span class="badge {rule.action.toUpperCase() === 'ALLOW' ? 'success' : 'danger'}">
                  {rule.action}
                </span>
              </td>
              <td class="mono-val"><code>{rule.from}</code></td>
              <td class="actions-cell">
                <button class="btn-table danger-text" onclick={() => deleteRule(rule.num)} title="Usuń regułę">
                  <Trash2 size={14} />
                </button>
              </td>
            </tr>
          {/each}

          {#if rules.length === 0}
            <tr>
              <td colspan="5" class="empty-state">Brak skonfigurowanych reguł. UFW blokuje domyślnie ruch wejściowy.</td>
            </tr>
          {/if}
        </tbody>
      </table>
    </div>
  {/if}

  <!-- Sudo Password Prompt Modal -->
  {#if showSudoModal}
    <div class="modal-overlay">
      <div class="modal-content glass">
        <div class="modal-header-icon">
          <KeyRound size={32} class="accent-amber-text" />
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

  <!-- Kreator Nowej Reguły -->
  {#if showAddModal}
    <div class="modal-overlay">
      <div class="modal-content glass">
        <h3>Dodaj regułę do zapory</h3>
        
        <div class="form-group">
          <label for="rule-action">Działanie (Action)</label>
          <select id="rule-action" bind:value={ruleAction}>
            <option value="allow">ALLOW (Zezwól)</option>
            <option value="deny">DENY (Blokuj)</option>
          </select>
        </div>

        <div class="form-group">
          <label for="rule-port">Port lub porty (np. 80, 443, 3000:3005)</label>
          <input id="rule-port" type="text" placeholder="8080" bind:value={rulePort} />
        </div>

        <div class="form-group">
          <label for="rule-proto">Protokół (Protocol)</label>
          <select id="rule-proto" bind:value={ruleProto}>
            <option value="any">Dowolny (Any)</option>
            <option value="tcp">TCP</option>
            <option value="udp">UDP</option>
          </select>
        </div>

        <div class="form-group">
          <label for="rule-source">Dozwolony źródłowy adres IP / podsieć</label>
          <input id="rule-source" type="text" placeholder="Anywhere (lub np. 192.168.1.50)" bind:value={ruleSource} />
        </div>

        <div class="modal-actions">
          <button class="primary" onclick={addRule} disabled={!rulePort}>Dodaj regułę</button>
          <button class="secondary" onclick={() => { showAddModal = false; rulePort = ''; }}>Anuluj</button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .firewall-manager {
    padding: 30px;
    display: flex;
    flex-direction: column;
    gap: 24px;
    height: 100%;
    overflow: hidden;
  }

  .fm-header {
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

  /* Status Bar */
  .status-bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 20px 24px;
    border-radius: var(--radius-md);
    flex-shrink: 0;
  }

  .status-indicator {
    display: flex;
    align-items: center;
    gap: 20px;
  }

  .shield-icon {
    padding: 12px;
    border-radius: 50%;
    box-shadow: 0 0 15px rgba(255, 255, 255, 0.05);
  }

  .shield-icon.active {
    color: var(--accent-green);
    background: rgba(16, 185, 129, 0.1);
    box-shadow: 0 0 15px var(--accent-green-glow);
    border: 1px solid rgba(16, 185, 129, 0.2);
  }

  .shield-icon.inactive {
    color: var(--text-muted);
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid var(--border-color);
  }

  .status-title {
    display: block;
    font-size: 1.1rem;
    font-weight: 600;
    color: white;
  }

  .status-desc {
    display: block;
    font-size: 0.85rem;
    color: var(--text-secondary);
    margin-top: 2px;
  }

  .status-actions {
    display: flex;
    gap: 10px;
  }

  /* Rules list */
  .rules-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-top: 8px;
    flex-shrink: 0;
  }

  .rules-header h2 {
    font-size: 1.2rem;
    color: white;
  }

  /* Table */
  .table-container {
    flex: 1;
    overflow-y: auto;
    border-radius: var(--radius-md);
  }

  .rules-table {
    width: 100%;
    border-collapse: collapse;
    text-align: left;
  }

  .rules-table th, .rules-table td {
    padding: 14px 20px;
    border-bottom: 1px solid var(--border-color);
  }

  .rules-table th {
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

  .rules-table tr {
    transition: var(--transition-fast);
  }

  .rules-table tr:hover {
    background: rgba(255, 255, 255, 0.02);
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
    width: 400px;
    padding: 24px;
    border-radius: var(--radius-md);
    display: flex;
    flex-direction: column;
    gap: 16px;
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
</style>
