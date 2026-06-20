<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { BarChart3, Play, FileSearch } from 'lucide-svelte';
  import SudoModal from './SudoModal.svelte';
  import { LL } from '$lib/i18n/i18n-svelte';
  import { get } from 'svelte/store';
  import { shQuote } from '$lib/exec/target';
  import { notifications } from '$lib/notifications.svelte';
  import { formatInvokeError, isSudoPasswordRequired } from '$lib/i18n/backendErrors';

  interface Row { count: number; value: string; }

  const presets: { id: 'nginx' | 'apache' | 'apacheHttpd'; path: string }[] = [
    { id: 'nginx', path: '/var/log/nginx/access.log' },
    { id: 'apache', path: '/var/log/apache2/access.log' },
    { id: 'apacheHttpd', path: '/var/log/httpd/access_log' },
  ];

  let logPath = $state('/var/log/nginx/access.log');
  let lines = $state(50000);
  let analyzing = $state(false);
  let errorMsg = $state('');
  let hasRun = $state(false);

  let total = $state(0);
  let statusRows = $state<Row[]>([]);
  let ipRows = $state<Row[]>([]);
  let pathRows = $state<Row[]>([]);
  let methodRows = $state<Row[]>([]);
  let uaRows = $state<Row[]>([]);
  let hourRows = $state<Row[]>([]);

  let showSudoModal = $state(false);
  let pendingAction: (() => Promise<void>) | null = null;

  $effect(() => {
    if (errorMsg) {
      notifications.error(errorMsg);
      errorMsg = '';
    }
  });

  function parseSection(text: string): Row[] {
    const rows: Row[] = [];
    for (const line of text.trim().split('\n')) {
      const m = line.trim().match(/^(\d+)\s+(.*)$/);
      if (m) rows.push({ count: parseInt(m[1]), value: m[2] || '—' });
    }
    return rows;
  }

  function maxOf(rows: Row[]): number {
    return rows.reduce((mx, r) => Math.max(mx, r.count), 1);
  }

  async function analyze() {
    const lp = shQuote(logPath);
    const n = Math.max(1000, Math.min(1000000, Number(lines) || 50000));
    const cmd =
      `if ! test -r ${lp}; then echo "__NOACCESS__"; else ` +
      `T=$(tail -n ${n} ${lp}); ` +
      `echo '@@@TOTAL'; printf '%s\\n' "$T" | wc -l; ` +
      `echo '@@@STATUS'; printf '%s\\n' "$T" | awk '{print $9}' | sort | uniq -c | sort -rn | head -20; ` +
      `echo '@@@IP'; printf '%s\\n' "$T" | awk '{print $1}' | sort | uniq -c | sort -rn | head -20; ` +
      `echo '@@@PATH'; printf '%s\\n' "$T" | awk '{print $7}' | sort | uniq -c | sort -rn | head -25; ` +
      `echo '@@@METHOD'; printf '%s\\n' "$T" | awk '{print $6}' | tr -d '\"' | sort | uniq -c | sort -rn | head -10; ` +
      `echo '@@@UA'; printf '%s\\n' "$T" | awk -F'\"' '{print $6}' | sort | uniq -c | sort -rn | head -15; ` +
      `echo '@@@HOUR'; printf '%s\\n' "$T" | awk '{print $4}' | awk -F: '{print $2}' | sort | uniq -c | sort -k2n; ` +
      `fi`;

    const run = async (useSudo: boolean) => {
      analyzing = true;
      errorMsg = '';
      try {
        const out = await invoke<string>('exec_custom_command', { cmd, useSudo });
        if (out.includes('__NOACCESS__')) {
          // Retry with sudo
          pendingAction = () => run(true);
          showSudoModal = true;
          return;
        }
        const sections: Record<string, string> = {};
        for (const block of out.split('@@@').slice(1)) {
          const nl = block.indexOf('\n');
          const name = block.slice(0, nl).trim();
          sections[name] = block.slice(nl + 1);
        }
        total = parseInt((sections['TOTAL'] || '0').trim()) || 0;
        statusRows = parseSection(sections['STATUS'] || '');
        ipRows = parseSection(sections['IP'] || '');
        pathRows = parseSection(sections['PATH'] || '');
        methodRows = parseSection(sections['METHOD'] || '');
        uaRows = parseSection(sections['UA'] || '');
        hourRows = parseSection(sections['HOUR'] || '');
        hasRun = true;
      } catch (err) {
        if (isSudoPasswordRequired(err)) {
          pendingAction = () => run(true);
          showSudoModal = true;
        } else {
          errorMsg = formatInvokeError(err);
        }
      } finally {
        analyzing = false;
      }
    };
    await run(false);
  }

  function statusClass(code: string): string {
    if (code.startsWith('2')) return 'ok';
    if (code.startsWith('3')) return 'redir';
    if (code.startsWith('4')) return 'warn';
    if (code.startsWith('5')) return 'err';
    return '';
  }
</script>

<div class="log-analysis manager-shell fade-in">
  <header class="manager-header">
    <h1 class="page-title">{$LL.loganalysis.title()}</h1>
  </header>

  <div class="control-bar glass">
    <div class="presets">
      {#each presets as p}
        <button class="preset-chip" class:active={logPath === p.path} onclick={() => (logPath = p.path)}>{$LL.loganalysis.presets[p.id]()}</button>
      {/each}
    </div>
    <input class="path-input" type="text" bind:value={logPath} placeholder="/var/log/nginx/access.log" />
    <label class="lines-label">
      {$LL.loganalysis.lines()}
      <input class="lines-input" type="number" bind:value={lines} min="1000" step="1000" />
    </label>
    <button class="primary btn-compact" disabled={analyzing} onclick={analyze}>
      <Play size={14} /> {analyzing ? $LL.loganalysis.analyzing() : $LL.loganalysis.analyze()}
    </button>
  </div>

  {#if !hasRun}
    <div class="placeholder glass">
      <FileSearch size={32} />
      <p>{$LL.loganalysis.emptyHint()}</p>
    </div>
  {:else}
    <div class="results">
      <div class="stat-row">
        <div class="stat-card glass">
          <span class="stat-num">{total.toLocaleString()}</span>
          <span class="stat-label">{$LL.loganalysis.totalRequests()}</span>
        </div>
        <div class="status-chips glass">
          {#each statusRows as s}
            <div class="status-chip {statusClass(s.value)}">
              <span class="sc-code">{s.value}</span>
              <span class="sc-count">{s.count.toLocaleString()}</span>
            </div>
          {/each}
        </div>
      </div>

      <div class="grid-2">
        {#each [{ title: $LL.loganalysis.topIps(), rows: ipRows }, { title: $LL.loganalysis.topPaths(), rows: pathRows }, { title: $LL.loganalysis.methods(), rows: methodRows }, { title: $LL.loganalysis.topUserAgents(), rows: uaRows }, { title: $LL.loganalysis.byHour(), rows: hourRows }] as section}
          {@const mx = maxOf(section.rows)}
          <div class="panel glass">
            <h3 class="panel-title">{section.title}</h3>
            <div class="bars">
              {#each section.rows as r}
                <div class="bar-row" title={r.value}>
                  <span class="bar-label">{r.value}</span>
                  <div class="bar-track"><div class="bar-fill" style="width: {(r.count / mx) * 100}%"></div></div>
                  <span class="bar-count">{r.count.toLocaleString()}</span>
                </div>
              {/each}
              {#if section.rows.length === 0}<span class="empty">{$LL.common.noData()}</span>{/if}
            </div>
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>

<SudoModal bind:open={showSudoModal} onSuccess={() => { const a = pendingAction; pendingAction = null; if (a) a(); }} onCancel={() => (pendingAction = null)} />

<style>
  .control-bar { display: flex; gap: 10px; align-items: center; padding: 10px; border-radius: var(--radius-md); flex-shrink: 0; flex-wrap: wrap; }
  .presets { display: flex; gap: 4px; }
  .preset-chip { background: transparent; border: 1px solid var(--border-color); color: var(--text-secondary); padding: 5px 10px; font-size: 0.74rem; border-radius: var(--radius-sm); cursor: pointer; }
  .preset-chip.active { background: var(--bg-active); color: var(--accent-amber); border-color: rgba(245, 158, 11, 0.25); }
  .path-input { flex: 1; min-width: 200px; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--radius-sm); padding: 7px 10px; color: var(--text-primary); font-size: 0.82rem; font-family: var(--font-mono); }
  .lines-label { display: flex; align-items: center; gap: 6px; font-size: 0.74rem; color: var(--text-muted); }
  .lines-input { width: 100px; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--radius-sm); padding: 7px 8px; color: var(--text-primary); font-size: 0.8rem; }
  .placeholder { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; color: var(--text-muted); border-radius: var(--radius-md); }
  .results { flex: 1; overflow: auto; display: flex; flex-direction: column; gap: 12px; }
  .stat-row { display: flex; gap: 12px; flex-wrap: wrap; }
  .stat-card { padding: 14px 20px; border-radius: var(--radius-md); display: flex; flex-direction: column; gap: 4px; min-width: 140px; }
  .stat-num { font-size: 1.6rem; font-weight: 700; color: var(--accent-amber); font-variant-numeric: tabular-nums; }
  .stat-label { font-size: 0.72rem; color: var(--text-muted); text-transform: uppercase; letter-spacing: 0.05em; }
  .status-chips { flex: 1; display: flex; flex-wrap: wrap; gap: 8px; padding: 12px; border-radius: var(--radius-md); align-content: flex-start; }
  .status-chip { display: flex; flex-direction: column; align-items: center; padding: 6px 12px; border-radius: var(--radius-sm); border: 1px solid var(--border-color); min-width: 64px; }
  .status-chip.ok { border-color: rgba(34,197,94,0.4); }
  .status-chip.redir { border-color: rgba(59,130,246,0.4); }
  .status-chip.warn { border-color: rgba(245,158,11,0.4); }
  .status-chip.err { border-color: rgba(239,68,68,0.5); background: rgba(239,68,68,0.08); }
  .sc-code { font-weight: 700; font-size: 0.9rem; font-family: var(--font-mono); }
  .sc-count { font-size: 0.7rem; color: var(--text-muted); }
  .grid-2 { display: grid; grid-template-columns: repeat(auto-fill, minmax(340px, 1fr)); gap: 12px; }
  .panel { padding: 12px; border-radius: var(--radius-md); }
  .panel-title { font-size: 0.8rem; color: var(--text-secondary); margin-bottom: 10px; text-transform: uppercase; letter-spacing: 0.04em; }
  .bars { display: flex; flex-direction: column; gap: 5px; }
  .bar-row { display: grid; grid-template-columns: 1fr 90px auto; align-items: center; gap: 8px; font-size: 0.74rem; }
  .bar-label { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-secondary); font-family: var(--font-mono); }
  .bar-track { height: 8px; background: var(--bg-primary); border-radius: 4px; overflow: hidden; }
  .bar-fill { height: 100%; background: var(--accent-primary); border-radius: 4px; }
  .bar-count { font-variant-numeric: tabular-nums; color: var(--text-muted); text-align: right; }
  .empty { color: var(--text-muted); font-size: 0.76rem; }
</style>
