<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { Play, Square, Power, PowerOff, Eye, Timer as TimerIcon, X, Search, Loader2 } from 'lucide-svelte';
  import SortableTh from './ui/SortableTh.svelte';
  import SudoModal from './SudoModal.svelte';
  import { applySort, nextSort, type SortState } from '$lib/sort/sortUtils';
    import { get } from 'svelte/store';
  import { shQuote } from '$lib/exec/target';
  import { notifications } from '$lib/notifications.svelte';
  import { formatInvokeError, isSudoPasswordRequired } from '$lib/backendErrors';

  interface TimerUnit {
    unit: string;
    description: string;
    active: string;
    fileState: string;
    next: number; // epoch ms, 0 = none
    last: number; // epoch ms, 0 = none
  }

  let { visible = true } = $props();

  let timers = $state<TimerUnit[]>([]);
  let isLoading = $state(false);
  let errorMsg = $state('');
  let showSudoModal = $state(false);
  let pendingAction: (() => Promise<void>) | null = null;

  let detailUnit = $state<string | null>(null);
  let detailOutput = $state('');

  let searchQuery = $state('');

  type SortCol = 'unit' | 'next' | 'last' | 'state';
  let sort = $state<SortState<SortCol>>({ column: 'next', direction: 'asc' });

  $effect(() => {
    if (errorMsg) {
      notifications.error(errorMsg);
      errorMsg = '';
    }
  });

  const sorted = $derived(
    applySort(timers, sort, {
      unit: (t) => t.unit,
      next: (t) => t.next || Number.MAX_SAFE_INTEGER,
      last: (t) => t.last || 0,
      state: (t) => t.fileState,
    }),
  );

  const filtered = $derived(() => {
    const q = searchQuery.trim().toLowerCase();
    if (!q) return sorted;
    return sorted.filter(
      (t) => t.unit.toLowerCase().includes(q) || t.description.toLowerCase().includes(q),
    );
  });

  function fmt(ms: number): string {
    if (!ms) return '—';
    try {
      return new Date(ms).toLocaleString();
    } catch {
      return '—';
    }
  }

  function parse(output: string): TimerUnit[] {
    const result: TimerUnit[] = [];
    const blocks = output.split('@@@').map((b) => b.trim()).filter(Boolean);
    for (const block of blocks) {
      const lines = block.split('\n');
      const unit = lines[0].trim();
      if (!unit.endsWith('.timer')) continue;
      const kv: Record<string, string> = {};
      for (const line of lines.slice(1)) {
        const idx = line.indexOf('=');
        if (idx === -1) continue;
        kv[line.slice(0, idx)] = line.slice(idx + 1).trim();
      }
      const toMs = (v: string) => {
        const n = parseInt(v);
        if (!n || isNaN(n)) return 0;
        return Math.floor(n / 1000); // usec -> ms
      };
      result.push({
        unit,
        description: kv['Description'] || '',
        active: kv['ActiveState'] || '',
        fileState: kv['UnitFileState'] || '',
        next: toMs(kv['NextElapseUSecRealtime'] || '0'),
        last: toMs(kv['LastTriggerUSec'] || '0'),
      });
    }
    return result;
  }

  async function load() {
    isLoading = true;
    errorMsg = '';
    const cmd =
      `units=$(( systemctl list-unit-files --type=timer --no-legend --no-pager --plain 2>/dev/null | awk '{print $1}'; ` +
      `systemctl list-units --type=timer --all --no-legend --no-pager --plain 2>/dev/null | awk '{print $1}' ) | sort -u); ` +
      `for u in $units; do echo "@@@$u"; systemctl show "$u" -p Description -p ActiveState -p UnitFileState -p NextElapseUSecRealtime -p LastTriggerUSec 2>/dev/null; done`;
    try {
      const out = await invoke<string>('exec_custom_command', { cmd, useSudo: false });
      timers = parse(out);
    } catch (err) {
      errorMsg = formatInvokeError(err);
    } finally {
      isLoading = false;
    }
  }

  async function action(act: string, unit: string) {
    const run = async () => {
      try {
        await invoke('exec_custom_command', { cmd: `systemctl ${act} ${shQuote(unit)}`, useSudo: true });
        notifications.success(`${act} on ${unit} done`);
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

  async function inspect(unit: string) {
    detailUnit = unit;
    detailOutput = "Loading…";
    try {
      detailOutput = await invoke<string>('exec_custom_command', {
        cmd: `systemctl status ${shQuote(unit)} --no-pager -l 2>&1 | head -n 60; echo '---'; systemctl cat ${shQuote(unit)} 2>/dev/null`,
        useSudo: false,
      });
    } catch (err) {
      detailOutput = formatInvokeError(err);
    }
  }

  export function refresh() { load(); }

  onMount(load);
</script>

<div class="timer-manager manager-shell fade-in">
  <header class="manager-header">
    <h1 class="page-title">Systemd timers</h1>
  </header>

  <div class="ops-bar">
    <div class="search-box">
      <Search size={14} />
      <input type="text" placeholder="Filter timers…" bind:value={searchQuery} />
      {#if searchQuery}
        <button class="clear-search" onclick={() => (searchQuery = '')} aria-label="Clear"><X size={13} /></button>
      {/if}
    </div>
  </div>

  <div class="table-wrap glass">
    <table>
      <thead>
        <tr>
          <SortableTh label="Timer unit" column="unit" activeColumn={sort.column} direction={sort.direction} onsort={(c) => (sort = nextSort(sort, c as SortCol))} />
          <SortableTh label="Next run" column="next" activeColumn={sort.column} direction={sort.direction} onsort={(c) => (sort = nextSort(sort, c as SortCol))} />
          <SortableTh label="Last run" column="last" activeColumn={sort.column} direction={sort.direction} onsort={(c) => (sort = nextSort(sort, c as SortCol))} />
          <SortableTh label="State" column="state" activeColumn={sort.column} direction={sort.direction} onsort={(c) => (sort = nextSort(sort, c as SortCol))} />
          <th class="actions-col">Actions</th>
        </tr>
      </thead>
      <tbody>
        {#each filtered() as t (t.unit)}
          <tr>
            <td>
              <div class="unit-name">{t.unit}</div>
              {#if t.description}<div class="unit-desc">{t.description}</div>{/if}
            </td>
            <td class="mono-val">{fmt(t.next)}</td>
            <td class="mono-val">{fmt(t.last)}</td>
            <td>
              <span class="badge" class:success={t.fileState === 'enabled'} class:muted={t.fileState !== 'enabled'}>{t.fileState || '—'}</span>
              {#if t.active === 'active'}<span class="badge success">{t.active}</span>{/if}
            </td>
            <td class="actions-col">
              <button class="row-btn" onclick={() => inspect(t.unit)} title="Inspect"><Eye size={13} /></button>
              <button class="row-btn" onclick={() => action('start', t.unit)} title="Run now (start)"><Play size={13} /></button>
              <button class="row-btn" onclick={() => action('stop', t.unit)} title="Stop"><Square size={13} /></button>
              {#if t.fileState === 'enabled'}
                <button class="row-btn warn" onclick={() => action('disable', t.unit)} title="Disable"><PowerOff size={13} /></button>
              {:else}
                <button class="row-btn" onclick={() => action('enable', t.unit)} title="Enable"><Power size={13} /></button>
              {/if}
            </td>
          </tr>
        {/each}
        {#if isLoading}
          <tr><td colspan="5" class="loading-cell"><Loader2 size={18} class="spin" /> Loading timers…</td></tr>
        {:else if filtered().length === 0}
          <tr><td colspan="5" class="empty-cell"><TimerIcon size={20} /> {searchQuery ? 'No matching timers' : 'No timers found'}</td></tr>
        {/if}
      </tbody>
    </table>
  </div>
</div>

{#if detailUnit}
  <div class="modal-overlay" role="presentation" onclick={() => (detailUnit = null)}>
    <div class="modal-content glass detail-modal" role="dialog" onclick={(e) => e.stopPropagation()}>
      <div class="modal-header">
        <h3>{detailUnit}</h3>
        <button class="icon-btn-compact" onclick={() => (detailUnit = null)}><X size={16} /></button>
      </div>
      <pre class="detail-output">{detailOutput}</pre>
    </div>
  </div>
{/if}

<SudoModal bind:open={showSudoModal} onSuccess={() => { const a = pendingAction; pendingAction = null; if (a) a(); }} onCancel={() => (pendingAction = null)} />

<style>
  .ops-bar { display: flex; align-items: center; gap: 10px; flex-shrink: 0; }
  .search-box { display: flex; align-items: center; gap: 6px; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--radius-sm); padding: 5px 10px; flex: 1; max-width: 380px; color: var(--text-muted); }
  .search-box input { background: transparent; border: none; outline: none; color: var(--text-primary); font-size: 0.82rem; flex: 1; }
  .clear-search { background: transparent; border: none; color: var(--text-muted); cursor: pointer; display: flex; }
  .table-wrap { flex: 1; overflow: auto; border-radius: var(--radius-md); padding: 0; }
  table { width: 100%; border-collapse: collapse; font-size: 0.8rem; }
  th, td { padding: 8px 12px; text-align: left; border-bottom: 1px solid var(--border-color); vertical-align: top; }
  th { color: var(--text-muted); font-weight: 500; position: sticky; top: 0; background: var(--bg-secondary); z-index: 1; }
  .unit-name { font-family: var(--font-mono); font-size: 0.78rem; color: var(--text-primary); }
  .unit-desc { font-size: 0.72rem; color: var(--text-muted); margin-top: 2px; max-width: 360px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .badge.muted { opacity: 0.6; }
  .actions-col { text-align: right; white-space: nowrap; }
  .row-btn { background: transparent; border: 1px solid var(--border-color); color: var(--text-secondary); border-radius: var(--radius-sm); padding: 4px 6px; cursor: pointer; margin-left: 4px; }
  .row-btn:hover { background: var(--bg-hover); color: var(--text-primary); }
  .row-btn.warn:hover { color: var(--accent-amber); border-color: rgba(245, 158, 11, 0.3); }
  .empty-cell { text-align: center; color: var(--text-muted); padding: 30px; }
  .loading-cell { text-align: center; color: var(--text-muted); padding: 30px; display: flex; align-items: center; justify-content: center; gap: 8px; }
  .detail-modal { width: 760px; max-width: 92vw; max-height: 80vh; display: flex; flex-direction: column; }
  .modal-header { display: flex; justify-content: space-between; align-items: center; }
  .detail-output { flex: 1; overflow: auto; background: var(--bg-primary); border: 1px solid var(--border-color); border-radius: var(--radius-sm); padding: 12px; font-family: var(--font-mono); font-size: 0.74rem; color: var(--text-secondary); white-space: pre-wrap; word-break: break-word; margin-top: 10px; }
</style>
