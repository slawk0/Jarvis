<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { Shield, ShieldOff, Plus, Trash2, KeyRound, Check, ShieldAlert } from 'lucide-svelte';
  import SortableTh from './ui/SortableTh.svelte';
  import { applySort, nextSort, type SortState } from '$lib/sort/sortUtils';
  import { get } from 'svelte/store';
  import { LL } from '$lib/i18n/i18n-svelte';
  import { notifications } from '$lib/notifications.svelte';
  import {
    formatInvokeError,
    isSudoPasswordRequired,
  } from '$lib/i18n/backendErrors';

  let { visible = true } = $props();

  let firewallMode = $state<'ufw' | 'iptables'>('ufw');

  // UFW state
  let ufwActive = $state(false);
  let rules = $state<any[]>([]);
  
  // iptables state
  let iptablesChains = $state<{name: string, policy: string, rules: any[]}[]>([]);
  let activeChain = $state('INPUT');
  let iptablesTable = $state<'filter' | 'nat' | 'mangle' | 'raw'>('filter');
  let iptablesRawOutput = $state('');
  let showRawModal = $state(false);

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

  $effect(() => {
    if (errorMsg) {
      notifications.error(errorMsg);
      errorMsg = '';
    }
  });

  // Sudo auth state
  let showSudoModal = $state(false);
  let sudoPassword = $state('');
  let pendingAction: (() => Promise<void>) | null = null;
  let sudoError = $state('');
  let isSudoAuthorized = $state(false);
  let isInitialized = $state(false);

  async function checkSudo() {
    try {
      isSudoAuthorized = await invoke<boolean>('has_sudo_password');
    } catch {
      isSudoAuthorized = false;
    }
    isInitialized = true;
    if (isSudoAuthorized) {
      if (firewallMode === 'ufw') loadUfwStatus();
      else loadIptablesStatus();
    }
  }

  function requestSudoAuth() {
    pendingAction = async () => {
      isSudoAuthorized = true;
      if (firewallMode === 'ufw') await loadUfwStatus();
      else await loadIptablesStatus();
    };
    showSudoModal = true;
  }

  function formatProtocol(prot: string): string {
    const p = String(prot).toLowerCase().trim();
    if (p === '6' || p === 'tcp') return 'TCP';
    if (p === '17' || p === 'udp') return 'UDP';
    if (p === '1' || p === 'icmp') return 'ICMP';
    if (p === '0' || p === 'all') return 'all';
    if (p === '47') return 'GRE';
    if (p === '50') return 'ESP';
    if (p === '51') return 'AH';
    if (p === '89') return 'OSPF';
    return prot;
  }

  // New rule form
  let showAddModal = $state(false);
  let ruleAction = $state('allow');
  let rulePort = $state('');
  let ruleProto = $state('any');
  let ruleSource = $state('Anywhere');
  let iptablesPosition = $state<'append' | 'first' | 'custom'>('append');
  let iptablesInsertIndex = $state(1);
  let previousActiveChain = $state('');
  let ruleTable = $state<'filter' | 'nat' | 'mangle' | 'raw'>('filter');
  let ruleChain = $state('INPUT');

  function getChainsForTable(table: string): string[] {
    if (table === iptablesTable) {
      return iptablesChains.map(c => c.name);
    }
    if (table === 'filter') return ['INPUT', 'FORWARD', 'OUTPUT'];
    if (table === 'nat') return ['PREROUTING', 'INPUT', 'OUTPUT', 'POSTROUTING'];
    if (table === 'mangle') return ['PREROUTING', 'INPUT', 'FORWARD', 'OUTPUT', 'POSTROUTING'];
    if (table === 'raw') return ['PREROUTING', 'OUTPUT'];
    return ['INPUT', 'FORWARD', 'OUTPUT'];
  }

  function openAddModal(chainName?: string) {
    ruleTable = iptablesTable;
    if (chainName) {
      ruleChain = chainName;
    } else {
      ruleChain = activeChain === 'ALL' ? (getChainsForTable(ruleTable)[0] || 'INPUT') : activeChain;
    }
    showAddModal = true;
  }

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
    } catch (err: unknown) {
      const errStr = String(err).toLowerCase();
      if (isSudoPasswordRequired(err)) {
        pendingAction = loadUfwStatus;
        showSudoModal = true;
      } else if (errStr.includes('not found') || errStr.includes('nie znaleziono')) {
        ufwActive = false;
        rules = [];
        if (firewallMode === 'ufw') {
          errorMsg = get(LL).firewall.notInstalled();
        }
      } else {
        errorMsg = get(LL).firewall.loadUfwFailed({ error: formatInvokeError(err) });
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
        cmd: `iptables -t ${iptablesTable} -L -n --line-numbers`,
        useSudo: true
      });
      iptablesRawOutput = out;
      
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
      if (activeChain !== 'ALL' && !chains.find(c => c.name === activeChain) && chains.length > 0) {
        activeChain = chains[0].name;
      }
    } catch (err: unknown) {
      if (isSudoPasswordRequired(err)) {
        pendingAction = loadIptablesStatus;
        showSudoModal = true;
      } else {
        errorMsg = get(LL).firewall.loadIptablesFailed({ error: formatInvokeError(err) });
      }
    } finally {
      isLoading = false;
    }
  }

  async function handleActionWithSudo(action: () => Promise<void>) {
    const run = async () => {
      try {
        await action();
      } catch (err: unknown) {
        if (isSudoPasswordRequired(err)) {
          pendingAction = run;
          showSudoModal = true;
        } else {
          errorMsg = get(LL).common.actionFailed({ error: formatInvokeError(err) });
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
      isSudoAuthorized = true;
      if (pendingAction) {
        const action = pendingAction;
        pendingAction = null;
        await action();
      }
    } catch (err: unknown) {
      sudoError = formatInvokeError(err);
    }
  }

  async function toggleUfw() {
    const action = async () => {
      isLoading = true;
      errorMsg = '';
      await invoke('secure_ufw_toggle', { enable: !ufwActive });
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
        await invoke('secure_ufw_rule', {
          action: ruleAction,
          port: rulePort,
          proto: ruleProto !== 'any' ? ruleProto : null,
          source: (ruleSource !== 'Anywhere' && ruleSource !== '') ? ruleSource : null,
        });
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

        let cmd = '';
        if (iptablesPosition === 'first') {
          cmd = `iptables -t ${ruleTable} -I ${ruleChain} 1`;
        } else if (iptablesPosition === 'custom') {
          cmd = `iptables -t ${ruleTable} -I ${ruleChain} ${iptablesInsertIndex}`;
        } else {
          cmd = `iptables -t ${ruleTable} -A ${ruleChain}`;
        }

        if (prot !== 'any') {
          cmd += ` -p ${prot}`;
          if (rulePort && rulePort !== 'any') cmd += ` --dport ${rulePort}`;
        }
        if (ruleSource && ruleSource !== 'Anywhere' && ruleSource !== '0.0.0.0/0') {
          cmd += ` -s ${ruleSource}`;
        }
        cmd += ` -j ${target}`;

        await invoke('exec_custom_command', { cmd, useSudo: true });
        
        // Auto-switch view to the table/chain where the rule was added
        iptablesTable = ruleTable;
        if (activeChain !== 'ALL') {
          activeChain = ruleChain;
        }

        closeAddModal();
        await loadIptablesStatus();
      };
      await handleActionWithSudo(action);
    }
  }

  function closeAddModal() {
    showAddModal = false;
    rulePort = '';
    ruleSource = firewallMode === 'ufw' ? 'Anywhere' : '0.0.0.0/0';
    ruleProto = 'any';
    iptablesPosition = 'append';
    iptablesInsertIndex = 1;
    if (previousActiveChain) {
      activeChain = previousActiveChain;
      previousActiveChain = '';
    }
  }

  async function deleteRule(num: number, chainName?: string) {
    if (firewallMode === 'ufw') {
      if (confirm(get(LL).firewall.confirmDeleteUfwRule({ num }))) {
        const action = async () => {
          isLoading = true;
          errorMsg = '';
          await invoke('secure_ufw_delete_rule', { ruleNum: num });
          await loadUfwStatus();
        };
        await handleActionWithSudo(action);
      }
    } else {
      const chain = chainName || activeChain;
      if (confirm(get(LL).firewall.confirmDeleteIptablesRule({ num, chain }))) {
        const action = async () => {
          isLoading = true;
          errorMsg = '';
          await invoke('exec_custom_command', {
            cmd: `iptables -t ${iptablesTable} -D ${chain} ${num}`,
            useSudo: true
          });
          await loadIptablesStatus();
        };
        await handleActionWithSudo(action);
      }
    }
  }

  async function setIptablesPolicy(policy: string, chainName?: string) {
    const chain = chainName || activeChain;
    if (confirm(get(LL).firewall.confirmChangePolicy({ chain, policy }))) {
      const action = async () => {
        isLoading = true;
        errorMsg = '';
        await invoke('exec_custom_command', {
          cmd: `iptables -t ${iptablesTable} -P ${chain} ${policy}`,
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
      ruleSource = 'Anywhere';
      loadUfwStatus();
    } else {
      ruleSource = '0.0.0.0/0';
      loadIptablesStatus();
    }
  }

  export function refresh() {
    if (isSudoAuthorized) {
      if (firewallMode === 'ufw') loadUfwStatus();
      else loadIptablesStatus();
    } else {
      checkSudo();
    }
  }

  onMount(() => {
    checkSudo();
  });

  let activeChainData = $derived(iptablesChains.find(c => c.name === activeChain));
</script>

<div class="firewall-manager manager-shell fade-in">
  <header class="manager-header">
    <div class="header-title-section">
      <h1 class="page-title">{$LL.firewall.title()}</h1>
      {#if isSudoAuthorized}
        <div class="mode-selector">
          <button class="mode-btn {firewallMode === 'ufw' ? 'active' : ''}" onclick={() => switchMode('ufw')}>UFW</button>
          <button class="mode-btn {firewallMode === 'iptables' ? 'active' : ''}" onclick={() => switchMode('iptables')}>iptables</button>
        </div>
      {/if}
    </div>
  </header>

  {#if !isSudoAuthorized && isInitialized}
    <div class="auth-gate fade-in">
      <div class="auth-gate-card glass">
        <div class="auth-gate-icon">
          <KeyRound size={40} class="accent-amber-text" />
        </div>
        <h2>{$LL.sudo.authTitle()}</h2>
        <p>
          {$LL.sudo.authDesc()}
        </p>
        <button class="primary" onclick={requestSudoAuth}>
          <KeyRound size={16} /> {$LL.sudo.passwordInputPlaceholder()}
        </button>
      </div>
    </div>
  {:else if isInitialized}
  {#if firewallMode === 'ufw'}
    <!-- Pasek stanu zapory UFW -->
    <div class="status-bar glass">
      <div class="status-indicator">
        {#if ufwActive}
          <Shield size={16} class="shield-icon active" />
          <span class="status-title">{$LL.firewall.ufwActive()}</span>
        {:else}
          <ShieldOff size={16} class="shield-icon inactive" />
          <span class="status-title">{$LL.firewall.ufwInactive()}</span>
        {/if}
      </div>
      <div class="status-actions">
        <button class={ufwActive ? 'danger' : 'primary'} onclick={toggleUfw} disabled={isLoading}>
          {ufwActive ? $LL.firewall.disableUfw() : $LL.firewall.enableUfw()}
        </button>
      </div>
    </div>

    {#if ufwActive}
      <!-- Operational section for UFW rules -->
      <div class="rules-header">
        <h2>{$LL.firewall.activeRules()}</h2>
        <button class="primary" onclick={() => showAddModal = true}>
          <Plus size={16} /> {$LL.firewall.addRule()}
        </button>
      </div>

      <div class="table-container glass">
        <table class="rules-table">
          <thead>
            <tr>
              <SortableTh label={$LL.firewall.num()} column="num" activeColumn={ruleSort.column} direction={ruleSort.direction} onsort={setRuleSort} width="10%" />
              <SortableTh label={$LL.firewall.portService()} column="to" activeColumn={ruleSort.column} direction={ruleSort.direction} onsort={setRuleSort} width="25%" />
              <SortableTh label={$LL.firewall.action()} column="action" activeColumn={ruleSort.column} direction={ruleSort.direction} onsort={setRuleSort} width="20%" />
              <SortableTh label={$LL.firewall.fromIp()} column="from" activeColumn={ruleSort.column} direction={ruleSort.direction} onsort={setRuleSort} width="25%" />
              <th style="width: 20%; text-align: right;">{$LL.common.delete()}</th>
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
                  <button class="btn-table danger-text" onclick={() => deleteRule(rule.num)} title={$LL.firewall.deleteRule()}>
                    <Trash2 size={14} />
                  </button>
                </td>
              </tr>
            {/each}

            {#if sortedRules.length === 0}
              <tr>
                <td colspan="5" class="empty-state">{$LL.firewall.emptyUfw()}</td>
              </tr>
            {/if}
          </tbody>
        </table>
      </div>
    {/if}

  {:else}
    <!-- IPTABLES MODE -->
    <div class="status-bar glass iptables-header">
      <div style="display: flex; gap: 16px; flex-wrap: wrap;">
        <div class="iptables-chain-selector">
          <label for="table-select" class="status-title">{$LL.firewall.selectTable()}</label>
          <select id="table-select" class="form-select" bind:value={iptablesTable} onchange={loadIptablesStatus}>
            <option value="filter">filter</option>
            <option value="nat">nat</option>
            <option value="mangle">mangle</option>
            <option value="raw">raw</option>
          </select>
        </div>

        <div class="iptables-chain-selector">
          <label for="chain-select" class="status-title">{$LL.firewall.selectChain()}</label>
          <select id="chain-select" class="form-select" bind:value={activeChain}>
            <option value="ALL">{$LL.firewall.allChains()}</option>
            {#each iptablesChains as chain}
              <option value={chain.name}>{$LL.firewall.chainPolicy({ name: chain.name, policy: chain.policy })}</option>
            {/each}
          </select>
        </div>
      </div>
 
      <div class="status-actions">
        <button class="secondary" onclick={() => showRawModal = true}>
          {$LL.firewall.showRawOutput()}
        </button>
      </div>
    </div>
 
    <div class="rules-header">
      <h2>
        {#if activeChain === 'ALL'}
          {$LL.firewall.iptablesRules({ chain: $LL.firewall.allChains() })}
        {:else}
          {$LL.firewall.iptablesRules({ chain: activeChain })}
        {/if}
      </h2>
      <button class="primary" onclick={() => openAddModal()}>
        <Plus size={16} /> {$LL.firewall.addRule()}
      </button>
    </div>

    {#if activeChain === 'ALL'}
      <div class="all-chains-container">
        {#each iptablesChains as chain}
          <div class="chain-block glass">
            <div class="chain-block-header">
              <h3>{$LL.firewall.iptablesRules({ chain: chain.name })} {#if chain.policy !== '-'}(Polityka: <strong>{chain.policy}</strong>){/if}</h3>
              <div class="chain-block-actions">
                {#if chain.policy !== '-'}
                  <button class="btn-sm {chain.policy === 'ACCEPT' ? 'active-policy success' : 'secondary'}" onclick={() => setIptablesPolicy('ACCEPT', chain.name)}>ACCEPT</button>
                  <button class="btn-sm {chain.policy === 'DROP' ? 'active-policy danger' : 'secondary'}" onclick={() => setIptablesPolicy('DROP', chain.name)}>DROP</button>
                {/if}
              </div>
            </div>
 
            <div class="table-container">
              <table class="rules-table iptables-table">
                <thead>
                  <tr>
                    <th style="width: 5%">{$LL.firewall.num()}</th>
                    <th style="width: 15%">{$LL.firewall.target()}</th>
                    <th style="width: 10%">{$LL.firewall.protocol()}</th>
                    <th style="width: 20%">{$LL.firewall.source()}</th>
                    <th style="width: 20%">{$LL.firewall.destination()}</th>
                    <th style="width: 20%">{$LL.firewall.extra()}</th>
                    <th style="width: 10%; text-align: right;">{$LL.common.delete()}</th>
                  </tr>
                </thead>
                <tbody>
                  {#if chain.rules.length > 0}
                    {#each chain.rules as rule}
                      <tr>
                        <td><span class="badge warning mono-val">{rule.num}</span></td>
                        <td>
                          <span class="badge {rule.target === 'ACCEPT' ? 'success' : (rule.target === 'DROP' || rule.target === 'REJECT' ? 'danger' : 'neutral')}">
                            {rule.target}
                          </span>
                        </td>
                        <td class="mono-val">{formatProtocol(rule.prot)}</td>
                        <td class="mono-val"><code>{rule.source}</code></td>
                        <td class="mono-val"><code>{rule.destination}</code></td>
                        <td class="mono-val small-text">{rule.extra}</td>
                        <td class="actions-cell">
                          <button class="btn-table danger-text" onclick={() => deleteRule(parseInt(rule.num), chain.name)} title={$LL.firewall.deleteRule()}>
                            <Trash2 size={14} />
                          </button>
                        </td>
                      </tr>
                    {/each}
                  {:else}
                    <tr>
                      <td colspan="7" class="empty-state">{$LL.firewall.emptyChain()}</td>
                    </tr>
                  {/if}
                </tbody>
              </table>
            </div>
          </div>
        {/each}
      </div>
    {:else}
      {#if activeChainData && activeChainData.policy !== '-'}
        <div class="policy-bar">
          <span>{$LL.firewall.defaultPolicy()} <strong>{activeChainData.policy}</strong></span>
          <div class="policy-actions">
            <button class="btn-sm {activeChainData.policy === 'ACCEPT' ? 'active-policy success' : 'secondary'}" onclick={() => setIptablesPolicy('ACCEPT')}>ACCEPT</button>
            <button class="btn-sm {activeChainData.policy === 'DROP' ? 'active-policy danger' : 'secondary'}" onclick={() => setIptablesPolicy('DROP')}>DROP</button>
          </div>
        </div>
      {/if}
 
      <div class="table-container glass">
        <table class="rules-table iptables-table">
          <thead>
            <tr>
              <th style="width: 5%">{$LL.firewall.num()}</th>
              <th style="width: 15%">{$LL.firewall.target()}</th>
              <th style="width: 10%">{$LL.firewall.protocol()}</th>
              <th style="width: 20%">{$LL.firewall.source()}</th>
              <th style="width: 20%">{$LL.firewall.destination()}</th>
              <th style="width: 20%">{$LL.firewall.extra()}</th>
              <th style="width: 10%; text-align: right;">{$LL.common.delete()}</th>
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
                  <td class="mono-val">{formatProtocol(rule.prot)}</td>
                  <td class="mono-val"><code>{rule.source}</code></td>
                  <td class="mono-val"><code>{rule.destination}</code></td>
                  <td class="mono-val small-text">{rule.extra}</td>
                  <td class="actions-cell">
                    <button class="btn-table danger-text" onclick={() => deleteRule(parseInt(rule.num))} title={$LL.firewall.deleteRule()}>
                      <Trash2 size={14} />
                    </button>
                  </td>
                </tr>
              {/each}
            {:else}
              <tr>
                <td colspan="7" class="empty-state">{$LL.firewall.emptyChain()}</td>
              </tr>
            {/if}
          </tbody>
        </table>
      </div>
    {/if}
  {/if}
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

  <!-- New Rule Creator -->
  {#if showAddModal}
    <div class="modal-overlay">
      <div class="modal-content glass">
        <h3>{$LL.firewall.addRuleTitle({ mode: firewallMode === 'ufw' ? 'UFW' : 'iptables' })}</h3>
        
        <div class="form-group">
          <label for="rule-action">{$LL.firewall.ruleAction()}</label>
          <select id="rule-action" bind:value={ruleAction}>
            <option value="allow">{firewallMode === 'ufw' ? $LL.firewall.ufwAllow() : $LL.firewall.iptablesAccept()}</option>
            <option value="deny">{firewallMode === 'ufw' ? $LL.firewall.ufwDeny() : $LL.firewall.iptablesDrop()}</option>
            {#if firewallMode === 'iptables'}
              <option value="reject">{$LL.firewall.iptablesReject()}</option>
            {/if}
          </select>
        </div>

        {#if firewallMode === 'iptables'}
          <div class="form-group">
            <label for="rule-table">{$LL.firewall.selectTable()}</label>
            <select id="rule-table" bind:value={ruleTable} onchange={() => { ruleChain = getChainsForTable(ruleTable)[0] || 'INPUT'; }}>
              <option value="filter">filter</option>
              <option value="nat">nat</option>
              <option value="mangle">mangle</option>
              <option value="raw">raw</option>
            </select>
          </div>

          <div class="form-group">
            <label for="rule-chain">{$LL.firewall.ruleChain()}</label>
            <select id="rule-chain" bind:value={ruleChain}>
              {#each getChainsForTable(ruleTable) as chainName}
                <option value={chainName}>{chainName}</option>
              {/each}
            </select>
          </div>
        {/if}

        <div class="form-group">
          <label for="rule-port">{$LL.firewall.rulePort()}{firewallMode === 'iptables' ? $LL.firewall.rulePortOptional() : ''}</label>
          <input id="rule-port" type="text" placeholder={$LL.firewall.rulePortPlaceholder()} bind:value={rulePort} />
        </div>

        <div class="form-group">
          <label for="rule-proto">{$LL.firewall.ruleProtocol()}</label>
          <select id="rule-proto" bind:value={ruleProto}>
            <option value="any">{$LL.firewall.protocolAny()}</option>
            <option value="tcp">TCP</option>
            <option value="udp">UDP</option>
          </select>
        </div>

        <div class="form-group">
          <label for="rule-source">{$LL.firewall.ruleSource()}</label>
          <input id="rule-source" type="text" placeholder={$LL.firewall.ruleSourcePlaceholder()} bind:value={ruleSource} />
        </div>

        {#if firewallMode === 'iptables'}
          <div class="form-group">
            <label for="iptables-position">{$LL.firewall.iptablesPriority()}</label>
            <select id="iptables-position" bind:value={iptablesPosition}>
              <option value="append">{$LL.firewall.iptablesPriorityAppend()}</option>
              <option value="first">{$LL.firewall.iptablesPriorityFirst()}</option>
              <option value="custom">{$LL.firewall.iptablesPriorityCustom()}</option>
            </select>
          </div>

          {#if iptablesPosition === 'custom'}
            <div class="form-group">
              <label for="iptables-insert-index">{$LL.firewall.iptablesLineNumber()}</label>
              <input id="iptables-insert-index" type="number" min="1" bind:value={iptablesInsertIndex} />
            </div>
          {/if}
        {/if}

        <div class="modal-actions">
          <button class="primary" onclick={addRule} disabled={firewallMode === 'ufw' && !rulePort}>{$LL.common.addRule()}</button>
          <button class="secondary" onclick={closeAddModal}>{$LL.common.cancel()}</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Raw Output Modal -->
  {#if showRawModal}
    <div class="modal-overlay">
      <div class="modal-content glass raw-modal">
        <h3>{$LL.firewall.rawOutputTitle({ command: `iptables -t ${iptablesTable} -L -n --line-numbers` })}</h3>
        <pre class="raw-output-pre"><code>{iptablesRawOutput}</code></pre>
        <div class="modal-actions">
          <button class="secondary" onclick={() => showRawModal = false}>{$LL.common.cancel()}</button>
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

  .header-title-section {
    display: flex;
    align-items: center;
    gap: 16px;
    flex-wrap: wrap;
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
    padding: 8px 12px;
    border-bottom: 1px solid var(--border-color);
  }

  .rules-table td {
    font-size: 0.9rem;
  }

  .rules-table th {
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

  .iptables-table th, .iptables-table td {
    padding: 8px 12px;
  }

  .iptables-table td {
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

  .auth-gate {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 420px;
    padding: 24px;
  }

  .auth-gate-card {
    max-width: 440px;
    padding: 36px 32px;
    border-radius: var(--radius-md);
    text-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
  }

  .auth-gate-icon {
    background: rgba(245, 158, 11, 0.08);
    border: 1px solid rgba(245, 158, 11, 0.2);
    border-radius: 50%;
    padding: 16px;
    margin-bottom: 4px;
  }

  .auth-gate-card h2 {
    font-size: 1.15rem;
    font-weight: 600;
    color: var(--text-primary);
  }

  .auth-gate-card p {
    font-size: 0.88rem;
    color: var(--text-secondary);
    line-height: 1.55;
    margin: 0;
  }

  .auth-gate-card button {
    margin-top: 8px;
    display: inline-flex;
    align-items: center;
    gap: 8px;
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

  .chain-block {
    margin-bottom: 24px;
    padding: 16px;
    border-radius: var(--radius-md);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .chain-block-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid var(--border-color);
    padding-bottom: 8px;
  }

  .chain-block-header h3 {
    font-size: 1.05rem;
    color: white;
    margin: 0;
  }

  .chain-block-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .raw-modal {
    width: 800px;
    max-width: 95vw;
  }

  .raw-output-pre {
    background: #090a0f;
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    padding: 12px;
    max-height: 550px;
    overflow: auto;
    font-family: var(--font-mono, monospace);
    font-size: 0.82rem;
    color: #e4e4e7;
    white-space: pre-wrap;
    text-align: left;
  }

  .all-chains-container {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 20px;
    padding-right: 4px;
  }
</style>
