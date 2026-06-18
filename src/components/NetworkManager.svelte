<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { RefreshCw, Network, Globe, ArrowRightLeft } from 'lucide-svelte';
  import SortableTh from './ui/SortableTh.svelte';
  import { applySort, nextSort, type SortState } from '$lib/sort/sortUtils';
  import { LL } from '$lib/i18n/i18n-svelte';
  import { formatInvokeError } from '$lib/i18n/backendErrors';
  import { notifications } from '$lib/notifications.svelte';

  let activeSubTab = $state<'listening' | 'connections'>('listening');
  let isLoading = $state(false);
  let errorMsg = $state('');

  $effect(() => {
    if (errorMsg) {
      notifications.error(errorMsg);
      errorMsg = '';
    }
  });
  let listening = $state<any[]>([]);
  let connections = $state<any[]>([]);

  type ListenSortCol = 'proto' | 'local' | 'process';
  type ConnSortCol = 'local' | 'remote' | 'state';
  let listenSort = $state<SortState<ListenSortCol>>({ column: 'local', direction: 'asc' });
  let connSort = $state<SortState<ConnSortCol>>({ column: 'local', direction: 'asc' });

  const sortedListening = $derived(
    applySort(listening, listenSort, {
      proto: (r) => r.proto || '',
      local: (r) => r.local || '',
      process: (r) => r.process || '',
    }),
  );

  const sortedConnections = $derived(
    applySort(connections, connSort, {
      local: (r) => r.local || '',
      remote: (r) => r.remote || '',
      state: (r) => r.state || '',
    }),
  );

  function parseListening(output: string) {
    const rows: any[] = [];
    for (const line of output.trim().split('\n')) {
      if (!line.trim() || line.startsWith('State') || line.startsWith('Netid')) continue;
      const parts = line.trim().split(/\s+/);
      if (parts.length < 5) continue;
      const proto = parts[0];
      const local = parts[3] || parts[1] || '';
      const process = parts.slice(5).join(' ') || parts[parts.length - 1] || '—';
      rows.push({ proto, local, process });
    }
    return rows;
  }

  function parseConnections(output: string) {
    const rows: any[] = [];
    for (const line of output.trim().split('\n')) {
      if (!line.trim() || line.startsWith('State') || line.startsWith('Recv-Q')) continue;
      const parts = line.trim().split(/\s+/);
      if (parts.length < 4) continue;
      rows.push({
        state: parts[0],
        local: parts[3] || '—',
        remote: parts[4] || '—',
      });
    }
    return rows;
  }

  async function loadData() {
    isLoading = true;
    errorMsg = '';
    try {
      const listenOut = await invoke<string>('exec_custom_command', {
        cmd: 'ss -tulpn 2>/dev/null || netstat -tulpn 2>/dev/null',
        useSudo: false,
      });
      listening = parseListening(listenOut);

      const connOut = await invoke<string>('exec_custom_command', {
        cmd: 'ss -tn state established 2>/dev/null | head -80',
        useSudo: false,
      });
      connections = parseConnections(connOut);
    } catch (err: unknown) {
      errorMsg = formatInvokeError(err);
    } finally {
      isLoading = false;
    }
  }

  onMount(loadData);
</script>

<div class="network manager-shell fade-in">
  <header class="manager-header">
    <h1 class="page-title">{$LL.network.title()}</h1>
    <button class="secondary btn-compact" disabled={isLoading} onclick={loadData}>
      <RefreshCw size={14} class={isLoading ? 'spin' : ''} /> {$LL.common.refresh()}
    </button>
  </header>

  <div class="sub-tabs">
    <button class="sub-tab {activeSubTab === 'listening' ? 'active' : ''}" onclick={() => (activeSubTab = 'listening')}>
      <Network size={14} /> {$LL.network.listening({ count: listening.length })}
    </button>
    <button class="sub-tab {activeSubTab === 'connections' ? 'active' : ''}" onclick={() => (activeSubTab = 'connections')}>
      <ArrowRightLeft size={14} /> {$LL.network.connections({ count: connections.length })}
    </button>
  </div>

  <div class="table-wrap glass">
    {#if activeSubTab === 'listening'}
      <table>
        <thead>
          <tr>
            <SortableTh label={$LL.network.protocol()} column="proto" activeColumn={listenSort.column} direction={listenSort.direction} onsort={(c) => { listenSort = nextSort(listenSort, c as ListenSortCol); }} />
            <SortableTh label={$LL.network.localAddress()} column="local" activeColumn={listenSort.column} direction={listenSort.direction} onsort={(c) => { listenSort = nextSort(listenSort, c as ListenSortCol); }} />
            <SortableTh label={$LL.network.process()} column="process" activeColumn={listenSort.column} direction={listenSort.direction} onsort={(c) => { listenSort = nextSort(listenSort, c as ListenSortCol); }} />
          </tr>
        </thead>
        <tbody>
          {#each sortedListening as row}
            <tr>
              <td><span class="badge">{row.proto}</span></td>
              <td class="mono-val">{row.local}</td>
              <td class="process-cell">{row.process}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <table>
        <thead>
          <tr>
            <SortableTh label={$LL.network.state()} column="state" activeColumn={connSort.column} direction={connSort.direction} onsort={(c) => { connSort = nextSort(connSort, c as ConnSortCol); }} />
            <SortableTh label={$LL.network.local()} column="local" activeColumn={connSort.column} direction={connSort.direction} onsort={(c) => { connSort = nextSort(connSort, c as ConnSortCol); }} />
            <SortableTh label={$LL.network.remote()} column="remote" activeColumn={connSort.column} direction={connSort.direction} onsort={(c) => { connSort = nextSort(connSort, c as ConnSortCol); }} />
          </tr>
        </thead>
        <tbody>
          {#each sortedConnections as row}
            <tr>
              <td><span class="badge success">{row.state}</span></td>
              <td class="mono-val">{row.local}</td>
              <td class="mono-val">{row.remote}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>

  <div class="hint glass">
    <Globe size={14} />
    <span>{$LL.network.hint()}</span>
  </div>
</div>

<style>
  .sub-tabs { display: flex; gap: 6px; flex-shrink: 0; }
  .sub-tab {
    background: transparent;
    border: 1px solid var(--border-color);
    color: var(--text-secondary);
    padding: 6px 12px;
    font-size: 0.8rem;
    border-radius: var(--radius-sm);
  }
  .sub-tab.active {
    background: var(--bg-active);
    color: var(--accent-amber);
    border-color: rgba(245, 158, 11, 0.25);
  }
  .table-wrap {
    flex: 1;
    overflow: auto;
    border-radius: var(--radius-md);
    padding: 0;
  }
  table { width: 100%; border-collapse: collapse; font-size: 0.8rem; }
  th, td { padding: 8px 12px; text-align: left; border-bottom: 1px solid var(--border-color); }
  th { color: var(--text-muted); font-weight: 500; position: sticky; top: 0; background: var(--bg-secondary); }
  .process-cell { color: var(--text-secondary); max-width: 400px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .hint { padding: 10px 12px; display: flex; gap: 8px; align-items: center; font-size: 0.78rem; color: var(--text-muted); border-radius: var(--radius-md); flex-shrink: 0; }
  .spin { animation: spin 1s linear infinite; }
  @keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
</style>
