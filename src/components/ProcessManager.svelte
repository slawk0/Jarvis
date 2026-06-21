<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { Search, Skull, X, Gauge, Cpu, MemoryStick } from 'lucide-svelte';
  import SortableTh from './ui/SortableTh.svelte';
  import SudoModal from './SudoModal.svelte';
  import { applySort, nextSort, type SortState } from '$lib/sort/sortUtils';
  import { LL } from '$lib/i18n/i18n-svelte';
  import { get } from 'svelte/store';
  import { notifications } from '$lib/notifications.svelte';
  import { formatInvokeError, isSudoPasswordRequired } from '$lib/i18n/backendErrors';

  interface Proc {
    pid: string;
    user: string;
    cpu: number;
    mem: number;
    nice: string;
    command: string;
  }

  let { visible = true } = $props();

  let procs = $state<Proc[]>([]);
  let isLoading = $state(false);
  let errorMsg = $state('');
  let searchQuery = $state('');
  let autoRefresh = $state(false);
  let refreshTimer: any = null;

  let showSudoModal = $state(false);
  let pendingAction: (() => Promise<void>) | null = null;

  type SortCol = 'pid' | 'user' | 'cpu' | 'mem' | 'command';
  let sort = $state<SortState<SortCol>>({ column: 'cpu', direction: 'desc' });

  $effect(() => {
    if (errorMsg) {
      notifications.error(errorMsg);
      errorMsg = '';
    }
  });

  const filtered = $derived(
    procs.filter((p) => {
      if (!searchQuery) return true;
      const q = searchQuery.toLowerCase();
      return p.command.toLowerCase().includes(q) || p.user.toLowerCase().includes(q) || p.pid.includes(q);
    }),
  );

  const sorted = $derived(
    applySort(filtered, sort, {
      pid: (p) => parseInt(p.pid) || 0,
      user: (p) => p.user,
      cpu: (p) => p.cpu,
      mem: (p) => p.mem,
      command: (p) => p.command,
    }),
  );

  function parse(output: string): Proc[] {
    const rows: Proc[] = [];
    const lines = output.trim().split('\n');
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];
      if (!line.trim()) continue;
      if (i === 0 && line.includes('PID')) continue;
      const parts = line.trim().split(/\s+/);
      if (parts.length < 6) continue;
      const [pid, user, cpu, mem, nice, ...cmd] = parts;
      rows.push({
        pid,
        user,
        cpu: parseFloat(cpu) || 0,
        mem: parseFloat(mem) || 0,
        nice,
        command: cmd.join(' '),
      });
    }
    return rows;
  }

  async function load() {
    isLoading = true;
    errorMsg = '';
    try {
      const out = await invoke<string>('exec_custom_command', {
        cmd: 'ps -eo pid,user,pcpu,pmem,ni,args --sort=-pcpu 2>/dev/null | head -n 300',
        useSudo: false,
      });
      procs = parse(out);
    } catch (err) {
      errorMsg = formatInvokeError(err);
    } finally {
      isLoading = false;
    }
  }

  async function runPrivileged(cmd: string, successMsg: string) {
    const run = async () => {
      try {
        await invoke('exec_custom_command', { cmd, useSudo: true });
        notifications.success(successMsg);
        await load();
      } catch (err) {
        if (isSudoPasswordRequired(err)) {
          pendingAction = run;
          showSudoModal = true;
        } else {
          errorMsg = formatInvokeError(err);
        }
      }
    };
    await run();
  }

  function killProc(p: Proc, force: boolean) {
    const msg = force ? get(LL).processes.confirmForceKill({ pid: p.pid }) : get(LL).processes.confirmKill({ pid: p.pid });
    if (!confirm(msg)) return;
    runPrivileged(`kill ${force ? '-9 ' : ''}${p.pid}`, get(LL).processes.killed({ pid: p.pid }));
  }

  function renice(p: Proc) {
    const val = prompt(get(LL).processes.nicePrompt({ pid: p.pid }), p.nice);
    if (val === null) return;
    const n = parseInt(val);
    if (isNaN(n) || n < -20 || n > 19) {
      errorMsg = get(LL).processes.niceInvalid();
      return;
    }
    runPrivileged(`renice -n ${n} -p ${p.pid}`, get(LL).processes.reniced({ pid: p.pid, nice: n }));
  }

  function toggleAuto() {
    autoRefresh = !autoRefresh;
    if (autoRefresh) {
      // Skip the tick while this pane is hidden (kept alive) to avoid wasted SSH calls.
      refreshTimer = setInterval(() => { if (visible) load(); }, 3000);
    } else if (refreshTimer) {
      clearInterval(refreshTimer);
      refreshTimer = null;
    }
  }

  export function refresh() { load(); }

  onMount(load);
  onDestroy(() => {
    if (refreshTimer) clearInterval(refreshTimer);
  });
</script>

<div class="process-manager manager-shell fade-in">
  <header class="manager-header">
    <h1 class="page-title">{$LL.processes.title()}</h1>
    <div class="header-actions">
      <button class="secondary btn-compact" class:active={autoRefresh} onclick={toggleAuto}>
        <Gauge size={14} /> {autoRefresh ? $LL.processes.autoOn() : $LL.processes.autoOff()}
      </button>
    </div>
  </header>

  <div class="toolbar">
    <div class="search-box">
      <Search size={14} />
      <input type="text" placeholder={$LL.processes.searchPlaceholder()} bind:value={searchQuery} />
      {#if searchQuery}
        <button class="clear-search" onclick={() => (searchQuery = '')} aria-label={$LL.common.clear()}><X size={13} /></button>
      {/if}
    </div>
    <span class="count-badge">{$LL.processes.count({ count: filtered.length })}</span>
  </div>

  <div class="table-wrap glass">
    <table>
      <thead>
        <tr>
          <SortableTh label="PID" column="pid" activeColumn={sort.column} direction={sort.direction} onsort={(c) => (sort = nextSort(sort, c as SortCol))} />
          <SortableTh label={$LL.processes.user()} column="user" activeColumn={sort.column} direction={sort.direction} onsort={(c) => (sort = nextSort(sort, c as SortCol))} />
          <SortableTh label="CPU %" column="cpu" activeColumn={sort.column} direction={sort.direction} onsort={(c) => (sort = nextSort(sort, c as SortCol))} />
          <SortableTh label="MEM %" column="mem" activeColumn={sort.column} direction={sort.direction} onsort={(c) => (sort = nextSort(sort, c as SortCol))} />
          <SortableTh label={$LL.processes.command()} column="command" activeColumn={sort.column} direction={sort.direction} onsort={(c) => (sort = nextSort(sort, c as SortCol))} />
          <th class="actions-col">{$LL.common.actions()}</th>
        </tr>
      </thead>
      <tbody>
        {#each sorted as p (p.pid)}
          <tr>
            <td class="mono-val">{p.pid}</td>
            <td>{p.user}</td>
            <td><span class="metric" class:hot={p.cpu > 50}><Cpu size={11} /> {p.cpu.toFixed(1)}</span></td>
            <td><span class="metric" class:hot={p.mem > 50}><MemoryStick size={11} /> {p.mem.toFixed(1)}</span></td>
            <td class="cmd-cell" title={p.command}>{p.command}</td>
            <td class="actions-col">
              <button class="row-btn" onclick={() => renice(p)} title={$LL.processes.renice()}><Gauge size={13} /></button>
              <button class="row-btn warn" onclick={() => killProc(p, false)} title={$LL.processes.kill()}>TERM</button>
              <button class="row-btn danger" onclick={() => killProc(p, true)} title={$LL.processes.forceKill()}><Skull size={13} /></button>
            </td>
          </tr>
        {/each}
        {#if sorted.length === 0}
          <tr><td colspan="6" class="empty-cell">{$LL.common.noData()}</td></tr>
        {/if}
      </tbody>
    </table>
  </div>
</div>

<SudoModal bind:open={showSudoModal} onSuccess={() => { const a = pendingAction; pendingAction = null; if (a) a(); }} onCancel={() => (pendingAction = null)} />

<style>
  .header-actions { display: flex; gap: 6px; }
  .btn-compact.active { background: var(--bg-active); color: var(--accent-amber); border-color: rgba(245, 158, 11, 0.25); }
  .toolbar { display: flex; align-items: center; gap: 10px; flex-shrink: 0; }
  .search-box { display: flex; align-items: center; gap: 6px; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--radius-sm); padding: 5px 10px; flex: 1; max-width: 380px; color: var(--text-muted); }
  .search-box input { background: transparent; border: none; outline: none; color: var(--text-primary); font-size: 0.82rem; flex: 1; }
  .clear-search { background: transparent; border: none; color: var(--text-muted); cursor: pointer; display: flex; }
  .count-badge { font-family: var(--font-mono); font-size: 0.7rem; color: var(--text-muted); }
  .table-wrap { flex: 1; overflow: auto; border-radius: var(--radius-md); padding: 0; }
  table { width: 100%; border-collapse: collapse; font-size: 0.8rem; }
  th, td { padding: 7px 12px; text-align: left; border-bottom: 1px solid var(--border-color); }
  th { color: var(--text-muted); font-weight: 500; position: sticky; top: 0; background: var(--bg-secondary); z-index: 1; }
  .cmd-cell { color: var(--text-secondary); max-width: 460px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: var(--font-mono); font-size: 0.74rem; }
  .metric { display: inline-flex; align-items: center; gap: 4px; font-variant-numeric: tabular-nums; }
  .metric.hot { color: var(--accent-red); font-weight: 600; }
  .actions-col { text-align: right; white-space: nowrap; }
  .row-btn { background: transparent; border: 1px solid var(--border-color); color: var(--text-secondary); border-radius: var(--radius-sm); padding: 3px 7px; font-size: 0.68rem; cursor: pointer; margin-left: 4px; display: inline-flex; align-items: center; gap: 3px; }
  .row-btn:hover { background: var(--bg-hover); color: var(--text-primary); }
  .row-btn.warn:hover { color: var(--accent-amber); border-color: rgba(245, 158, 11, 0.3); }
  .row-btn.danger:hover { color: var(--accent-red); border-color: rgba(239, 68, 68, 0.3); }
  .empty-cell { text-align: center; color: var(--text-muted); padding: 24px; }
</style>
