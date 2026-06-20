<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { Database, Table, Play, Plug, RefreshCw, Download, Box, Server, ChevronRight, Terminal } from 'lucide-svelte';
  import { LL } from '$lib/i18n/i18n-svelte';
  import { get } from 'svelte/store';
  import { listContainers } from '$lib/exec/target';
  import { notifications } from '$lib/notifications.svelte';
  import { formatInvokeError } from '$lib/i18n/backendErrors';

  let { profileId = '' } = $props();

  interface QueryResult {
    columns: string[];
    rows: string[][];
    message: string | null;
  }

  type Engine = 'mysql' | 'postgres';

  let engine = $state<Engine>('mysql');
  let host = $state('127.0.0.1');
  let port = $state('3306');
  let user = $state('root');
  let password = $state('');
  let useDocker = $state(false);
  let container = $state('');
  let containers = $state<string[]>([]);

  let connected = $state(false);
  let connecting = $state(false);

  let databases = $state<string[]>([]);
  let selectedDb = $state('');
  let tables = $state<string[]>([]);
  let selectedTable = $state('');

  let mode = $state<'browse' | 'sql'>('browse');
  let sqlText = $state('');
  let result = $state<QueryResult | null>(null);
  let running = $state(false);
  let errorMsg = $state('');

  const storeKey = $derived(`jarvis-db-conn-${profileId}`);

  $effect(() => {
    if (errorMsg) {
      notifications.error(errorMsg);
      errorMsg = '';
    }
  });

  function defaultPort(e: Engine) {
    return e === 'mysql' ? '3306' : '5432';
  }

  function quoteIdent(name: string): string {
    return engine === 'mysql' ? `\`${name.replace(/`/g, '``')}\`` : `"${name.replace(/"/g, '""')}"`;
  }

  async function query(sql: string, database: string | null): Promise<QueryResult> {
    return invoke<QueryResult>('db_query', {
      engine,
      host,
      port,
      user,
      password,
      database,
      container: useDocker && container ? container : null,
      sql,
    });
  }

  async function loadContainers() {
    containers = await listContainers(false);
    if (containers.length && !container) container = containers[0];
  }

  async function connect() {
    connecting = true;
    errorMsg = '';
    try {
      const listSql =
        engine === 'mysql'
          ? 'SHOW DATABASES'
          : 'SELECT datname FROM pg_database WHERE datistemplate = false ORDER BY 1';
      const res = await query(listSql, engine === 'postgres' ? 'postgres' : null);
      databases = res.rows.map((r) => r[0]).filter(Boolean);
      connected = true;
      // Persist connection (without password)
      localStorage.setItem(
        storeKey,
        JSON.stringify({ engine, host, port, user, useDocker, container }),
      );
    } catch (err) {
      errorMsg = get(LL).database.connectFailed({ error: formatInvokeError(err) });
      connected = false;
    } finally {
      connecting = false;
    }
  }

  async function selectDatabase(db: string) {
    selectedDb = db;
    selectedTable = '';
    result = null;
    try {
      const tablesSql =
        engine === 'mysql'
          ? 'SHOW TABLES'
          : "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public' ORDER BY 1";
      const res = await query(tablesSql, db);
      tables = res.rows.map((r) => r[0]).filter(Boolean);
    } catch (err) {
      errorMsg = formatInvokeError(err);
      tables = [];
    }
  }

  async function browseTable(table: string) {
    selectedTable = table;
    mode = 'browse';
    running = true;
    errorMsg = '';
    try {
      result = await query(`SELECT * FROM ${quoteIdent(table)} LIMIT 200`, selectedDb);
    } catch (err) {
      errorMsg = get(LL).database.queryError({ error: formatInvokeError(err) });
    } finally {
      running = false;
    }
  }

  async function runSql() {
    if (!sqlText.trim()) return;
    if (!selectedDb) {
      errorMsg = get(LL).database.selectDbFirst();
      return;
    }
    running = true;
    errorMsg = '';
    try {
      result = await query(sqlText, selectedDb);
    } catch (err) {
      errorMsg = get(LL).database.queryError({ error: formatInvokeError(err) });
    } finally {
      running = false;
    }
  }

  function exportCsv() {
    if (!result || result.columns.length === 0) return;
    const esc = (v: string) => `"${(v ?? '').replace(/"/g, '""')}"`;
    const lines = [result.columns.map(esc).join(',')];
    for (const row of result.rows) lines.push(row.map(esc).join(','));
    const blob = new Blob([lines.join('\n')], { type: 'text/csv' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${selectedTable || 'query'}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  }

  onMount(() => {
    const saved = localStorage.getItem(storeKey);
    if (saved) {
      try {
        const c = JSON.parse(saved);
        engine = c.engine ?? 'mysql';
        host = c.host ?? '127.0.0.1';
        port = c.port ?? defaultPort(engine);
        user = c.user ?? 'root';
        useDocker = c.useDocker ?? false;
        container = c.container ?? '';
      } catch {}
    }
    loadContainers();
  });
</script>

<div class="database-manager manager-shell fade-in">
  <header class="manager-header">
    <h1 class="page-title">{$LL.database.title()}</h1>
  </header>

  <div class="conn-bar glass">
    <div class="engine-toggle">
      <button class="eng-btn" class:active={engine === 'mysql'} onclick={() => { engine = 'mysql'; port = defaultPort('mysql'); }}>MySQL</button>
      <button class="eng-btn" class:active={engine === 'postgres'} onclick={() => { engine = 'postgres'; port = defaultPort('postgres'); }}>PostgreSQL</button>
    </div>
    <input class="ci host" type="text" bind:value={host} placeholder={$LL.database.host()} />
    <input class="ci port" type="text" bind:value={port} placeholder={$LL.database.port()} />
    <input class="ci user" type="text" bind:value={user} placeholder={$LL.database.user()} />
    <input class="ci pass" type="password" bind:value={password} placeholder={$LL.database.password()} onkeydown={(e) => e.key === 'Enter' && connect()} />
    <label class="docker-toggle" title={$LL.database.useDocker()}>
      <input type="checkbox" bind:checked={useDocker} /> <Box size={13} />
    </label>
    {#if useDocker}
      <select class="ci container" bind:value={container}>
        {#if containers.length === 0}<option value="">{$LL.database.noContainers()}</option>{/if}
        {#each containers as c}<option value={c}>{c}</option>{/each}
      </select>
    {/if}
    <button class="primary btn-compact" disabled={connecting} onclick={connect}>
      <Plug size={14} /> {connecting ? $LL.database.connecting() : $LL.database.connect()}
    </button>
  </div>

  {#if connected}
    <div class="db-workspace">
      <aside class="db-sidebar glass">
        <div class="sb-section">
          <div class="sb-title"><Server size={13} /> {$LL.database.databases()}</div>
          <div class="sb-list">
            {#each databases as db}
              <button class="sb-item" class:active={selectedDb === db} onclick={() => selectDatabase(db)}>
                <Database size={12} /> {db}
              </button>
            {/each}
          </div>
        </div>
        {#if selectedDb}
          <div class="sb-section">
            <div class="sb-title"><Table size={13} /> {$LL.database.tables()} ({tables.length})</div>
            <div class="sb-list">
              {#each tables as t}
                <button class="sb-item" class:active={selectedTable === t} onclick={() => browseTable(t)}>
                  <ChevronRight size={11} /> {t}
                </button>
              {/each}
            </div>
          </div>
        {/if}
      </aside>

      <main class="db-main">
        <div class="main-tabs">
          <button class="mtab" class:active={mode === 'browse'} onclick={() => (mode = 'browse')}><Table size={13} /> {$LL.database.browse()}</button>
          <button class="mtab" class:active={mode === 'sql'} onclick={() => (mode = 'sql')}><Terminal size={13} /> {$LL.database.sqlEditor()}</button>
          <div class="mtab-spacer"></div>
          {#if result && result.columns.length}
            <button class="secondary btn-compact" onclick={exportCsv}><Download size={13} /> {$LL.database.exportCsv()}</button>
          {/if}
        </div>

        {#if mode === 'sql'}
          <div class="sql-box">
            <textarea class="sql-area" bind:value={sqlText} placeholder={selectedDb ? `SELECT * FROM ... ` : $LL.database.selectDbFirst()} spellcheck="false"></textarea>
            <button class="primary btn-compact run-btn" disabled={running} onclick={runSql}><Play size={14} /> {running ? $LL.database.running() : $LL.database.runQuery()}</button>
          </div>
        {/if}

        <div class="result-area glass">
          {#if running}
            <div class="result-empty">{$LL.common.loading()}</div>
          {:else if !result}
            <div class="result-empty"><Database size={22} /> {$LL.database.pickHint()}</div>
          {:else if result.message}
            <div class="result-msg">{result.message}</div>
          {:else if result.columns.length === 0}
            <div class="result-empty">{$LL.database.noResults()}</div>
          {:else}
            <div class="result-meta">{$LL.database.rows({ count: result.rows.length })}</div>
            <div class="grid-scroll">
              <table>
                <thead>
                  <tr>{#each result.columns as col}<th>{col}</th>{/each}</tr>
                </thead>
                <tbody>
                  {#each result.rows as row}
                    <tr>{#each row as cell}<td title={cell}>{cell}</td>{/each}</tr>
                  {/each}
                </tbody>
              </table>
            </div>
          {/if}
        </div>
      </main>
    </div>
  {:else}
    <div class="empty glass"><Database size={26} /> {$LL.database.connectHint()}</div>
  {/if}
</div>

<style>
  .conn-bar { display: flex; gap: 8px; align-items: center; padding: 10px; border-radius: var(--radius-md); flex-shrink: 0; flex-wrap: wrap; }
  .engine-toggle { display: flex; gap: 2px; border: 1px solid var(--border-color); border-radius: var(--radius-sm); overflow: hidden; }
  .eng-btn { background: transparent; border: none; color: var(--text-secondary); padding: 6px 12px; font-size: 0.76rem; cursor: pointer; }
  .eng-btn.active { background: var(--bg-active); color: var(--accent-amber); }
  .ci { background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--radius-sm); padding: 7px 9px; color: var(--text-primary); font-size: 0.8rem; }
  .ci.host { width: 130px; } .ci.port { width: 70px; } .ci.user { width: 110px; } .ci.pass { width: 130px; } .ci.container { width: 150px; }
  .docker-toggle { display: flex; align-items: center; gap: 4px; color: var(--text-muted); font-size: 0.78rem; cursor: pointer; }
  .empty { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; color: var(--text-muted); border-radius: var(--radius-md); }
  .db-workspace { flex: 1; display: flex; gap: 10px; overflow: hidden; }
  .db-sidebar { width: 230px; flex-shrink: 0; overflow-y: auto; padding: 10px; border-radius: var(--radius-md); display: flex; flex-direction: column; gap: 14px; }
  .sb-title { display: flex; align-items: center; gap: 6px; font-size: 0.7rem; text-transform: uppercase; letter-spacing: 0.05em; color: var(--text-muted); margin-bottom: 6px; }
  .sb-list { display: flex; flex-direction: column; gap: 2px; }
  .sb-item { display: flex; align-items: center; gap: 6px; background: transparent; border: none; color: var(--text-secondary); padding: 5px 8px; border-radius: var(--radius-sm); cursor: pointer; font-size: 0.78rem; text-align: left; width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: var(--font-mono); }
  .sb-item:hover { background: var(--bg-hover); color: var(--text-primary); }
  .sb-item.active { background: var(--bg-active); color: var(--accent-amber); }
  .db-main { flex: 1; display: flex; flex-direction: column; gap: 10px; overflow: hidden; }
  .main-tabs { display: flex; gap: 6px; align-items: center; flex-shrink: 0; }
  .mtab { display: flex; align-items: center; gap: 6px; background: transparent; border: 1px solid var(--border-color); color: var(--text-secondary); padding: 6px 12px; font-size: 0.78rem; border-radius: var(--radius-sm); cursor: pointer; }
  .mtab.active { background: var(--bg-active); color: var(--accent-amber); border-color: rgba(245,158,11,0.25); }
  .mtab-spacer { flex: 1; }
  .sql-box { display: flex; flex-direction: column; gap: 8px; flex-shrink: 0; }
  .sql-area { min-height: 120px; background: var(--bg-primary); border: 1px solid var(--border-color); border-radius: var(--radius-sm); padding: 10px; color: var(--text-primary); font-family: var(--font-mono); font-size: 0.8rem; resize: vertical; }
  .run-btn { align-self: flex-start; }
  .result-area { flex: 1; overflow: hidden; border-radius: var(--radius-md); display: flex; flex-direction: column; }
  .result-empty { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; color: var(--text-muted); padding: 30px; }
  .result-msg { padding: 16px; color: var(--accent-green); font-family: var(--font-mono); font-size: 0.85rem; }
  .result-meta { padding: 8px 12px; font-size: 0.72rem; color: var(--text-muted); border-bottom: 1px solid var(--border-color); flex-shrink: 0; }
  .grid-scroll { flex: 1; overflow: auto; }
  table { width: 100%; border-collapse: collapse; font-size: 0.76rem; }
  th, td { padding: 6px 10px; text-align: left; border-bottom: 1px solid var(--border-color); border-right: 1px solid var(--border-color); white-space: nowrap; max-width: 320px; overflow: hidden; text-overflow: ellipsis; }
  th { color: var(--text-muted); font-weight: 600; position: sticky; top: 0; background: var(--bg-secondary); font-family: var(--font-mono); }
  td { color: var(--text-secondary); font-family: var(--font-mono); }
</style>
