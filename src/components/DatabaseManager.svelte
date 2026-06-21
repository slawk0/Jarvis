<script lang="ts">
  import { onMount, onDestroy, untrack } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { Database, Table, Play, Plug, PlugZap, Download, Server, ChevronRight, Terminal, Settings2, Save, Plus, Trash2, X, FileCode, RefreshCw, Container, HardDrive, Wand2, Columns3, Pencil, Check, ChevronLeft, Filter as FilterIcon, Eye } from 'lucide-svelte';
    import { get } from 'svelte/store';
  import { shQuote, listContainers } from '$lib/exec/target';
  import { notifications } from '$lib/notifications.svelte';
  import { formatInvokeError } from '$lib/backendErrors';
  import { defaultPort, engineLabel, FILTER_OPS, type Engine } from '$lib/db/dialect';

  let { profileId = '', visible = true } = $props();

  export function refresh() {
    loadContainers();
    if (profileId) loadProfiles();
  }

  // ---------------------------------------------------------------------------
  // Backend result shapes (camelCase, matching serde rename_all)
  // ---------------------------------------------------------------------------
  interface QueryResult {
    columns: string[];
    rows: (string | null)[][];
    message: string | null;
    rowsAffected?: number | null;
  }
  interface SelectResult {
    columns: string[];
    rows: (string | null)[][];
    total: number;
  }
  interface TableInfo {
    name: string;
    kind: string;
  }
  interface ColumnInfo {
    name: string;
    dataType: string;
    nullable: boolean;
    default: string | null;
    key: string;
    extra: string;
    comment: string;
  }
  interface IndexInfo {
    name: string;
    columns: string[];
    unique: boolean;
    primary: boolean;
  }
  interface ForeignKeyInfo {
    name: string;
    column: string;
    refTable: string;
    refColumn: string;
  }
  interface TableStructure {
    columns: ColumnInfo[];
    indexes: IndexInfo[];
    foreignKeys: ForeignKeyInfo[];
    primaryKey: string[];
  }

  interface Filter {
    column: string;
    op: string;
    value: string | null;
  }

  // A saved connection profile. Stored per server profile in localStorage.
  interface DbProfile {
    id: string;
    name: string;
    engine: Engine;
    kind: 'host' | 'docker';
    container: string;
    host: string;
    port: string;
    user: string;
    password: string;
  }

  // ---------------------------------------------------------------------------
  // Profile state
  // ---------------------------------------------------------------------------
  const getProfilesKey = () => `jarvis-db-profiles-${profileId}`;
  const getActiveKey = () => `jarvis-db-active-${profileId}`;

  let profiles = $state<DbProfile[]>([]);
  let activeProfileId = $state('');

  // Active connection context, derived from the active profile.
  let engine = $state<Engine>('mysql');
  let host = $state('127.0.0.1');
  let port = $state('3306');
  let user = $state('root');
  let password = $state('');
  let useDocker = $state(false);
  let container = $state('');

  // Profile form / setup state
  let showProfilesModal = $state(false);
  let isEditingProfile = $state(false);
  let profileFormId = $state('');
  let profileFormName = $state('');
  let profileFormEngine = $state<Engine>('mysql');
  let profileFormKind = $state<'host' | 'docker'>('host');
  let profileFormContainer = $state('');
  let profileFormHost = $state('127.0.0.1');
  let profileFormPort = $state('3306');
  let profileFormUser = $state('root');
  let profileFormPassword = $state('');

  let containers = $state<string[]>([]);
  let loadingContainers = $state(false);
  let detecting = $state(false);

  // ---------------------------------------------------------------------------
  // Connection / browsing state
  // ---------------------------------------------------------------------------
  let connectionId = $state('');
  let connected = $state(false);
  let connecting = $state(false);
  // The database a Postgres connection is currently bound to (PG cannot switch
  // databases on a live connection; MySQL switches per query).
  let pgDatabase = $state('');

  let databases = $state<string[]>([]);
  let selectedDb = $state('');
  let tables = $state<TableInfo[]>([]);
  let selectedTable = $state('');
  let selectedKind = $state('table');

  let view = $state<'data' | 'structure' | 'sql'>('data');
  let structure = $state<TableStructure | null>(null);
  let errorMsg = $state('');

  // Data grid state
  let dataColumns = $state<string[]>([]);
  let dataRows = $state<(string | null)[][]>([]);
  let total = $state(0);
  let page = $state(0);
  let pageSize = $state(50);
  let orderBy = $state('');
  let orderDir = $state<'asc' | 'desc'>('asc');
  let filters = $state<Filter[]>([]);
  let running = $state(false);

  // Inline editing
  let editRowIndex = $state<number | null>(null);
  let editValues = $state<(string | null)[]>([]);
  let inserting = $state(false);
  let insertValues = $state<(string | null)[]>([]);
  let selectedRows = $state<Set<number>>(new Set());

  // SQL editor
  let sqlText = $state('');
  let sqlResult = $state<QueryResult | null>(null);
  let sqlRunning = $state(false);

  $effect(() => {
    if (errorMsg) {
      notifications.error(errorMsg);
      errorMsg = '';
    }
  });

  const pages = $derived(Math.max(1, Math.ceil(total / pageSize)));
  const whereCols = $derived(
    structure && structure.primaryKey.length ? structure.primaryKey : dataColumns,
  );

  async function loadContainers() {
    loadingContainers = true;
    containers = await listContainers(false);
    if (containers.length > 0 && !profileFormContainer) profileFormContainer = containers[0];
    loadingContainers = false;
  }

  // ---------------------------------------------------------------------------
  // Profile management
  // ---------------------------------------------------------------------------
  function loadProfiles() {
    if (!profileId) return;
    const stored = localStorage.getItem(getProfilesKey());
    const storedActive = localStorage.getItem(getActiveKey());
    if (stored) {
      try {
        const parsed: DbProfile[] = JSON.parse(stored);
        profiles = parsed.map((p) => ({
          id: p.id || Math.random().toString(36).substring(7),
          name: p.name || 'Database',
          engine: p.engine === 'postgres' ? 'postgres' : 'mysql',
          kind: p.kind === 'docker' ? 'docker' : 'host',
          container: p.container || '',
          host: p.host || '127.0.0.1',
          port: p.port || defaultPort(p.engine === 'postgres' ? 'postgres' : 'mysql'),
          user: p.user || 'root',
          password: p.password || '',
        }));
      } catch {
        profiles = [];
      }
    } else {
      profiles = [];
    }

    if (storedActive && profiles.some((p) => p.id === storedActive)) {
      activeProfileId = storedActive;
    } else if (profiles.length > 0) {
      activeProfileId = profiles[0].id;
    } else {
      activeProfileId = '';
    }

    applyActiveProfile();
    if (profiles.length === 0) openAddProfile();
  }

  function saveProfiles() {
    if (!profileId) return;
    localStorage.setItem(getProfilesKey(), JSON.stringify(profiles));
  }

  function applyActiveProfile() {
    const active = profiles.find((p) => p.id === activeProfileId);
    if (active) {
      engine = active.engine;
      host = active.host;
      port = active.port;
      user = active.user;
      password = active.password;
      useDocker = active.kind === 'docker';
      container = active.container;
      if (profileId) localStorage.setItem(getActiveKey(), activeProfileId);
    } else {
      engine = 'mysql';
      host = '127.0.0.1';
      port = '3306';
      user = 'root';
      password = '';
      useDocker = false;
      container = '';
    }
    resetBrowsing();
  }

  function resetBrowsing() {
    if (connectionId) invoke('db_disconnect', { connectionId }).catch(() => {});
    connectionId = '';
    connected = false;
    databases = [];
    selectedDb = '';
    tables = [];
    selectedTable = '';
    structure = null;
    dataColumns = [];
    dataRows = [];
    total = 0;
    sqlResult = null;
    cancelEdit();
  }

  function openAddProfile() {
    isEditingProfile = false;
    profileFormId = Math.random().toString(36).substring(7);
    profileFormName = '';
    profileFormEngine = 'mysql';
    profileFormKind = 'host';
    profileFormContainer = containers[0] || '';
    profileFormHost = '127.0.0.1';
    profileFormPort = defaultPort('mysql');
    profileFormUser = 'root';
    profileFormPassword = '';
  }

  function openEditProfile(p: DbProfile) {
    isEditingProfile = true;
    profileFormId = p.id;
    profileFormName = p.name;
    profileFormEngine = p.engine;
    profileFormKind = p.kind;
    profileFormContainer = p.kind === 'docker' ? p.container : containers[0] || '';
    profileFormHost = p.host;
    profileFormPort = p.port;
    profileFormUser = p.user;
    profileFormPassword = p.password;
  }

  function onFormEngineChange(e: Engine) {
    profileFormEngine = e;
    if (profileFormPort === '3306' || profileFormPort === '5432' || !profileFormPort) {
      profileFormPort = defaultPort(e);
    }
    if (profileFormUser === 'root' || profileFormUser === 'postgres' || !profileFormUser) {
      profileFormUser = e === 'postgres' ? 'postgres' : 'root';
    }
  }

  function saveProfileForm() {
    if (!profileFormName.trim()) {
      notifications.error("Please enter a profile name");
      return;
    }
    const finalId = profileFormId || Math.random().toString(36).substring(7);
    const entry: DbProfile = {
      id: finalId,
      name: profileFormName.trim(),
      engine: profileFormEngine,
      kind: profileFormKind,
      container: profileFormKind === 'docker' ? profileFormContainer : '',
      host: profileFormHost.trim() || '127.0.0.1',
      port: profileFormPort.trim() || defaultPort(profileFormEngine),
      user: profileFormUser.trim() || (profileFormEngine === 'postgres' ? 'postgres' : 'root'),
      password: profileFormPassword,
    };

    if (isEditingProfile) {
      profiles = profiles.map((p) => (p.id === profileFormId ? entry : p));
    } else {
      profiles = [...profiles, entry];
      if (profiles.length === 1 || !activeProfileId) activeProfileId = finalId;
    }
    saveProfiles();
    applyActiveProfile();
    profileFormId = '';
    profileFormName = '';
    isEditingProfile = false;
  }

  function deleteProfile(id: string) {
    const p = profiles.find((x) => x.id === id);
    if (!p) return;
    if (!confirm(`Delete profile "${p.name}"?`)) return;
    profiles = profiles.filter((x) => x.id !== id);
    saveProfiles();
    if (activeProfileId === id) {
      activeProfileId = profiles.length > 0 ? profiles[0].id : '';
    }
    applyActiveProfile();
  }

  function handleProfileChange() {
    applyActiveProfile();
  }

  // ---------------------------------------------------------------------------
  // Auto-detection from a container's environment.
  // ---------------------------------------------------------------------------
  async function autoDetect() {
    if (!profileFormContainer) return;
    detecting = true;
    try {
      const out = await invoke<string>('exec_custom_command', {
        cmd: `docker inspect ${shQuote(profileFormContainer)} --format '{{json .Config}}' 2>/dev/null`,
        useSudo: false,
      });
      const cfg = JSON.parse(out);
      const env: Record<string, string> = {};
      for (const e of (cfg.Env as string[]) || []) {
        const i = e.indexOf('=');
        if (i > 0) env[e.slice(0, i)] = e.slice(i + 1);
      }
      const image = String(cfg.Image || '').toLowerCase();

      const isPg =
        image.includes('postgres') ||
        env.POSTGRES_USER !== undefined ||
        env.POSTGRES_PASSWORD !== undefined ||
        env.POSTGRES_DB !== undefined;
      const isMy =
        image.includes('mysql') ||
        image.includes('mariadb') ||
        env.MYSQL_ROOT_PASSWORD !== undefined ||
        env.MARIADB_ROOT_PASSWORD !== undefined ||
        env.MYSQL_USER !== undefined ||
        env.MARIADB_USER !== undefined;

      if (!isPg && !isMy) {
        notifications.warning("No database settings found in the container environment.");
        return;
      }

      if (isPg) {
        profileFormEngine = 'postgres';
        profileFormPort = '5432';
        profileFormUser = env.POSTGRES_USER || 'postgres';
        profileFormPassword = env.POSTGRES_PASSWORD || '';
      } else {
        profileFormEngine = 'mysql';
        profileFormPort = '3306';
        const rootPw = env.MYSQL_ROOT_PASSWORD || env.MARIADB_ROOT_PASSWORD;
        if (rootPw !== undefined) {
          profileFormUser = 'root';
          profileFormPassword = rootPw;
        } else {
          profileFormUser = env.MYSQL_USER || env.MARIADB_USER || 'root';
          profileFormPassword = env.MYSQL_PASSWORD || env.MARIADB_PASSWORD || '';
        }
      }
      profileFormHost = '127.0.0.1';
      if (!profileFormName.trim()) profileFormName = profileFormContainer;

      notifications.success(
        `Detected ${isPg ? 'PostgreSQL' : 'MySQL'} settings from the container.`,
      );
    } catch (err) {
      notifications.error(`Could not auto-detect settings: ${formatInvokeError(err)}`);
    } finally {
      detecting = false;
    }
  }

  // ---------------------------------------------------------------------------
  // Connection lifecycle (sqlx over an SSH tunnel)
  // ---------------------------------------------------------------------------
  async function doConnect(database: string | null): Promise<string> {
    const res = await invoke<{ connectionId: string }>('db_connect', {
      engine,
      host,
      port,
      user,
      password,
      database,
      container: useDocker && container ? container : null,
    });
    return res.connectionId;
  }

  async function connect() {
    connecting = true;
    errorMsg = '';
    try {
      const initialDb = engine === 'postgres' ? null : null;
      connectionId = await doConnect(initialDb);
      connected = true;
      pgDatabase = engine === 'postgres' ? 'postgres' : '';
      databases = await invoke<string[]>('db_list_databases', { connectionId });
      if (databases.length) await selectDatabase(databases[0]);
    } catch (err) {
      errorMsg = `Connection failed: ${formatInvokeError(err)}`;
      connected = false;
      connectionId = '';
    } finally {
      connecting = false;
    }
  }

  async function disconnect() {
    if (connectionId) await invoke('db_disconnect', { connectionId }).catch(() => {});
    connectionId = '';
    connected = false;
    databases = [];
    selectedDb = '';
    tables = [];
    selectedTable = '';
    structure = null;
    dataColumns = [];
    dataRows = [];
    total = 0;
    sqlResult = null;
    cancelEdit();
  }

  async function selectDatabase(db: string) {
    selectedDb = db;
    selectedTable = '';
    structure = null;
    dataColumns = [];
    dataRows = [];
    total = 0;
    cancelEdit();
    try {
      // Postgres is bound to a single database; switching requires a reconnect.
      if (engine === 'postgres' && db !== pgDatabase) {
        if (connectionId) await invoke('db_disconnect', { connectionId }).catch(() => {});
        connectionId = await doConnect(db);
        pgDatabase = db;
      }
      tables = await invoke<TableInfo[]>('db_list_tables', { connectionId, database: db });
    } catch (err) {
      errorMsg = formatInvokeError(err);
      tables = [];
    }
  }

  async function openTable(t: TableInfo) {
    selectedTable = t.name;
    selectedKind = t.kind;
    view = 'data';
    page = 0;
    orderBy = '';
    orderDir = 'asc';
    filters = [];
    cancelEdit();
    await loadStructure();
    await loadData();
  }

  async function loadStructure() {
    try {
      structure = await invoke<TableStructure>('db_table_structure', {
        connectionId,
        database: selectedDb,
        table: selectedTable,
      });
    } catch (err) {
      structure = null;
      errorMsg = formatInvokeError(err);
    }
  }

  async function loadData() {
    if (!selectedTable) return;
    running = true;
    cancelEdit();
    try {
      const res = await invoke<SelectResult>('db_select', {
        connectionId,
        database: engine === 'mysql' ? selectedDb : null,
        table: selectedTable,
        filters: filters.filter((f) => f.column),
        orderBy: orderBy || null,
        orderDir,
        limit: pageSize,
        offset: page * pageSize,
      });
      dataColumns = res.columns;
      dataRows = res.rows;
      total = res.total;
      selectedRows = new Set();
    } catch (err) {
      errorMsg = `Query error: ${formatInvokeError(err)}`;
    } finally {
      running = false;
    }
  }

  function toggleSort(col: string) {
    if (orderBy === col) {
      orderDir = orderDir === 'asc' ? 'desc' : 'asc';
    } else {
      orderBy = col;
      orderDir = 'asc';
    }
    page = 0;
    loadData();
  }

  function gotoPage(p: number) {
    page = Math.max(0, Math.min(pages - 1, p));
    loadData();
  }

  function changePageSize(n: number) {
    pageSize = n;
    page = 0;
    loadData();
  }

  // --- Filters ---------------------------------------------------------------
  function addFilter() {
    filters = [...filters, { column: dataColumns[0] || '', op: '=', value: '' }];
  }
  function removeFilter(i: number) {
    filters = filters.filter((_, idx) => idx !== i);
  }
  function applyFilters() {
    page = 0;
    loadData();
  }
  function clearFilters() {
    filters = [];
    page = 0;
    loadData();
  }

  // --- Inline editing --------------------------------------------------------
  function startEdit(i: number) {
    editRowIndex = i;
    editValues = [...dataRows[i]];
    inserting = false;
  }
  function cancelEdit() {
    editRowIndex = null;
    editValues = [];
    inserting = false;
    insertValues = [];
  }
  function setEditNull(c: number) {
    editValues[c] = null;
    editValues = [...editValues];
  }

  function pkCellsFor(rowIndex: number) {
    return whereCols.map((col) => ({
      column: col,
      value: dataRows[rowIndex][dataColumns.indexOf(col)] ?? null,
    }));
  }

  async function saveEdit() {
    if (editRowIndex === null) return;
    const values = dataColumns.map((col, c) => ({ column: col, value: editValues[c] }));
    const pk = pkCellsFor(editRowIndex);
    try {
      await invoke('db_update_row', {
        connectionId,
        database: engine === 'mysql' ? selectedDb : null,
        table: selectedTable,
        values,
        pk,
      });
      notifications.success("Row updated");
      cancelEdit();
      await loadData();
    } catch (err) {
      notifications.error(`Query error: ${formatInvokeError(err)}`);
    }
  }

  function startInsert() {
    inserting = true;
    editRowIndex = null;
    insertValues = dataColumns.map(() => null);
  }
  function setInsertNull(c: number) {
    insertValues[c] = null;
    insertValues = [...insertValues];
  }
  async function saveInsert() {
    // Omit cells left as NULL so column defaults / auto-increment apply.
    const values = dataColumns
      .map((col, c) => ({ column: col, value: insertValues[c] }))
      .filter((cell) => cell.value !== null && cell.value !== undefined);
    if (values.length === 0) {
      notifications.warning("Enter at least one value");
      return;
    }
    try {
      await invoke('db_insert_row', {
        connectionId,
        database: engine === 'mysql' ? selectedDb : null,
        table: selectedTable,
        values,
      });
      notifications.success("Row inserted");
      cancelEdit();
      await loadData();
    } catch (err) {
      notifications.error(`Query error: ${formatInvokeError(err)}`);
    }
  }

  function toggleRowSelect(i: number) {
    const s = new Set(selectedRows);
    if (s.has(i)) s.delete(i);
    else s.add(i);
    selectedRows = s;
  }
  function toggleSelectAll() {
    if (selectedRows.size === dataRows.length) selectedRows = new Set();
    else selectedRows = new Set(dataRows.map((_, i) => i));
  }

  async function deleteSelected() {
    if (selectedRows.size === 0) return;
    if (!confirm(`Delete ${selectedRows.size} row(s)?`)) return;
    const rows = [...selectedRows].map((i) => pkCellsFor(i));
    try {
      await invoke('db_delete_rows', {
        connectionId,
        database: engine === 'mysql' ? selectedDb : null,
        table: selectedTable,
        rows,
      });
      notifications.success(`${rows.length} row(s) deleted`);
      await loadData();
    } catch (err) {
      notifications.error(`Query error: ${formatInvokeError(err)}`);
    }
  }

  // --- SQL editor ------------------------------------------------------------
  async function runSql() {
    if (!sqlText.trim()) return;
    sqlRunning = true;
    errorMsg = '';
    try {
      sqlResult = await invoke<QueryResult>('db_query', {
        connectionId,
        database: engine === 'mysql' ? selectedDb || null : null,
        sql: sqlText,
      });
    } catch (err) {
      errorMsg = `Query error: ${formatInvokeError(err)}`;
    } finally {
      sqlRunning = false;
    }
  }

  // --- Export ----------------------------------------------------------------
  function download(name: string, content: string, mime: string) {
    const blob = new Blob([content], { type: mime });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = name;
    a.click();
    URL.revokeObjectURL(url);
  }
  function exportCsv(cols: string[], rows: (string | null)[][], base: string) {
    const esc = (v: string | null) => `"${(v ?? '').replace(/"/g, '""')}"`;
    const lines = [cols.map((c) => esc(c)).join(',')];
    for (const row of rows) lines.push(row.map(esc).join(','));
    download(`${base}.csv`, lines.join('\n'), 'text/csv');
  }
  function exportJson(cols: string[], rows: (string | null)[][], base: string) {
    const objs = rows.map((row) => Object.fromEntries(cols.map((c, i) => [c, row[i]])));
    download(`${base}.json`, JSON.stringify(objs, null, 2), 'application/json');
  }

  const sourceLabel = $derived(useDocker ? container : "On the host");

  onMount(async () => {
    await loadContainers();
    if (profileId) loadProfiles();
  });

  onDestroy(() => {
    if (connectionId) invoke('db_disconnect', { connectionId }).catch(() => {});
  });

  $effect(() => {
    if (profileId) untrack(() => loadProfiles());
  });
</script>

<div class="database-manager manager-shell fade-in">
  <header class="manager-header">
    <h1 class="page-title">Database browser</h1>
    <div class="header-actions">
      {#if profiles.length > 0}
        <div class="profile-selector glass">
          <span class="ps-label">Profile:</span>
          <select bind:value={activeProfileId} class="profile-select" onchange={handleProfileChange}>
            {#each profiles as p}
              <option value={p.id}>{p.name} ({engineLabel(p.engine)} · {p.kind === 'host' ? "On the host" : p.container})</option>
            {/each}
          </select>
          <button class="icon-btn-compact" onclick={() => (showProfilesModal = true)} title="Manage profiles">
            <Settings2 size={14} />
          </button>
        </div>
      {/if}
    </div>
  </header>

  {#if profiles.length === 0}
    <!-- SETUP VIEW -->
    <div class="setup-container fade-in">
      <div class="setup-card glass">
        <div class="setup-header">
          <Database size={32} class="accent" />
          <h2>Database Browser Setup</h2>
          <p>Create a connection profile to browse databases on the host or inside a Docker container.</p>
        </div>
        {@render profileForm()}
        <div class="setup-actions">
          <button class="primary" onclick={saveProfileForm} disabled={!profileFormName.trim()}>
            <Save size={14} /> Save profile
          </button>
        </div>
      </div>
    </div>
  {:else}
    <!-- CONNECTION BAR -->
    <div class="conn-bar glass">
      <span class="engine-badge">
        {#if useDocker}<Container size={13} />{:else}<HardDrive size={13} />{/if}
        {engineLabel(engine)}
      </span>
      <span class="conn-detail mono">{user}@{sourceLabel}:{port}</span>
      <div class="conn-spacer"></div>
      {#if connected}
        <button class="secondary btn-compact" onclick={disconnect}>
          <PlugZap size={14} /> Disconnect
        </button>
      {:else}
        <button class="primary btn-compact" disabled={connecting} onclick={connect}>
          <Plug size={14} /> {connecting ? "Connecting…" : "Connect"}
        </button>
      {/if}
    </div>

    {#if connected}
      <div class="db-workspace">
        <aside class="db-sidebar glass">
          <div class="sb-section">
            <div class="sb-title"><Server size={13} /> Databases</div>
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
              <div class="sb-title"><Table size={13} /> Tables ({tables.length})</div>
              <div class="sb-list">
                {#each tables as t}
                  <button class="sb-item" class:active={selectedTable === t.name} onclick={() => openTable(t)}>
                    {#if t.kind === 'view'}<Eye size={11} />{:else}<ChevronRight size={11} />{/if} {t.name}
                  </button>
                {/each}
                {#if tables.length === 0}<span class="sb-empty">No tables</span>{/if}
              </div>
            </div>
          {/if}
        </aside>

        <main class="db-main">
          <div class="main-tabs">
            <button class="mtab" class:active={view === 'data'} disabled={!selectedTable} onclick={() => (view = 'data')}><Table size={13} /> Data</button>
            <button class="mtab" class:active={view === 'structure'} disabled={!selectedTable} onclick={() => (view = 'structure')}><Columns3 size={13} /> Structure</button>
            <button class="mtab" class:active={view === 'sql'} onclick={() => (view = 'sql')}><Terminal size={13} /> SQL editor</button>
          </div>

          <!-- ============================ DATA ============================ -->
          {#if view === 'data'}
            {#if !selectedTable}
              <div class="result-empty"><Database size={22} /> Pick a table to browse, or run a query in the SQL editor.</div>
            {:else}
              <div class="data-toolbar">
                <button class="secondary btn-compact" onclick={loadData} title="Refresh"><RefreshCw size={13} class={running ? 'spin' : ''} /></button>
                <button class="secondary btn-compact" onclick={startInsert} disabled={selectedKind === 'view'}><Plus size={13} /> Add row</button>
                <button class="secondary btn-compact danger" onclick={deleteSelected} disabled={selectedRows.size === 0 || selectedKind === 'view'}><Trash2 size={13} /> Delete {selectedRows.size > 0 ? `(${selectedRows.size})` : ''}</button>
                <div class="mtab-spacer"></div>
                <button class="secondary btn-compact" onclick={() => exportCsv(dataColumns, dataRows, selectedTable)}><Download size={13} /> CSV</button>
                <button class="secondary btn-compact" onclick={() => exportJson(dataColumns, dataRows, selectedTable)}><Download size={13} /> JSON</button>
              </div>

              <!-- Filters -->
              <div class="filter-bar">
                <FilterIcon size={13} class="filter-icon" />
                {#each filters as f, i}
                  <div class="filter-row">
                    <select bind:value={f.column} class="flt-col">
                      {#each dataColumns as c}<option value={c}>{c}</option>{/each}
                    </select>
                    <select bind:value={f.op} class="flt-op">
                      {#each FILTER_OPS as op}<option value={op}>{op}</option>{/each}
                    </select>
                    <input class="flt-val mono" bind:value={f.value} placeholder="value" />
                    <button class="icon-btn-compact" onclick={() => removeFilter(i)}><X size={12} /></button>
                  </div>
                {/each}
                <button class="secondary btn-compact" onclick={addFilter}><Plus size={12} /> Filter</button>
                {#if filters.length > 0}
                  <button class="primary btn-compact" onclick={applyFilters}><Check size={12} /> Apply</button>
                  <button class="secondary btn-compact" onclick={clearFilters}>Clear</button>
                {/if}
              </div>

              <div class="result-area glass">
                {#if running}
                  <div class="result-empty">Loading…</div>
                {:else}
                  <div class="grid-scroll">
                    <table>
                      <thead>
                        <tr>
                          <th class="sel-col"><input type="checkbox" checked={selectedRows.size === dataRows.length && dataRows.length > 0} onchange={toggleSelectAll} /></th>
                          <th class="act-col"></th>
                          {#each dataColumns as col}
                            <th class="sortable" onclick={() => toggleSort(col)}>
                              {col}{#if orderBy === col}<span class="sort-ind">{orderDir === 'asc' ? '▲' : '▼'}</span>{/if}
                            </th>
                          {/each}
                        </tr>
                      </thead>
                      <tbody>
                        {#if inserting}
                          <tr class="edit-row">
                            <td class="sel-col"></td>
                            <td class="act-col">
                              <button class="row-btn ok" onclick={saveInsert} title="Save"><Check size={13} /></button>
                              <button class="row-btn" onclick={cancelEdit} title="Cancel"><X size={13} /></button>
                            </td>
                            {#each dataColumns as _col, c}
                              <td>
                                <div class="cell-edit">
                                  <input class="cell-input mono" value={insertValues[c] ?? ''} oninput={(e) => (insertValues[c] = e.currentTarget.value)} placeholder={insertValues[c] === null ? 'NULL' : ''} />
                                  <button class="null-btn" class:on={insertValues[c] === null} onclick={() => setInsertNull(c)} title="Set NULL">∅</button>
                                </div>
                              </td>
                            {/each}
                          </tr>
                        {/if}
                        {#each dataRows as row, i}
                          {#if editRowIndex === i}
                            <tr class="edit-row">
                              <td class="sel-col"></td>
                              <td class="act-col">
                                <button class="row-btn ok" onclick={saveEdit} title="Save"><Check size={13} /></button>
                                <button class="row-btn" onclick={cancelEdit} title="Cancel"><X size={13} /></button>
                              </td>
                              {#each dataColumns as _col, c}
                                <td>
                                  <div class="cell-edit">
                                    <input class="cell-input mono" value={editValues[c] ?? ''} oninput={(e) => (editValues[c] = e.currentTarget.value)} placeholder={editValues[c] === null ? 'NULL' : ''} />
                                    <button class="null-btn" class:on={editValues[c] === null} onclick={() => setEditNull(c)} title="Set NULL">∅</button>
                                  </div>
                                </td>
                              {/each}
                            </tr>
                          {:else}
                            <tr class:selected={selectedRows.has(i)}>
                              <td class="sel-col"><input type="checkbox" checked={selectedRows.has(i)} onchange={() => toggleRowSelect(i)} /></td>
                              <td class="act-col">
                                <button class="row-btn" onclick={() => startEdit(i)} disabled={selectedKind === 'view'} title="Edit"><Pencil size={12} /></button>
                              </td>
                              {#each row as cell}
                                <td title={cell ?? 'NULL'}>{#if cell === null}<span class="null-cell">NULL</span>{:else}{cell}{/if}</td>
                              {/each}
                            </tr>
                          {/if}
                        {/each}
                      </tbody>
                    </table>
                    {#if dataRows.length === 0 && !inserting}<div class="result-empty">No rows</div>{/if}
                  </div>
                  <div class="pager">
                    <span class="pager-info">{`${total} row(s)`}</span>
                    {#if structure && structure.primaryKey.length === 0}<span class="pk-warn">No primary key — row edits match on all columns</span>{/if}
                    <div class="mtab-spacer"></div>
                    <select class="page-size" value={pageSize} onchange={(e) => changePageSize(Number(e.currentTarget.value))}>
                      {#each [10, 25, 50, 100, 200] as n}<option value={n}>{n}</option>{/each}
                    </select>
                    <button class="icon-btn-compact" disabled={page <= 0} onclick={() => gotoPage(page - 1)}><ChevronLeft size={14} /></button>
                    <span class="pager-info">{page + 1} / {pages}</span>
                    <button class="icon-btn-compact" disabled={page >= pages - 1} onclick={() => gotoPage(page + 1)}><ChevronRight size={14} /></button>
                  </div>
                {/if}
              </div>
            {/if}

          <!-- ========================== STRUCTURE ========================== -->
          {:else if view === 'structure'}
            {#if structure}
              <div class="result-area glass struct-area">
                <h4 class="struct-h">Columns</h4>
                <div class="grid-scroll struct-grid">
                  <table>
                    <thead><tr><th>Name</th><th>Type</th><th>Null</th><th>Default</th><th>Key</th><th>Extra</th></tr></thead>
                    <tbody>
                      {#each structure.columns as c}
                        <tr>
                          <td class="mono">{c.name}</td>
                          <td class="mono">{c.dataType}</td>
                          <td>{c.nullable ? 'YES' : 'NO'}</td>
                          <td class="mono">{c.default ?? ''}</td>
                          <td>{c.key}</td>
                          <td>{c.extra}</td>
                        </tr>
                      {/each}
                    </tbody>
                  </table>
                </div>

                {#if structure.indexes.length}
                  <h4 class="struct-h">Indexes</h4>
                  <div class="grid-scroll struct-grid">
                    <table>
                      <thead><tr><th>Name</th><th>Columns</th><th>Unique</th><th>Primary</th></tr></thead>
                      <tbody>
                        {#each structure.indexes as ix}
                          <tr><td class="mono">{ix.name}</td><td class="mono">{ix.columns.join(', ')}</td><td>{ix.unique ? '✓' : ''}</td><td>{ix.primary ? '✓' : ''}</td></tr>
                        {/each}
                      </tbody>
                    </table>
                  </div>
                {/if}

                {#if structure.foreignKeys.length}
                  <h4 class="struct-h">Foreign keys</h4>
                  <div class="grid-scroll struct-grid">
                    <table>
                      <thead><tr><th>Name</th><th>Name</th><th>References</th></tr></thead>
                      <tbody>
                        {#each structure.foreignKeys as fk}
                          <tr><td class="mono">{fk.name}</td><td class="mono">{fk.column}</td><td class="mono">{fk.refTable}.{fk.refColumn}</td></tr>
                        {/each}
                      </tbody>
                    </table>
                  </div>
                {/if}
              </div>
            {:else}
              <div class="result-empty">Pick a table to browse, or run a query in the SQL editor.</div>
            {/if}

          <!-- ============================= SQL ============================= -->
          {:else}
            <div class="sql-box">
              <textarea class="sql-area" bind:value={sqlText} placeholder={`SELECT * FROM ...`} spellcheck="false"></textarea>
              <div class="sql-actions">
                <button class="primary btn-compact" disabled={sqlRunning} onclick={runSql}><Play size={14} /> {sqlRunning ? "Running…" : "Run"}</button>
                {#if sqlResult && sqlResult.columns.length}
                  <button class="secondary btn-compact" onclick={() => exportCsv(sqlResult!.columns, sqlResult!.rows, 'query')}><Download size={13} /> CSV</button>
                  <button class="secondary btn-compact" onclick={() => exportJson(sqlResult!.columns, sqlResult!.rows, 'query')}><Download size={13} /> JSON</button>
                {/if}
              </div>
            </div>
            <div class="result-area glass">
              {#if sqlRunning}
                <div class="result-empty">Loading…</div>
              {:else if !sqlResult}
                <div class="result-empty"><Terminal size={22} /> Pick a table to browse, or run a query in the SQL editor.</div>
              {:else if sqlResult.columns.length === 0}
                <div class="result-msg">{sqlResult.message}</div>
              {:else}
                <div class="result-meta">{`${sqlResult.rows.length} row(s)`}</div>
                <div class="grid-scroll">
                  <table>
                    <thead><tr>{#each sqlResult.columns as col}<th>{col}</th>{/each}</tr></thead>
                    <tbody>
                      {#each sqlResult.rows as row}
                        <tr>{#each row as cell}<td title={cell ?? 'NULL'}>{#if cell === null}<span class="null-cell">NULL</span>{:else}{cell}{/if}</td>{/each}</tr>
                      {/each}
                    </tbody>
                  </table>
                </div>
              {/if}
            </div>
          {/if}
        </main>
      </div>
    {:else}
      <div class="empty glass"><Database size={26} /> Select a profile and connect to a database server.</div>
    {/if}
  {/if}
</div>

<!-- ===================== Profiles Manager Modal ===================== -->
{#if showProfilesModal}
  <div class="modal-overlay" role="presentation" onclick={() => (showProfilesModal = false)}>
    <div class="modal-content glass profiles-modal" role="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()}>
      <div class="modal-header">
        <h3>Manage profiles</h3>
        <button class="icon-btn-compact" onclick={() => (showProfilesModal = false)}><X size={16} /></button>
      </div>

      <div class="modal-body flex-col gap-md">
        <div class="profiles-list-section">
          <h4>Profiles</h4>
          <div class="profiles-grid">
            {#each profiles as p}
              <div class="profile-item glass" class:active={p.id === activeProfileId}>
                <div class="profile-details">
                  <span class="profile-title">{p.name}</span>
                  <span class="profile-subtitle mono">{engineLabel(p.engine)} · {p.kind === 'host' ? "On the host" : p.container}</span>
                </div>
                <div class="profile-actions">
                  <button class="row-btn" onclick={() => openEditProfile(p)} title="Edit"><FileCode size={13} /></button>
                  <button class="row-btn danger" onclick={() => deleteProfile(p.id)} title="Delete"><Trash2 size={13} /></button>
                </div>
              </div>
            {/each}
          </div>
          <button class="secondary btn-compact mt-sm" onclick={openAddProfile}>
            <Plus size={14} /> Add profile
          </button>
        </div>
      </div>

      {#if profileFormId && (!profiles.some((p) => p.id === profileFormId) || isEditingProfile)}
        <div class="profile-form-section glass mt-md p-md">
          <h4>{isEditingProfile ? "Edit profile" : "Add profile"}</h4>
          {@render profileForm()}
          <div class="modal-actions mt-md">
            <button class="secondary" onclick={() => { isEditingProfile = false; profileFormId = ''; }}>Cancel</button>
            <button class="primary" onclick={saveProfileForm} disabled={!profileFormName.trim()}><Save size={14} /> Save</button>
          </div>
        </div>
      {/if}
    </div>
  </div>
{/if}

<!-- ===================== Reusable profile form ===================== -->
{#snippet profileForm()}
  <div class="form-group mt-sm">
    <label for="db-form-name">Profile name</label>
    <input id="db-form-name" type="text" bind:value={profileFormName} placeholder="e.g. App database, Postgres prod" />
  </div>

  <div class="form-group">
    <label>Engine</label>
    <div class="seg">
      <button type="button" class="seg-btn" class:active={profileFormEngine === 'mysql'} onclick={() => onFormEngineChange('mysql')}>MySQL</button>
      <button type="button" class="seg-btn" class:active={profileFormEngine === 'postgres'} onclick={() => onFormEngineChange('postgres')}>PostgreSQL</button>
    </div>
  </div>

  <div class="form-group">
    <label>Source</label>
    <div class="target-options">
      <label class="target-radio" class:active={profileFormKind === 'host'}>
        <input type="radio" name="db-kind" value="host" bind:group={profileFormKind} />
        <HardDrive size={14} /> <span>On the host</span>
      </label>
      <label class="target-radio" class:active={profileFormKind === 'docker'}>
        <input type="radio" name="db-kind" value="docker" bind:group={profileFormKind} />
        <Container size={14} /> <span>In a Docker container</span>
      </label>
    </div>
  </div>

  {#if profileFormKind === 'docker'}
    <div class="form-group fade-in">
      <label for="db-form-container">Container</label>
      <div class="select-row">
        <select id="db-form-container" bind:value={profileFormContainer}>
          {#if containers.length === 0}<option value="">No containers</option>{/if}
          {#each containers as c}
            <option value={c}>{c}</option>
          {/each}
        </select>
        <button class="secondary btn-compact" onclick={loadContainers} disabled={loadingContainers} title="Refresh">
          <RefreshCw size={14} class={loadingContainers ? 'spin' : ''} />
        </button>
        <button class="secondary btn-compact" onclick={autoDetect} disabled={detecting || !profileFormContainer}>
          <Wand2 size={14} /> {detecting ? "Detecting…" : "Auto-detect"}
        </button>
      </div>
      <span class="field-hint">Pick a database container and auto-detect reads its engine and credentials from the container environment.</span>
    </div>
  {/if}

  <div class="form-row">
    <div class="form-group grow">
      <label for="db-form-host">Host</label>
      <input id="db-form-host" class="mono" type="text" bind:value={profileFormHost} placeholder="127.0.0.1" />
    </div>
    <div class="form-group port-col">
      <label for="db-form-port">Port</label>
      <input id="db-form-port" class="mono" type="text" bind:value={profileFormPort} placeholder={defaultPort(profileFormEngine)} />
    </div>
  </div>

  <div class="form-row">
    <div class="form-group grow">
      <label for="db-form-user">User</label>
      <input id="db-form-user" class="mono" type="text" bind:value={profileFormUser} />
    </div>
    <div class="form-group grow">
      <label for="db-form-pass">Password</label>
      <input id="db-form-pass" type="password" bind:value={profileFormPassword} autocomplete="off" />
    </div>
  </div>
{/snippet}

<style>
  .header-actions { display: flex; gap: 6px; flex-wrap: wrap; }
  .profile-selector { display: flex; align-items: center; gap: 8px; padding: 4px 10px; border-radius: var(--radius-sm); border: 1px solid var(--border-color); }
  .ps-label { font-size: 0.72rem; color: var(--text-muted); font-weight: 500; text-transform: uppercase; letter-spacing: 0.05em; }
  .profile-select { background: transparent; border: none; color: var(--accent-amber); font-size: 0.8rem; font-weight: 600; cursor: pointer; outline: none; padding-right: 4px; }
  .profile-select option { background: var(--bg-primary); color: var(--text-primary); }

  .conn-bar { display: flex; gap: 10px; align-items: center; padding: 10px; border-radius: var(--radius-md); flex-shrink: 0; flex-wrap: wrap; }
  .engine-badge { display: inline-flex; align-items: center; gap: 6px; padding: 5px 10px; font-size: 0.76rem; font-weight: 600; color: var(--accent-amber); background: var(--bg-active); border: 1px solid rgba(245,158,11,0.25); border-radius: var(--radius-sm); white-space: nowrap; }
  .conn-detail { font-family: var(--font-mono); font-size: 0.8rem; color: var(--text-muted); }
  .conn-spacer { flex: 1; }

  .empty { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; color: var(--text-muted); border-radius: var(--radius-md); }
  .db-workspace { flex: 1; display: flex; gap: 10px; overflow: hidden; }
  .db-sidebar { width: 230px; flex-shrink: 0; overflow-y: auto; padding: 10px; border-radius: var(--radius-md); display: flex; flex-direction: column; gap: 14px; }
  .sb-title { display: flex; align-items: center; gap: 6px; font-size: 0.7rem; text-transform: uppercase; letter-spacing: 0.05em; color: var(--text-muted); margin-bottom: 6px; }
  .sb-list { display: flex; flex-direction: column; gap: 2px; }
  .sb-empty { font-size: 0.74rem; color: var(--text-muted); padding: 4px 8px; }
  .sb-item { display: flex; align-items: center; gap: 6px; background: transparent; border: none; color: var(--text-secondary); padding: 5px 8px; border-radius: var(--radius-sm); cursor: pointer; font-size: 0.78rem; text-align: left; width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: var(--font-mono); }
  .sb-item:hover { background: var(--bg-hover); color: var(--text-primary); }
  .sb-item.active { background: var(--bg-active); color: var(--accent-amber); }
  .db-main { flex: 1; display: flex; flex-direction: column; gap: 10px; overflow: hidden; }
  .main-tabs { display: flex; gap: 6px; align-items: center; flex-shrink: 0; }
  .mtab { display: flex; align-items: center; gap: 6px; background: transparent; border: 1px solid var(--border-color); color: var(--text-secondary); padding: 6px 12px; font-size: 0.78rem; border-radius: var(--radius-sm); cursor: pointer; }
  .mtab.active { background: var(--bg-active); color: var(--accent-amber); border-color: rgba(245,158,11,0.25); }
  .mtab:disabled { opacity: 0.4; cursor: not-allowed; }
  .mtab-spacer { flex: 1; }

  .data-toolbar { display: flex; gap: 6px; align-items: center; flex-shrink: 0; }
  .filter-bar { display: flex; gap: 6px; align-items: center; flex-wrap: wrap; flex-shrink: 0; }
  :global(.filter-bar .filter-icon) { color: var(--text-muted); }
  .filter-row { display: flex; gap: 4px; align-items: center; }
  .flt-col, .flt-op, .flt-val { background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--radius-sm); color: var(--text-primary); padding: 4px 6px; font-size: 0.76rem; }
  .flt-val { width: 140px; }

  .sql-box { display: flex; flex-direction: column; gap: 8px; flex-shrink: 0; }
  .sql-area { min-height: 120px; background: var(--bg-primary); border: 1px solid var(--border-color); border-radius: var(--radius-sm); padding: 10px; color: var(--text-primary); font-family: var(--font-mono); font-size: 0.8rem; resize: vertical; }
  .sql-actions { display: flex; gap: 6px; }
  .result-area { flex: 1; overflow: hidden; border-radius: var(--radius-md); display: flex; flex-direction: column; }
  .result-empty { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; color: var(--text-muted); padding: 30px; }
  .result-msg { padding: 16px; color: var(--accent-green); font-family: var(--font-mono); font-size: 0.85rem; }
  .result-meta { padding: 8px 12px; font-size: 0.72rem; color: var(--text-muted); border-bottom: 1px solid var(--border-color); flex-shrink: 0; }
  .grid-scroll { flex: 1; overflow: auto; }
  table { width: 100%; border-collapse: collapse; font-size: 0.76rem; }
  th, td { padding: 6px 10px; text-align: left; border-bottom: 1px solid var(--border-color); border-right: 1px solid var(--border-color); white-space: nowrap; max-width: 320px; overflow: hidden; text-overflow: ellipsis; }
  th { color: var(--text-muted); font-weight: 600; position: sticky; top: 0; background: var(--bg-secondary); font-family: var(--font-mono); }
  td { color: var(--text-secondary); font-family: var(--font-mono); }
  th.sortable { cursor: pointer; user-select: none; }
  th.sortable:hover { color: var(--accent-amber); }
  .sort-ind { margin-left: 4px; font-size: 0.6rem; }
  .sel-col, .act-col { width: 1%; white-space: nowrap; }
  .act-col { min-width: 56px; }
  tr.selected td { background: rgba(245,158,11,0.08); }
  tr.edit-row td { background: var(--bg-active); }
  .null-cell { color: var(--text-muted); font-style: italic; opacity: 0.6; }
  .cell-edit { display: flex; gap: 2px; align-items: center; }
  .cell-input { background: var(--bg-primary); border: 1px solid var(--border-color); border-radius: 3px; color: var(--text-primary); padding: 3px 5px; font-size: 0.74rem; width: 160px; }
  .null-btn { background: transparent; border: 1px solid var(--border-color); color: var(--text-muted); border-radius: 3px; cursor: pointer; padding: 2px 5px; font-size: 0.7rem; }
  .null-btn.on { color: var(--accent-amber); border-color: rgba(245,158,11,0.4); }
  .row-btn { background: transparent; border: 1px solid var(--border-color); color: var(--text-secondary); border-radius: var(--radius-sm); padding: 4px 6px; cursor: pointer; }
  .row-btn:hover { background: var(--bg-hover); color: var(--text-primary); }
  .row-btn.ok:hover { color: var(--accent-green); }
  .row-btn:disabled { opacity: 0.3; cursor: not-allowed; }

  .pager { display: flex; gap: 8px; align-items: center; padding: 8px 10px; border-top: 1px solid var(--border-color); flex-shrink: 0; }
  .pager-info { font-size: 0.74rem; color: var(--text-muted); }
  .pk-warn { font-size: 0.7rem; color: var(--accent-red); }
  .page-size { background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--radius-sm); color: var(--text-primary); padding: 3px 6px; font-size: 0.74rem; }

  .struct-area { overflow-y: auto; padding: 12px; gap: 8px; }
  .struct-h { font-size: 0.78rem; text-transform: uppercase; letter-spacing: 0.05em; color: var(--text-muted); margin: 8px 0 4px; }
  .struct-grid { max-height: none; border: 1px solid var(--border-color); border-radius: var(--radius-sm); }

  .setup-container { display: flex; align-items: center; justify-content: center; padding: 40px 20px; min-height: 70vh; }
  .setup-card { width: 480px; max-width: 94vw; padding: 30px; border-radius: var(--radius-lg); display: flex; flex-direction: column; gap: 16px; box-shadow: var(--shadow-md); }
  .setup-header { text-align: center; display: flex; flex-direction: column; align-items: center; gap: 10px; margin-bottom: 10px; }
  .setup-header h2 { font-size: 1.4rem; font-weight: 700; color: var(--text-primary); }
  .setup-header p { font-size: 0.82rem; color: var(--text-muted); line-height: 1.5; }
  :global(.setup-header .accent) { color: var(--accent-amber); }
  .setup-actions { margin-top: 10px; display: flex; justify-content: flex-end; }

  .seg { display: flex; gap: 6px; }
  .seg-btn { flex: 1; background: transparent; border: 1px solid var(--border-color); color: var(--text-secondary); padding: 8px 10px; font-size: 0.78rem; border-radius: var(--radius-sm); cursor: pointer; transition: all 0.15s ease; }
  .seg-btn:hover { background: var(--bg-hover); }
  .seg-btn.active { background: var(--bg-active); color: var(--accent-amber); border-color: rgba(245,158,11,0.35); font-weight: 600; }
  .target-options { display: flex; gap: 12px; margin-top: 4px; }
  .target-radio { flex: 1; display: flex; align-items: center; justify-content: center; gap: 8px; padding: 10px; border: 1px solid var(--border-color); border-radius: var(--radius-sm); cursor: pointer; font-size: 0.82rem; color: var(--text-secondary); transition: all 0.2s ease; }
  .target-radio:hover { background: var(--bg-hover); }
  .target-radio.active { border-color: rgba(245,158,11,0.35); background: var(--bg-active); color: var(--accent-amber); }
  .target-radio input { margin: 0; }
  .select-row { display: flex; gap: 8px; }
  .select-row select { flex: 1; }
  .field-hint { font-size: 0.7rem; color: var(--text-muted); margin-top: 2px; }
  .form-row { display: flex; gap: 12px; }
  .form-group.grow { flex: 1; }
  .form-group.port-col { width: 110px; flex-shrink: 0; }

  .modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.6); display: flex; align-items: center; justify-content: center; z-index: 1000; backdrop-filter: blur(4px); }
  .modal-content { background: var(--bg-primary); border: 1px solid var(--border-color); border-radius: var(--radius-lg); padding: 20px; box-shadow: var(--shadow-lg); display: flex; flex-direction: column; }
  .modal-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px; }
  .modal-actions { display: flex; justify-content: flex-end; gap: 8px; }
  .profiles-modal { width: 500px; max-width: 94vw; max-height: 90vh; overflow-y: auto; }
  .profiles-list-section h4, .profile-form-section h4 { font-size: 0.85rem; text-transform: uppercase; letter-spacing: 0.05em; color: var(--text-muted); margin-bottom: 8px; }
  .profiles-grid { display: flex; flex-direction: column; gap: 8px; max-height: 200px; overflow-y: auto; padding-right: 4px; }
  .profile-item { display: flex; justify-content: space-between; align-items: center; padding: 8px 12px; border-radius: var(--radius-sm); border: 1px solid var(--border-color); }
  .profile-item.active { border-color: rgba(245,158,11,0.25); background: var(--bg-active); }
  .profile-details { display: flex; flex-direction: column; min-width: 0; }
  .profile-title { font-weight: 600; font-size: 0.85rem; color: var(--text-primary); }
  .profile-subtitle { font-size: 0.7rem; color: var(--text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .profile-actions { display: flex; gap: 6px; flex-shrink: 0; }
  .profile-form-section { border-radius: var(--radius-md); border: 1px solid var(--border-color); }
  .row-btn.danger:hover { color: var(--accent-red); border-color: rgba(239,68,68,0.3); }
  .btn-compact.danger:hover { color: var(--accent-red); }

  .form-group { display: flex; flex-direction: column; gap: 5px; margin-bottom: 12px; }
  .form-group label { font-size: 0.78rem; color: var(--text-secondary); }
  .form-group input, .form-group select { background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--radius-sm); padding: 8px 10px; color: var(--text-primary); font-size: 0.85rem; }
  .mono { font-family: var(--font-mono); }
  .flex-col { display: flex; flex-direction: column; }
  .gap-md { gap: 16px; }
  .mt-sm { margin-top: 8px; }
  .mt-md { margin-top: 16px; }
  .p-md { padding: 16px; }
</style>
