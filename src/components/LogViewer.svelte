<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { FileText, Users, LogOut, RefreshCw, KeyRound, Play, Pause, Search } from 'lucide-svelte';
  import { stickToBottom } from '$lib/stickToBottom';
  import SortableTh from './ui/SortableTh.svelte';
  import { applySort, nextSort, type SortState } from '$lib/sort/sortUtils';
  import { get } from 'svelte/store';
  import { LL } from '$lib/i18n/i18n-svelte';
  import { notifications } from '$lib/notifications.svelte';
  import {
    formatInvokeError,
    isSudoPasswordRequired,
  } from '$lib/i18n/backendErrors';

  let activeSubTab = $state('logs'); // 'logs' | 'sessions'
  let isLoading = $state(false);
  let errorMsg = $state('');

  $effect(() => {
    if (errorMsg) {
      notifications.error(errorMsg);
      errorMsg = '';
    }
  });

  // Sudo auth
  let showSudoModal = $state(false);
  let sudoPassword = $state('');
  let pendingAction: (() => Promise<void>) | null = null;
  let sudoError = $state('');

  // --- LOGS TAB ---
  let selectedLogFile = $state('syslog');
  let logContent = $state('');
  let logSearchQuery = $state('');
  let isStreaming = $state(true);
  let logIntervalId: any;

  const logSources = $derived.by(() => {
    const ll = get(LL);
    return [
      { id: 'syslog', label: ll.logs.sourceSyslog(), cmd: 'tail -n 150 /var/log/syslog' },
      { id: 'auth', label: ll.logs.sourceAuth(), cmd: 'tail -n 150 /var/log/auth.log' },
      { id: 'nginx_access', label: ll.logs.sourceNginxAccess(), cmd: 'tail -n 150 /var/log/nginx/access.log' },
      { id: 'nginx_error', label: ll.logs.sourceNginxError(), cmd: 'tail -n 150 /var/log/nginx/error.log' },
      { id: 'journal', label: ll.logs.sourceJournal(), cmd: 'journalctl -n 150 --no-pager' },
    ];
  });

  async function loadLogs() {
    if (!isStreaming && logContent !== '') return;
    errorMsg = '';
    const source = logSources.find(s => s.id === selectedLogFile) || logSources[0];
    try {
      const output: string = await invoke('exec_custom_command', {
        cmd: source.cmd,
        useSudo: selectedLogFile === 'auth' // auth.log wymaga roota
      });
      logContent = output;
    } catch (err: unknown) {
      if (isSudoPasswordRequired(err)) {
        pendingAction = loadLogs;
        showSudoModal = true;
        isStreaming = false;
      } else {
        logContent = get(LL).logs.readFailed({ error: formatInvokeError(err) });
      }
    }
  }

  function getFilteredLogs() {
    if (!logSearchQuery) return logContent;
    const q = logSearchQuery.toLowerCase();
    return logContent
      .split('\n')
      .filter(line => line.toLowerCase().includes(q))
      .join('\n');
  }

  // --- SESSIONS TAB ---
  let activeSessions = $state<any[]>([]);
  let loginHistory = $state<any[]>([]);

  type SessionSortCol = 'username' | 'tty' | 'ip' | 'date';
  type HistorySortCol = 'username' | 'tty' | 'ip' | 'time';
  let sessionSort = $state<SortState<SessionSortCol>>({ column: 'username', direction: 'asc' });
  let historySort = $state<SortState<HistorySortCol>>({ column: 'time', direction: 'desc' });

  const sortedSessions = $derived(
    applySort(activeSessions, sessionSort, {
      username: (s) => s.username || '',
      tty: (s) => s.tty || '',
      ip: (s) => s.ip || '',
      date: (s) => s.date || '',
    }),
  );

  const sortedHistory = $derived(
    applySort(loginHistory, historySort, {
      username: (h) => h.username || '',
      tty: (h) => h.tty || '',
      ip: (h) => h.ip || '',
      time: (h) => h.time || '',
    }),
  );

  function setSessionSort(column: string) {
    sessionSort = nextSort(sessionSort, column as SessionSortCol);
  }

  function setHistorySort(column: string) {
    historySort = nextSort(historySort, column as HistorySortCol);
  }

  async function loadSessionsAndHistory() {
    isLoading = true;
    errorMsg = '';
    try {
      // 1. Aktywne sesje (who)
      const whoOut: string = await invoke('exec_custom_command', {
        cmd: 'who',
        useSudo: false
      });
      
      // Parsowanie wierszy: user tty date (ip)
      const sessions = whoOut.trim().split('\n').map(line => {
        const parts = line.trim().split(/\s+/);
        if (parts.length < 2) return null;
        return {
          username: parts[0],
          tty: parts[1],
          date: parts.slice(2, 4).join(' '),
          ip: parts[4] ? parts[4].replace(/[()]/g, '') : get(LL).common.local()
        };
      }).filter(Boolean);

      // 2. Historia logowania (last -n 15)
      const lastOut: string = await invoke('exec_custom_command', {
        cmd: 'last -n 15 -F',
        useSudo: false
      });
      
      const history = lastOut.trim().split('\n')
        .filter(line => line.trim() !== '' && !line.startsWith('wtmp') && !line.startsWith('reboot'))
        .map(line => {
          const parts = line.trim().split(/\s+/);
          if (parts.length < 3) return null;
          // user tty ip date_start - date_end (duration)
          const username = parts[0];
          const tty = parts[1];
          let ip = parts[2];
          let dateIndex = 3;
          
          // If IP is not an IP but a date
          if (ip.includes(':') || ip.length > 15 || ip === 'gone' || ip === 'still') {
            ip = get(LL).common.local();
            dateIndex = 2;
          }
          
          const timeDetails = parts.slice(dateIndex).join(' ');
          
          return {
            username,
            tty,
            ip,
            time: timeDetails
          };
        }).filter(Boolean);

      activeSessions = sessions;
      loginHistory = history;
    } catch (err: unknown) {
      errorMsg = get(LL).logs.sessionsLoadFailed({ error: formatInvokeError(err) });
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
      if (pendingAction) {
        const action = pendingAction;
        pendingAction = null;
        await action();
      }
    } catch (err: unknown) {
      sudoError = formatInvokeError(err);
    }
  }

  async function disconnectSession(tty: string) {
    if (confirm(get(LL).logs.confirmDisconnect({ tty }))) {
      const action = async () => {
        isLoading = true;
        errorMsg = '';
        await invoke('secure_kill_session', { tty });
        await loadSessionsAndHistory();
      };
      await handleActionWithSudo(action);
    }
  }

  function toggleStream() {
    isStreaming = !isStreaming;
    if (isStreaming) {
      loadLogs();
    }
  }

  // Set up log polling
  $effect(() => {
    if (activeSubTab === 'logs') {
      loadLogs();
      if (logIntervalId) clearInterval(logIntervalId);
      logIntervalId = setInterval(loadLogs, 3000);
    } else {
      if (logIntervalId) clearInterval(logIntervalId);
      loadSessionsAndHistory();
    }
  });

  onDestroy(() => {
    if (logIntervalId) clearInterval(logIntervalId);
  });
</script>

<div class="log-viewer manager-shell fade-in">
  <header class="manager-header">
    <h1 class="page-title">{$LL.logs.title()}</h1>
  </header>

  <!-- Sub-tabs navigation bar -->
  <div class="tabs-bar glass">
    <button class="tab-btn {activeSubTab === 'logs' ? 'active' : ''}" onclick={() => activeSubTab = 'logs'}>
      <FileText size={16} /> {$LL.logs.tabLogs()}
    </button>
    <button class="tab-btn {activeSubTab === 'sessions' ? 'active' : ''}" onclick={() => activeSubTab = 'sessions'}>
      <Users size={16} /> {$LL.logs.tabSessions()}
    </button>
  </div>

  {#if activeSubTab === 'logs'}
    <!-- LOGS SECTION -->
    <div class="log-controls-bar glass">
      <div class="selector-group">
        <label for="log-select">{$LL.logs.selectLog()}</label>
        <select id="log-select" bind:value={selectedLogFile} onchange={() => { logContent = ''; loadLogs(); }}>
          {#each logSources as source}
            <option value={source.id}>{source.label}</option>
          {/each}
        </select>
      </div>

      <div class="search-bar">
        <span class="search-icon-wrapper"><Search size={16} /></span>
        <input type="text" placeholder={$LL.logs.filterResults()} bind:value={logSearchQuery} />
      </div>

      <div class="stream-actions">
        <button class="secondary" onclick={toggleStream}>
          {#if isStreaming}
            <Pause size={16} /> {$LL.common.pause()}
          {:else}
            <Play size={16} /> {$LL.common.stream()}
          {/if}
        </button>
        <button class="secondary" onclick={loadLogs}>
          <RefreshCw size={16} /> {$LL.common.refresh()}
        </button>
      </div>
    </div>

    <!-- Log Preview -->
    <div class="log-display-container glass" use:stickToBottom>
      <pre class="log-text"><code>{getFilteredLogs() || $LL.logs.loadingContent()}</code></pre>
    </div>
  {:else}
    <!-- SESSIONS SECTION -->
    <div class="sessions-split">
      <!-- Aktywne Sesje -->
      <div class="sessions-table-card glass">
        <div class="card-header">
          <h3>{$LL.logs.currentlyLoggedIn()}</h3>
          <button class="secondary btn-sm" onclick={loadSessionsAndHistory} disabled={isLoading}>
            <RefreshCw size={14} class={isLoading ? 'spin' : ''} />
          </button>
        </div>
        
        <table class="sessions-table">
          <thead>
            <tr>
              <SortableTh label={$LL.logs.user()} column="username" activeColumn={sessionSort.column} direction={sessionSort.direction} onsort={setSessionSort} />
              <SortableTh label={$LL.logs.terminal()} column="tty" activeColumn={sessionSort.column} direction={sessionSort.direction} onsort={setSessionSort} />
              <SortableTh label={$LL.logs.ipAddress()} column="ip" activeColumn={sessionSort.column} direction={sessionSort.direction} onsort={setSessionSort} />
              <SortableTh label={$LL.logs.loginDate()} column="date" activeColumn={sessionSort.column} direction={sessionSort.direction} onsort={setSessionSort} />
              <th style="text-align: right; padding: 14px 16px; font-size: 0.8rem; text-transform: uppercase; color: var(--text-muted); font-weight: 600;">{$LL.logs.kick()}</th>
            </tr>
          </thead>
          <tbody>
            {#each sortedSessions as session}
              <tr>
                <td class="mono-val"><strong>{session.username}</strong></td>
                <td><span class="badge warning mono-val">{session.tty}</span></td>
                <td class="mono-val"><code>{session.ip}</code></td>
                <td class="date-cell mono-val">{session.date}</td>
                <td class="actions-cell">
                  {#if session.tty !== 'tty1' && session.username !== 'slawek'}
                    <button class="btn-table danger-text" onclick={() => disconnectSession(session.tty)} title={$LL.logs.disconnectSession()}>
                      <LogOut size={14} />
                    </button>
                  {/if}
                </td>
              </tr>
            {/each}

            {#if sortedSessions.length === 0}
              <tr>
                <td colspan="5" class="empty-state">{$LL.logs.noActiveSessions()}</td>
              </tr>
            {/if}
          </tbody>
        </table>
      </div>

      <!-- Ostatnie Logowania -->
      <div class="history-table-card glass">
        <div class="card-header">
          <h3>{$LL.logs.loginHistory()}</h3>
        </div>
        
        <table class="history-table">
          <thead>
            <tr>
              <SortableTh label={$LL.logs.user()} column="username" activeColumn={historySort.column} direction={historySort.direction} onsort={setHistorySort} />
              <SortableTh label={$LL.logs.terminal()} column="tty" activeColumn={historySort.column} direction={historySort.direction} onsort={setHistorySort} />
              <SortableTh label={$LL.logs.loginIp()} column="ip" activeColumn={historySort.column} direction={historySort.direction} onsort={setHistorySort} />
              <SortableTh label={$LL.logs.timeDetails()} column="time" activeColumn={historySort.column} direction={historySort.direction} onsort={setHistorySort} />
            </tr>
          </thead>
          <tbody>
            {#each sortedHistory as hist}
              <tr>
                <td class="mono-val"><strong>{hist.username}</strong></td>
                <td><span class="badge warning mono-val">{hist.tty}</span></td>
                <td class="mono-val"><code>{hist.ip}</code></td>
                <td class="time-cell mono-val">{hist.time}</td>
              </tr>
            {/each}
            
            {#if sortedHistory.length === 0}
              <tr>
                <td colspan="4" class="empty-state">{$LL.logs.noLoginHistory()}</td>
              </tr>
            {/if}
          </tbody>
        </table>
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
          <button class="secondary" onclick={() => { showSudoModal = false; sudoPassword = ''; pendingAction = null; if (activeSubTab === 'logs') isStreaming = true; }}>{$LL.common.cancel()}</button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .log-viewer {
    /* uses .manager-shell */
  }

  /* Tabs Bar */
  .tabs-bar {
    display: flex;
    padding: 3px;
    border-radius: var(--radius-md);
    gap: 3px;
    flex-shrink: 0;
  }

  .tab-btn {
    flex: 1;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    padding: 6px 8px;
    cursor: pointer;
    font-size: 0.78rem;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    transition: var(--transition-fast);
  }

  .tab-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .tab-btn.active {
    background: var(--bg-active);
    color: var(--accent-amber);
    border: 1px solid rgba(245, 158, 11, 0.2);
    font-weight: 600;
  }

  /* Log Controls */
  .log-controls-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-radius: var(--radius-md);
    flex-shrink: 0;
  }

  .selector-group {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .selector-group label {
    font-size: 0.85rem;
    color: var(--text-secondary);
  }

  .search-bar {
    position: relative;
    flex: 1;
  }

  .search-bar input {
    width: 100%;
    padding-left: 36px;
  }

  .search-icon-wrapper {
    position: absolute;
    left: 10px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--text-muted);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    pointer-events: none;
  }

  .stream-actions {
    display: flex;
    gap: 8px;
  }

  /* Log Display */
  .log-display-container {
    flex: 1;
    overflow: auto;
    border-radius: var(--radius-sm);
    background: var(--bg-primary);
    padding: 16px;
    border: 1px solid var(--border-color);
  }

  .log-text {
    font-family: var(--font-mono);
    font-size: 0.8rem;
    line-height: 1.5;
    color: #d1d4db;
    white-space: pre-wrap;
    user-select: text;
  }

  /* Sessions Split */
  .sessions-split {
    display: grid;
    grid-template-columns: 1.2fr 1fr;
    gap: 24px;
    flex: 1;
    overflow: hidden;
  }

  .sessions-table-card, .history-table-card {
    display: flex;
    flex-direction: column;
    padding: 24px;
    border-radius: var(--radius-md);
    overflow: hidden;
  }

  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid var(--border-color);
    padding-bottom: 12px;
    margin-bottom: 16px;
    flex-shrink: 0;
  }

  .card-header h3 {
    font-size: 1.1rem;
    color: white;
  }

  .sessions-table, .history-table {
    width: 100%;
    border-collapse: collapse;
    text-align: left;
  }

  .sessions-table th, .sessions-table td,
  .history-table th, .history-table td {
    padding: 12px 16px;
    border-bottom: 1px solid var(--border-color);
  }

  .sessions-table th, .history-table th {
    font-size: 0.75rem;
    text-transform: uppercase;
    color: var(--text-muted);
    font-weight: 600;
    letter-spacing: 0.05em;
    position: sticky;
    top: 0;
    background: var(--bg-secondary);
    z-index: 1;
  }

  .sessions-table tr, .history-table tr {
    transition: var(--transition-fast);
  }

  .sessions-table tr:hover, .history-table tr:hover {
    background: rgba(255, 255, 255, 0.02);
  }

  .date-cell, .time-cell {
    font-size: 0.85rem;
    color: var(--text-secondary);
  }

  .actions-cell {
    text-align: right;
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

  .empty-state {
    text-align: center;
    color: var(--text-muted);
    font-size: 0.9rem;
    padding: 40px !important;
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
</style>
