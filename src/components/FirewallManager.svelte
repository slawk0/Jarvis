<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { Shield, ShieldOff, Plus, Trash2, RefreshCw, KeyRound, Check, ShieldAlert } from 'lucide-svelte';
  import SortableTh from './ui/SortableTh.svelte';
  import { applySort, nextSort, type SortState } from '$lib/sort/sortUtils';

  let firewallMode = $state<'ufw' | 'iptables'>('ufw');

  // UFW state
  let ufwActive = $state(false);
  let rules = $state<any[]>([]);
  
  // iptables state
  let iptablesChains = $state<{name: string, policy: string, rules: any[]}[]>([]);
  let activeChain = $state('INPUT');

  type RuleSortCol = 'num' | 'to' | 'action' | 'from';
  let ruleSort = $state<SortState<RuleSortCol>>({ column: 'num', direction: 'asc' });

  const sortedRules = $derived(
    applySort(rules, ruleSort, {
      num: (r) => parseInt(String(r.num).replace(/\D/g, ''), 10) || 0,
      to: (r) => r.to || '',
      action: (r) => r.action || '',
      from: (r) => r.from || '',
    })
  );

  function setRuleSort(column: string) {
    ruleSort = nextSort(ruleSort, column as RuleSortCol);
  }

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
      const statusOut: string = await invoke('exec_custom_command', {
        cmd: 'ufw status numbered',
        useSudo: true
      });

      if (statusOut.includes('inactive') || statusOut.includes('nieaktywny')) {
        ufwActive = false;
        rules = [];
      } else {
        ufwActive = true;
        
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
      const errStr = err.toString().toLowerCase();
      if (errStr === 'sudo_password_required') {
        pendingAction = loadUfwStatus;
        showSudoModal = true;
      } else if (errStr.includes('not found') || errStr.includes('nie znaleziono')) {
        ufwActive = false;
        rules = [];
        if (firewallMode === 'ufw') {
          errorMsg = 'UFW nie jest zainstalowane na tym serwerze. Możesz przełączyć się na iptables.';
        }
      } else {
        errorMsg = 'Błąd wczytywania zapory UFW: ' + err.toString();
      }
    } finally {
      isLoading = false;
    }
  }

  async function loadIptablesStatus() {
    isLoading = true;
    errorMsg = '';
    try {
      const out: string = await invoke('exec_custom_command', {
        cmd: 'iptables -L -n --line-numbers',
        useSudo: true
      });
      
      const lines = out.trim().split('\n');
      let currentChain: any = null;
      let chains: any[] = [];
      
      for (const line of lines) {
        if (line.startsWith('Chain ')) {
          const chainMatch = line.match(/^Chain\s+(\S+)\s+\(policy\s+(\S+)\)/);
          if (chainMatch) {
            currentChain = { name: chainMatch[1], policy: chainMatch[2], rules: [] };
            chains.push(currentChain);
          } else {
            const noPolicyMatch = line.match(/^Chain\s+(\S+)/);
            if (noPolicyMatch) {
              currentChain = { name: noPolicyMatch[1], policy: '-', rules: [] };
              chains.push(currentChain);
            }
          }
        } else if (line.match(/^\d+\s+/) && currentChain) {
          const parts = line.trim().split(/\s+/);
          const num = parts[0];
          const target = parts[1];
          const prot = parts[2];
          const opt = parts[3];
          const source = parts[4];
          const destination = parts[5];
          const extra = parts.slice(6).join(' ');
          
          currentChain.rules.push({ num, target, prot, opt, source, destination, extra });
        }
      }
      iptablesChains = chains;
      if (!chains.find(c => c.name === activeChain) && chains.length > 0) {
        activeChain = chains[0].name;
      }
    } catch (err: any) {
      if (err.toString() === 'SUDO_PASSWORD_REQUIRED') {
        pendingAction = loadIptablesStatus;
        showSudoModal = true;
      } else {
        errorMsg = 'Błąd wczytywania reguł iptables: ' + err.toString();
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
    if (firewallMode === 'ufw') {
      if (!rulePort) return;
      const action = async () => {
        isLoading = true;
        errorMsg = '';
        let cmd = `ufw ${ruleAction}`;
        if (ruleProto !== 'any') cmd += ` proto ${ruleProto}`;
        if (ruleSource !== 'Anywhere' && ruleSource !== '') cmd += ` from ${ruleSource}`;
        cmd += ` to any port ${rulePort}`;
        await invoke('exec_custom_command', { cmd, useSudo: true });
        closeAddModal();
        await loadUfwStatus();
      };
      await handleActionWithSudo(action);
    } else {
      const action = async () => {
        isLoading = true;
        errorMsg = '';
        let prot = ruleProto;
        if (rulePort && prot === 'any') {
          prot = 'tcp';
        }
        let target = ruleAction.toUpperCase() === 'ALLOW' ? 'ACCEPT' : 'DROP';
        if (ruleAction.toLowerCase() === 'deny') target = 'DROP';
        else if (ruleAction.toLowerCase() === 'reject') target = 'REJECT';

        let cmd = `iptables -A ${activeChain}`;
        if (prot !== 'any') {
          cmd += ` -p ${prot}`;
          if (rulePort && rulePort !== 'any') cmd += ` --dport ${rulePort}`;
        }
        if (ruleSource && ruleSource !== 'Anywhere') cmd += ` -s ${ruleSource}`;
        cmd += ` -j ${target}`;

        await invoke('exec_custom_command', { cmd, useSudo: true });
        closeAddModal();
        await loadIptablesStatus();
      };
      await handleActionWithSudo(action);
    }
  }

  function closeAddModal() {
    showAddModal = false;
    rulePort = '';
    ruleSource = 'Anywhere';
    ruleProto = 'any';
  }

  async function deleteRule(num: number) {
    if (firewallMode === 'ufw') {
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
    } else {
      if (confirm(`Czy na pewno chcesz usunąć regułę nr ${num} z łańcucha ${activeChain}?`)) {
        const action = async () => {
          isLoading = true;
          errorMsg = '';
          await invoke('exec_custom_command', {
            cmd: `iptables -D ${activeChain} ${num}`,
            useSudo: true
          });
          await loadIptablesStatus();
        };
        await handleActionWithSudo(action);
      }
    }
  }

  async function setIptablesPolicy(policy: string) {
    if (confirm(`Zmienić domyślną politykę łańcucha ${activeChain} na ${policy}?`)) {
      const action = async () => {
        isLoading = true;
        errorMsg = '';
        await invoke('exec_custom_command', {
          cmd: `iptables -P ${activeChain} ${policy}`,
          useSudo: true
        });
        await loadIptablesStatus();
      };
      await handleActionWithSudo(action);
    }
  }

  function switchMode(mode: 'ufw' | 'iptables') {
    firewallMode = mode;
    errorMsg = '';
    if (mode === 'ufw') {
      loadUfwStatus();
    } else {
      loadIptablesStatus();
    }
  }

  onMount(() => {
    loadUfwStatus();
  });

  let activeChainData = $derived(iptablesChains.find(c => c.name === activeChain));
</script>

<div class="firewall-manager manager-shell fade-in">
  <header class="manager-header">
    <div class="header-content">
      <h1 class="page-title">Zapora Sieciowa</h1>
      <div class="mode-selector">
        <button class="mode-btn {firewallMode === 'ufw' ? 'active' : ''}" onclick={() => switchMode('ufw')}>UFW</button>
        <button class="mode-btn {firewallMode === 'iptables' ? 'active' : ''}" onclick={() => switchMode('iptables')}>iptables</button>
      </div>
    </div>
    {#if errorMsg}
      <div class="error-badge">{errorMsg}</div>
    {/if}
  </header>

  {#if firewallMode === 'ufw'}
    <!-- Pasek stanu zapory UFW -->
    <div class="status-bar glass">
      <div class="status-indicator">
        {#if ufwActive}
          <Shield size={16} class="shield-icon active" />
          <span class="status-title">UFW AKTYWNE</span>
        {:else}
          <ShieldOff size={16} class="shield-icon inactive" />
          <span class="status-title">UFW NIEAKTYWNE</span>
        {/if}
      </div>
      <div class="status-actions">
        <button class="secondary" onclick={loadUfwStatus} disabled={isLoading}>
          <RefreshCw size={16} class={isLoading ? 'spin' : ''} /> Odśwież
        </button>
        <button class={ufwActive ? 'danger' : 'primary'} onclick={toggleUfw} disabled={isLoading}>
          {ufwActive ? 'Wyłącz UFW' : 'Włącz UFW'}
        </button>
      </div>
    </div>

    {#if ufwActive}
      <!-- Sekcja operacyjna dla reguł UFW -->
      <div class="rules-header">
        <h2>Aktywne Reguły UFW</h2>
        <button class="primary" onclick={() => showAddModal = true}>
          <Plus size={16} /> Dodaj Regułę
        </button>
      </div>

      <div class="table-container glass">
        <table class="rules-table">
          <thead>
            <tr>
              <SortableTh label="Nr" column="num" activeColumn={ruleSort.column} direction={ruleSort.direction} onsort={setRuleSort} width="10%" />
              <SortableTh label="Port / Usługa" column="to" activeColumn={ruleSort.column} direction={ruleSort.direction} onsort={setRuleSort} width="25%" />
              <SortableTh label="Akcja" column="action" activeColumn={ruleSort.column} direction={ruleSort.direction} onsort={setRuleSort} width="20%" />
              <SortableTh label="Z adresu IP" column="from" activeColumn={ruleSort.column} direction={ruleSort.direction} onsort={setRuleSort} width="25%" />
              <th style="width: 20%; text-align: right; padding: 14px 16px; font-size: 0.8rem; text-transform: uppercase; color: var(--text-muted); font-weight: 600;">Usuń</th>
            </tr>
          </thead>
          <tbody>
            {#each sortedRules as rule}
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

            {#if sortedRules.length === 0}
              <tr>
                <td colspan="5" class="empty-state">Brak skonfigurowanych reguł. UFW blokuje domyślnie ruch wejściowy.</td>
              </tr>
            {/if}
          </tbody>
        </table>
      </div>
    {/if}

  {:else}
    <!-- IPTABLES MODE -->
    <div class="status-bar glass iptables-header">
      <div class="iptables-chain-selector">
        <label for="chain-select" class="status-title">Wybierz Łańcuch:</label>
        <select id="chain-select" class="form-select" bind:value={activeChain}>
          {#each iptablesChains as chain}
            <option value={chain.name}>{chain.name} (Polityka: {chain.policy})</option>
          {/each}
        </select>
      </div>

      <div class="status-actions">
        <button class="secondary" onclick={loadIptablesStatus} disabled={isLoading}>
          <RefreshCw size={16} class={isLoading ? 'spin' : ''} /> Odśwież
        </button>
      </div>
    </div>

    {#if activeChainData && activeChainData.policy !== '-'}
      <div class="policy-bar">
        <span>Domyślna polityka: <strong>{activeChainData.policy}</strong></span>
        <div class="policy-actions">
          <button class="btn-sm {activeChainData.policy === 'ACCEPT' ? 'active-policy success' : 'secondary'}" onclick={() => setIptablesPolicy('ACCEPT')}>ACCEPT</button>
          <button class="btn-sm {activeChainData.policy === 'DROP' ? 'active-policy danger' : 'secondary'}" onclick={() => setIptablesPolicy('DROP')}>DROP</button>
        </div>
      </div>
    {/if}

    <div class="rules-header">
      <h2>Reguły iptables: {activeChain}</h2>
      <button class="primary" onclick={() => showAddModal = true}>
        <Plus size={16} /> Dodaj Regułę
      </button>
    </div>

    <div class="table-container glass">
      <table class="rules-table iptables-table">
        <thead>
          <tr>
            <th style="width: 5%">Nr</th>
            <th style="width: 15%">Cel (Target)</th>
            <th style="width: 10%">Protokół</th>
            <th style="width: 20%">Źródło</th>
            <th style="width: 20%">Cel (IP)</th>
            <th style="width: 20%">Dodatkowe (Porty itp.)</th>
            <th style="width: 10%; text-align: right;">Usuń</th>
          </tr>
        </thead>
        <tbody>
          {#if activeChainData && activeChainData.rules.length > 0}
            {#each activeChainData.rules as rule}
              <tr>
                <td><span class="badge warning mono-val">{rule.num}</span></td>
                <td>
                  <span class="badge {rule.target === 'ACCEPT' ? 'success' : (rule.target === 'DROP' || rule.target === 'REJECT' ? 'danger' : 'neutral')}">
                    {rule.target}
                  </span>
                </td>
                <td class="mono-val">{rule.prot}</td>
                <td class="mono-val"><code>{rule.source}</code></td>
                <td class="mono-val"><code>{rule.destination}</code></td>
                <td class="mono-val small-text">{rule.extra}</td>
                <td class="actions-cell">
                  <button class="btn-table danger-text" onclick={() => deleteRule(parseInt(rule.num))} title="Usuń regułę">
                    <Trash2 size={14} />
                  </button>
                </td>
              </tr>
            {/each}
          {:else}
            <tr>
              <td colspan="7" class="empty-state">Brak reguł w tym łańcuchu.</td>
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
        <h3>Dodaj regułę ({firewallMode === 'ufw' ? 'UFW' : 'iptables'})</h3>
        
        <div class="form-group">
          <label for="rule-action">Działanie (Action)</label>
          <select id="rule-action" bind:value={ruleAction}>
            <option value="allow">{firewallMode === 'ufw' ? 'ALLOW (Zezwól)' : 'ACCEPT (Zezwól)'}</option>
            <option value="deny">{firewallMode === 'ufw' ? 'DENY (Blokuj)' : 'DROP (Odrzuć)'}</option>
            {#if firewallMode === 'iptables'}
              <option value="reject">REJECT (Odrzuć z info)</option>
            {/if}
          </select>
        </div>

        <div class="form-group">
          <label for="rule-port">Port lub porty (np. 80, 443){firewallMode === 'iptables' ? ' - opcjonalnie' : ''}</label>
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
          <button class="primary" onclick={addRule} disabled={firewallMode === 'ufw' && !rulePort}>Dodaj regułę</button>
          <button class="secondary" onclick={closeAddModal}>Anuluj</button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .firewall-manager {
    display: flex;
    flex-direction: column;
    height: 100%;
    gap: 16px;
  }

  .manager-header {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .header-content {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .mode-selector {
    display: flex;
    background: var(--bg-tertiary);
    border-radius: var(--radius-md);
    padding: 4px;
  }

  .mode-btn {
    background: transparent;
    border: none;
    padding: 6px 16px;
    border-radius: 4px;
    color: var(--text-secondary);
    font-size: 0.9rem;
    font-weight: 500;
    cursor: pointer;
    transition: var(--transition-fast);
  }

  .mode-btn:hover {
    color: var(--text-primary);
  }

  .mode-btn.active {
    background: var(--bg-secondary);
    color: var(--text-primary);
    box-shadow: 0 2px 5px rgba(0,0,0,0.2);
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
    padding: 8px 12px;
    border-radius: var(--radius-md);
    flex-shrink: 0;
  }

  .iptables-header {
    background: rgba(40, 44, 52, 0.5);
  }

  .iptables-chain-selector {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .form-select {
    background: var(--bg-tertiary);
    border: 1px solid var(--border-color);
    color: var(--text-primary);
    padding: 6px 12px;
    border-radius: var(--radius-sm);
    outline: none;
    font-size: 0.9rem;
  }

  .policy-bar {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 8px 12px;
    background: rgba(255, 255, 255, 0.02);
    border-radius: var(--radius-sm);
    border-left: 3px solid var(--accent-blue);
    font-size: 0.9rem;
    color: var(--text-secondary);
  }

  .policy-actions {
    display: flex;
    gap: 8px;
  }

  .btn-sm {
    padding: 4px 10px;
    font-size: 0.75rem;
    border-radius: 4px;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid transparent;
    transition: var(--transition-fast);
  }

  .active-policy.success {
    background: var(--accent-green-glow);
    color: var(--accent-green);
    border-color: rgba(34, 197, 94, 0.3);
  }

  .active-policy.danger {
    background: var(--accent-red-glow);
    color: var(--accent-red);
    border-color: rgba(244, 63, 94, 0.3);
  }

  .status-indicator {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .shield-icon {
    flex-shrink: 0;
  }

  .shield-icon.active {
    color: var(--accent-green);
  }

  .shield-icon.inactive {
    color: var(--text-muted);
  }

  .status-title {
    font-size: 0.85rem;
    font-weight: 600;
    color: white;
  }

  .status-actions {
    display: flex;
    gap: 8px;
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

  .iptables-table th, .iptables-table td {
    padding: 12px 16px;
    font-size: 0.9rem;
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

  .small-text {
    font-size: 0.8rem;
    color: var(--text-secondary);
  }

  .badge.neutral {
    background: var(--bg-tertiary);
    color: var(--text-secondary);
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

  input, select {
    background: var(--bg-tertiary);
    border: 1px solid var(--border-color);
    padding: 8px 12px;
    border-radius: 4px;
    color: var(--text-primary);
    outline: none;
  }

  select option {
    background-color: #0d0e12;
    color: var(--text-primary);
  }

  input:focus, select:focus {
    border-color: var(--accent-blue);
  }
</style>
