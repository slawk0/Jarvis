<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { Database, Table, Play, Plug, PlugZap, Download, Server, ChevronRight, Terminal, Settings2, Save, Plus, Trash2, X, FileCode, RefreshCw, Container, HardDrive, Wand2 } from 'lucide-svelte';
  import { LL } from '$lib/i18n/i18n-svelte';
  import { get } from 'svelte/store';
  import { shQuote, listContainers } from '$lib/exec/target';
  import { notifications } from '$lib/notifications.svelte';
  import { formatInvokeError } from '$lib/i18n/backendErrors';

  let { profileId = '', visible = true } = $props();

  export function refresh() {
    loadContainers();
    if (profileId) loadProfiles();
  }

  interface QueryResult {
    columns: string[];
    rows: string[][];
    message: string | null;
  }

  type Engine = 'mysql' | 'postgres';

  // A saved connection profile. Stored per server profile in localStorage.
  // Passwords are stored here as well so a saved profile can reconnect without
  // re-entering them; for Docker containers the credentials are anyway
  // recoverable from `docker inspect`.
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

  function defaultPort(e: Engine) {
    return e === 'mysql' ? '3306' : '5432';
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

  $effect(() => {
    if (errorMsg) {
      notifications.error(errorMsg);
      errorMsg = '';
    }
  });

  function quoteIdent(name: string): string {
    return engine === 'mysql' ? `\`${name.replace(/`/g, '``')}\`` : `"${name.replace(/"/g, '""')}"`;
  }

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
    // Reset browsing state when switching profiles.
    connected = false;
    databases = [];
    selectedDb = '';
    tables = [];
    selectedTable = '';
    result = null;
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
    profileFormContainer = p.kind === 'docker' ? p.container : (containers[0] || '');
    profileFormHost = p.host;
    profileFormPort = p.port;
    profileFormUser = p.user;
    profileFormPassword = p.password;
  }

  function onFormEngineChange(e: Engine) {
    profileFormEngine = e;
    // Move the port to the engine default unless the user typed a custom one.
    if (profileFormPort === '3306' || profileFormPort === '5432' || !profileFormPort) {
      profileFormPort = defaultPort(e);
    }
    // Postgres' canonical superuser is "postgres", MySQL's is "root".
    if (profileFormUser === 'root' || profileFormUser === 'postgres' || !profileFormUser) {
      profileFormUser = e === 'postgres' ? 'postgres' : 'root';
    }
  }

  function saveProfileForm() {
    if (!profileFormName.trim()) {
      notifications.error(get(LL).database.nameRequired());
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
    if (!confirm(get(LL).database.deleteProfileConfirm({ name: p.name }))) return;
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
  // Auto-detection: read engine + credentials from a container's environment.
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
        notifications.warning(get(LL).database.detectNothing());
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
        // Prefer the root account so all databases are visible.
        const rootPw = env.MYSQL_ROOT_PASSWORD || env.MARIADB_ROOT_PASSWORD;
        if (rootPw !== undefined) {
          profileFormUser = 'root';
          profileFormPassword = rootPw;
        } else {
          profileFormUser = env.MYSQL_USER || env.MARIADB_USER || 'root';
          profileFormPassword = env.MYSQL_PASSWORD || env.MARIADB_PASSWORD || '';
        }
      }
      // The client runs inside the container via `docker exec`, so localhost.
      profileFormHost = '127.0.0.1';
      if (!profileFormName.trim()) profileFormName = profileFormContainer;

      notifications.success(
        get(LL).database.detectSuccess({ engine: isPg ? 'PostgreSQL' : 'MySQL' }),
      );
    } catch (err) {
      notifications.error(get(LL).database.detectFailed({ error: formatInvokeError(err) }));
    } finally {
      detecting = false;
    }
  }

  // ---------------------------------------------------------------------------
  // Querying
  // ---------------------------------------------------------------------------
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
    } catch (err) {
      errorMsg = get(LL).database.connectFailed({ error: formatInvokeError(err) });
      connected = false;
    } finally {
      connecting = false;
    }
  }

  function disconnect() {
    connected = false;
    databases = [];
    selectedDb = '';
    tables = [];
    selectedTable = '';
    result = null;
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

  const sourceLabel = $derived(
    useDocker ? container : get(LL).database.sourceHost(),
  );

  onMount(async () => {
    await loadContainers();
    if (profileId) loadProfiles();
  });

  $effect(() => {
    if (profileId) untrack(() => loadProfiles());
  });
</script>

<div class="database-manager manager-shell fade-in">
  <header class="manager-header">
    <h1 class="page-title">{$LL.database.title()}</h1>
    <div class="header-actions">
      {#if profiles.length > 0}
        <div class="profile-selector glass">
          <span class="ps-label">{$LL.database.activeProfile()}:</span>
          <select bind:value={activeProfileId} class="profile-select" onchange={handleProfileChange}>
            {#each profiles as p}
              <option value={p.id}>{p.name} ({p.engine === 'postgres' ? 'PostgreSQL' : 'MySQL'} · {p.kind === 'host' ? $LL.database.sourceHost() : p.container})</option>
            {/each}
          </select>
          <button class="icon-btn-compact" onclick={() => (showProfilesModal = true)} title={$LL.database.manageProfiles()}>
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
          <h2>{$LL.database.setupTitle()}</h2>
          <p>{$LL.database.setupDesc()}</p>
        </div>

        {@render profileForm()}

        <div class="setup-actions">
          <button class="primary" onclick={saveProfileForm} disabled={!profileFormName.trim()}>
            <Save size={14} /> {$LL.database.saveProfile()}
          </button>
        </div>
      </div>
    </div>
  {:else}
    <!-- CONNECTION BAR -->
    <div class="conn-bar glass">
      <span class="engine-badge">
        {#if useDocker}<Container size={13} />{:else}<HardDrive size={13} />{/if}
        {engine === 'postgres' ? 'PostgreSQL' : 'MySQL'}
      </span>
      <span class="conn-detail mono">{user}@{sourceLabel}:{port}</span>
      <div class="conn-spacer"></div>
      {#if connected}
        <button class="secondary btn-compact" onclick={disconnect}>
          <PlugZap size={14} /> {$LL.database.disconnect()}
        </button>
      {:else}
        <button class="primary btn-compact" disabled={connecting} onclick={connect}>
          <Plug size={14} /> {connecting ? $LL.database.connecting() : $LL.database.connect()}
        </button>
      {/if}
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
  {/if}
</div>

<!-- ===================== Profiles Manager Modal ===================== -->
{#if showProfilesModal}
  <div class="modal-overlay" role="presentation" onclick={() => (showProfilesModal = false)}>
    <div class="modal-content glass profiles-modal" role="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()}>
      <div class="modal-header">
        <h3>{$LL.database.manageProfiles()}</h3>
        <button class="icon-btn-compact" onclick={() => (showProfilesModal = false)}><X size={16} /></button>
      </div>

      <div class="modal-body flex-col gap-md">
        <div class="profiles-list-section">
          <h4>{$LL.database.profilesTitle()}</h4>
          <div class="profiles-grid">
            {#each profiles as p}
              <div class="profile-item glass" class:active={p.id === activeProfileId}>
                <div class="profile-details">
                  <span class="profile-title">{p.name}</span>
                  <span class="profile-subtitle mono">{p.engine === 'postgres' ? 'PostgreSQL' : 'MySQL'} · {p.kind === 'host' ? $LL.database.sourceHost() : p.container}</span>
                </div>
                <div class="profile-actions">
                  <button class="row-btn" onclick={() => openEditProfile(p)} title={$LL.common.edit()}><FileCode size={13} /></button>
                  <button class="row-btn danger" onclick={() => deleteProfile(p.id)} title={$LL.common.delete()}><Trash2 size={13} /></button>
                </div>
              </div>
            {/each}
          </div>
          <button class="secondary btn-compact mt-sm" onclick={openAddProfile}>
            <Plus size={14} /> {$LL.database.addProfile()}
          </button>
        </div>
      </div>

      {#if profileFormId && (!profiles.some((p) => p.id === profileFormId) || isEditingProfile)}
        <div class="profile-form-section glass mt-md p-md">
          <h4>{isEditingProfile ? $LL.database.editProfile() : $LL.database.addProfile()}</h4>
          {@render profileForm()}
          <div class="modal-actions mt-md">
            <button class="secondary" onclick={() => { isEditingProfile = false; profileFormId = ''; }}>{$LL.common.cancel()}</button>
            <button class="primary" onclick={saveProfileForm} disabled={!profileFormName.trim()}><Save size={14} /> {$LL.common.save()}</button>
          </div>
        </div>
      {/if}
    </div>
  </div>
{/if}

<!-- ===================== Reusable profile form ===================== -->
{#snippet profileForm()}
  <div class="form-group mt-sm">
    <label for="db-form-name">{$LL.database.profileName()}</label>
    <input id="db-form-name" type="text" bind:value={profileFormName} placeholder={$LL.database.profileNamePlaceholder()} />
  </div>

  <div class="form-group">
    <label>{$LL.database.engine()}</label>
    <div class="seg">
      <button type="button" class="seg-btn" class:active={profileFormEngine === 'mysql'} onclick={() => onFormEngineChange('mysql')}>MySQL</button>
      <button type="button" class="seg-btn" class:active={profileFormEngine === 'postgres'} onclick={() => onFormEngineChange('postgres')}>PostgreSQL</button>
    </div>
  </div>

  <div class="form-group">
    <label>{$LL.database.source()}</label>
    <div class="target-options">
      <label class="target-radio" class:active={profileFormKind === 'host'}>
        <input type="radio" name="db-kind" value="host" bind:group={profileFormKind} />
        <HardDrive size={14} /> <span>{$LL.database.sourceHost()}</span>
      </label>
      <label class="target-radio" class:active={profileFormKind === 'docker'}>
        <input type="radio" name="db-kind" value="docker" bind:group={profileFormKind} />
        <Container size={14} /> <span>{$LL.database.sourceDocker()}</span>
      </label>
    </div>
  </div>

  {#if profileFormKind === 'docker'}
    <div class="form-group fade-in">
      <label for="db-form-container">{$LL.database.selectContainer()}</label>
      <div class="select-row">
        <select id="db-form-container" bind:value={profileFormContainer}>
          {#if containers.length === 0}<option value="">{$LL.database.noContainers()}</option>{/if}
          {#each containers as c}
            <option value={c}>{c}</option>
          {/each}
        </select>
        <button class="secondary btn-compact" onclick={loadContainers} disabled={loadingContainers} title={$LL.common.refresh()}>
          <RefreshCw size={14} class={loadingContainers ? 'spin' : ''} />
        </button>
        <button class="secondary btn-compact" onclick={autoDetect} disabled={detecting || !profileFormContainer}>
          <Wand2 size={14} /> {detecting ? $LL.database.detecting() : $LL.database.autoDetect()}
        </button>
      </div>
      <span class="field-hint">{$LL.database.detectHint()}</span>
    </div>
  {/if}

  <div class="form-row">
    <div class="form-group grow">
      <label for="db-form-host">{$LL.database.host()}</label>
      <input id="db-form-host" class="mono" type="text" bind:value={profileFormHost} placeholder="127.0.0.1" />
    </div>
    <div class="form-group port-col">
      <label for="db-form-port">{$LL.database.port()}</label>
      <input id="db-form-port" class="mono" type="text" bind:value={profileFormPort} placeholder={defaultPort(profileFormEngine)} />
    </div>
  </div>

  <div class="form-row">
    <div class="form-group grow">
      <label for="db-form-user">{$LL.database.user()}</label>
      <input id="db-form-user" class="mono" type="text" bind:value={profileFormUser} />
    </div>
    <div class="form-group grow">
      <label for="db-form-pass">{$LL.database.password()}</label>
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

  /* Setup view */
  .setup-container { display: flex; align-items: center; justify-content: center; padding: 40px 20px; min-height: 70vh; }
  .setup-card { width: 480px; max-width: 94vw; padding: 30px; border-radius: var(--radius-lg); display: flex; flex-direction: column; gap: 16px; box-shadow: var(--shadow-md); }
  .setup-header { text-align: center; display: flex; flex-direction: column; align-items: center; gap: 10px; margin-bottom: 10px; }
  .setup-header h2 { font-size: 1.4rem; font-weight: 700; color: var(--text-primary); }
  .setup-header p { font-size: 0.82rem; color: var(--text-muted); line-height: 1.5; }
  :global(.setup-header .accent) { color: var(--accent-amber); }
  .setup-actions { margin-top: 10px; display: flex; justify-content: flex-end; }

  /* Form */
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

  /* Profiles modal */
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
  .row-btn { background: transparent; border: 1px solid var(--border-color); color: var(--text-secondary); border-radius: var(--radius-sm); padding: 6px 8px; cursor: pointer; }
  .row-btn:hover { background: var(--bg-hover); color: var(--text-primary); }
  .row-btn.danger:hover { color: var(--accent-red); border-color: rgba(239,68,68,0.3); }

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
