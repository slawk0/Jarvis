<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { BarChart3, Play, FileSearch, Settings2, Save, Plus, Trash2, X, FileCode, RefreshCw, Container, HardDrive } from 'lucide-svelte';
  import SudoModal from './SudoModal.svelte';
  import { LL } from '$lib/i18n/i18n-svelte';
  import { get } from 'svelte/store';
  import { shQuote, wrapCmd, isDocker, listContainers, type ExecTarget } from '$lib/exec/target';
  import { notifications } from '$lib/notifications.svelte';
  import { formatInvokeError, isSudoPasswordRequired } from '$lib/i18n/backendErrors';

  interface Row { count: number; value: string; }

  type ServerType = 'nginx' | 'apache' | 'httpd' | 'traefik';
  const SERVER_TYPES: ServerType[] = ['nginx', 'apache', 'httpd', 'traefik'];

  // Default on-host access log path for each web server.
  const DEFAULT_PATHS: Record<ServerType, string> = {
    nginx: '/var/log/nginx/access.log',
    apache: '/var/log/apache2/access.log',
    httpd: '/var/log/httpd/access_log',
    traefik: '/var/log/traefik/access.log',
  };

  interface LogProfile {
    id: string;
    name: string;
    serverType: ServerType;
    target: ExecTarget;
    // Access log path used when target.kind === 'host'. For docker targets we
    // read the container's stdout via `docker logs` instead.
    logPath: string;
  }

  let { profileId = '', visible = true } = $props();

  export function refresh() {
    loadContainers();
    if (profileId) loadProfiles();
  }

  const getProfilesKey = () => `jarvis-loganalysis-profiles-${profileId}`;
  const getActiveKey = () => `jarvis-loganalysis-active-${profileId}`;

  // ---------------------------------------------------------------------------
  // Profile state
  // ---------------------------------------------------------------------------
  let profiles = $state<LogProfile[]>([]);
  let activeProfileId = $state('');

  // Active analysis context (derived from the active profile, path is editable).
  let target = $state<ExecTarget>({ kind: 'host' });
  let serverType = $state<ServerType>('nginx');
  let logPath = $state(DEFAULT_PATHS.nginx);

  let showProfilesModal = $state(false);
  let isEditingProfile = $state(false);
  let profileFormId = $state('');
  let profileFormName = $state('');
  let profileFormServer = $state<ServerType>('nginx');
  let profileFormKind = $state<'host' | 'docker'>('host');
  let profileFormContainer = $state('');
  let profileFormLogPath = $state(DEFAULT_PATHS.nginx);
  // Tracks whether the user manually edited the path so we don't clobber it
  // when switching server type.
  let pathTouched = $state(false);

  let containers = $state<string[]>([]);
  let loadingContainers = $state(false);

  async function loadContainers() {
    loadingContainers = true;
    containers = await listContainers(true);
    if (containers.length > 0 && !profileFormContainer) profileFormContainer = containers[0];
    loadingContainers = false;
  }

  const activeProfile = $derived(profiles.find((p) => p.id === activeProfileId));

  function loadProfiles() {
    if (!profileId) return;
    const stored = localStorage.getItem(getProfilesKey());
    const storedActive = localStorage.getItem(getActiveKey());
    let needsSave = false;
    if (stored) {
      try {
        const parsed: LogProfile[] = JSON.parse(stored);
        profiles = parsed.map((p) => {
          if (!p.id) { p.id = Math.random().toString(36).substring(7); needsSave = true; }
          // Backfill fields for profiles saved by older versions.
          if (!p.serverType) { p.serverType = 'nginx'; needsSave = true; }
          if (!p.logPath) { p.logPath = DEFAULT_PATHS[p.serverType] || DEFAULT_PATHS.nginx; needsSave = true; }
          return p;
        });
      } catch { profiles = []; }
    } else { profiles = []; }

    if (storedActive && profiles.some((p) => p.id === storedActive)) {
      activeProfileId = storedActive;
    } else if (profiles.length > 0) {
      activeProfileId = profiles[0].id;
    } else {
      activeProfileId = '';
    }

    if (needsSave) saveProfiles();
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
      target = active.target;
      serverType = active.serverType;
      logPath = active.logPath || DEFAULT_PATHS[active.serverType];
      if (profileId) localStorage.setItem(getActiveKey(), activeProfileId);
    } else {
      target = { kind: 'host' };
      serverType = 'nginx';
      logPath = DEFAULT_PATHS.nginx;
    }
    // reset previous results when switching profiles
    hasRun = false;
  }

  function openAddProfile() {
    isEditingProfile = false;
    profileFormId = Math.random().toString(36).substring(7);
    profileFormName = '';
    profileFormServer = 'nginx';
    profileFormKind = 'host';
    profileFormContainer = containers[0] || '';
    profileFormLogPath = DEFAULT_PATHS.nginx;
    pathTouched = false;
  }

  function openEditProfile(p: LogProfile) {
    isEditingProfile = true;
    profileFormId = p.id;
    profileFormName = p.name;
    profileFormServer = p.serverType;
    profileFormKind = p.target.kind;
    profileFormContainer = p.target.kind === 'docker' ? p.target.container : (containers[0] || '');
    profileFormLogPath = p.logPath || DEFAULT_PATHS[p.serverType];
    pathTouched = true;
  }

  function onFormServerChange() {
    // Update the default path to match the chosen server unless the user edited it.
    if (!pathTouched) profileFormLogPath = DEFAULT_PATHS[profileFormServer];
  }

  function saveProfileForm() {
    if (!profileFormName.trim()) {
      notifications.error('Please enter a profile name');
      return;
    }
    const t: ExecTarget = profileFormKind === 'host'
      ? { kind: 'host' }
      : { kind: 'docker', container: profileFormContainer };
    const finalId = profileFormId || Math.random().toString(36).substring(7);
    const entry: LogProfile = {
      id: finalId,
      name: profileFormName.trim(),
      serverType: profileFormServer,
      target: t,
      logPath: (profileFormLogPath.trim() || DEFAULT_PATHS[profileFormServer]),
    };

    if (isEditingProfile) {
      profiles = profiles.map((p) => p.id === profileFormId ? entry : p);
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
    if (!confirm(get(LL).loganalysis.deleteProfileConfirm({ name: p.name }))) return;
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
  // Analysis state
  // ---------------------------------------------------------------------------
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
    if (profileId) untrack(() => { loadProfiles(); });
  });

  $effect(() => {
    if (errorMsg) { notifications.error(errorMsg); errorMsg = ''; }
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

  // Shared awk pipeline that turns a Common Log Format stream in shell var $T
  // into the @@@-delimited sections we parse on the frontend.
  function analyzePipeline(): string {
    return (
      `echo '@@@TOTAL'; printf '%s\\n' "$T" | wc -l; ` +
      `echo '@@@STATUS'; printf '%s\\n' "$T" | awk '{print $9}' | sort | uniq -c | sort -rn | head -20; ` +
      `echo '@@@IP'; printf '%s\\n' "$T" | awk '{print $1}' | sort | uniq -c | sort -rn | head -20; ` +
      `echo '@@@PATH'; printf '%s\\n' "$T" | awk '{print $7}' | sort | uniq -c | sort -rn | head -25; ` +
      `echo '@@@METHOD'; printf '%s\\n' "$T" | awk '{print $6}' | tr -d '\"' | sort | uniq -c | sort -rn | head -10; ` +
      `echo '@@@UA'; printf '%s\\n' "$T" | awk -F'\"' '{print $6}' | sort | uniq -c | sort -rn | head -15; ` +
      `echo '@@@HOUR'; printf '%s\\n' "$T" | awk '{print $4}' | awk -F: '{print $2}' | sort | uniq -c | sort -k2n`
    );
  }

  function buildCmd(): string {
    const n = Math.max(1000, Math.min(1000000, Number(lines) || 50000));

    if (target.kind === 'docker') {
      // Container web servers conventionally log to stdout/stderr — read it
      // straight from docker logs on the host (no exec inside the container).
      const containerQ = shQuote(target.container);
      return `T=$(docker logs --tail ${n} ${containerQ} 2>&1); ` + analyzePipeline();
    }

    const lp = shQuote(logPath);
    // test -c guards against access logs symlinked to a character device such as
    // /dev/stdout, which would make `tail` block forever.
    // test -e distinguishes "file not found" from "permission denied" so we don't
    // prompt for sudo when the server is simply not installed.
    return wrapCmd(
      target,
      `if test -c ${lp}; then echo "__DEVICE__"; ` +
        `elif ! test -e ${lp}; then echo "__NOTFOUND__"; ` +
        `elif ! test -r ${lp}; then echo "__NOACCESS__"; else ` +
        `T=$(tail -n ${n} ${lp}); ` + analyzePipeline() + `; fi`,
    );
  }

  async function analyze() {
    const run = async (useSudo: boolean) => {
      analyzing = true;
      errorMsg = '';
      try {
        const out = await invoke<string>('exec_custom_command', { cmd: buildCmd(), useSudo });
        if (out.includes('__DEVICE__')) {
          errorMsg = `${logPath} is a device file (likely a /dev/stdout symlink) and can't be tailed. For containerized servers use a Docker profile instead.`;
          return;
        }
        if (out.includes('__NOTFOUND__')) {
          errorMsg = `Log file not found: ${logPath}. ${serverType.charAt(0).toUpperCase() + serverType.slice(1)} may not be installed, or the log path is wrong.`;
          return;
        }
        if (out.includes('__NOACCESS__')) {
          if (useSudo) {
            errorMsg = `Cannot read ${logPath} even with elevated privileges.`;
            return;
          }
          pendingAction = () => run(true);
          showSudoModal = true;
          return;
        }
        parseOutput(out);
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

  function parseOutput(out: string) {
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
  }

  function statusClass(code: string): string {
    if (code.startsWith('2')) return 'ok';
    if (code.startsWith('3')) return 'redir';
    if (code.startsWith('4')) return 'warn';
    if (code.startsWith('5')) return 'err';
    return '';
  }

  onMount(async () => {
    await loadContainers();
    if (profileId) loadProfiles();
  });
</script>

<div class="log-analysis manager-shell fade-in">
  <header class="manager-header">
    <h1 class="page-title">{$LL.loganalysis.title()}</h1>
    <div class="header-actions">
      {#if profiles.length > 0}
        <div class="profile-selector glass">
          <span class="ps-label">{$LL.loganalysis.activeProfile()}:</span>
          <select bind:value={activeProfileId} class="profile-select" onchange={handleProfileChange}>
            {#each profiles as p}
              <option value={p.id}>{p.name} ({$LL.loganalysis.servers[p.serverType]()} · {p.target.kind === 'host' ? $LL.loganalysis.sourceHost() : p.target.container})</option>
            {/each}
          </select>
          <button class="icon-btn-compact" onclick={() => (showProfilesModal = true)} title={$LL.loganalysis.manageProfiles()}>
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
          <BarChart3 size={32} class="accent" />
          <h2>{$LL.loganalysis.setupTitle()}</h2>
          <p>{$LL.loganalysis.setupDesc()}</p>
        </div>

        <div class="form-group">
          <label for="setup-name">{$LL.loganalysis.profileName()}</label>
          <input id="setup-name" type="text" bind:value={profileFormName} placeholder={$LL.loganalysis.profileNamePlaceholder()} />
        </div>

        <div class="form-group">
          <label>{$LL.loganalysis.serverType()}</label>
          <div class="server-chips">
            {#each SERVER_TYPES as st}
              <button type="button" class="server-chip" class:active={profileFormServer === st} onclick={() => { profileFormServer = st; onFormServerChange(); }}>
                {$LL.loganalysis.servers[st]()}
              </button>
            {/each}
          </div>
        </div>

        <div class="form-group">
          <label>{$LL.loganalysis.source()}</label>
          <div class="target-options">
            <label class="target-radio" class:active={profileFormKind === 'host'}>
              <input type="radio" name="setup-kind" value="host" bind:group={profileFormKind} />
              <HardDrive size={14} /> <span>{$LL.loganalysis.sourceHost()}</span>
            </label>
            <label class="target-radio" class:active={profileFormKind === 'docker'}>
              <input type="radio" name="setup-kind" value="docker" bind:group={profileFormKind} />
              <Container size={14} /> <span>{$LL.loganalysis.sourceDocker()}</span>
            </label>
          </div>
        </div>

        {#if profileFormKind === 'docker'}
          <div class="form-group fade-in">
            <label for="setup-container">{$LL.loganalysis.selectContainer()}</label>
            <div class="select-row">
              <select id="setup-container" bind:value={profileFormContainer}>
                {#each containers as c}
                  <option value={c}>{c}</option>
                {/each}
              </select>
              <button class="secondary btn-compact" onclick={loadContainers} disabled={loadingContainers}>
                <RefreshCw size={14} class={loadingContainers ? 'spin' : ''} />
              </button>
            </div>
            <span class="field-hint">{$LL.loganalysis.dockerLogsNote()}</span>
          </div>
        {:else}
          <div class="form-group fade-in">
            <label for="setup-path">{$LL.loganalysis.logPath()}</label>
            <input id="setup-path" class="mono" type="text" bind:value={profileFormLogPath} oninput={() => (pathTouched = true)} placeholder={DEFAULT_PATHS[profileFormServer]} />
          </div>
        {/if}

        {#if profileFormServer === 'traefik'}
          <span class="field-hint warn-hint">{$LL.loganalysis.traefikHint()}</span>
        {/if}

        <div class="setup-actions">
          <button class="primary" onclick={saveProfileForm} disabled={!profileFormName.trim()}>
            <Save size={14} /> {$LL.loganalysis.saveProfile()}
          </button>
        </div>
      </div>
    </div>
  {:else}
    <!-- ANALYSIS VIEW -->
    <div class="control-bar glass">
      <span class="server-badge">
        {#if target.kind === 'docker'}<Container size={13} />{:else}<HardDrive size={13} />{/if}
        {$LL.loganalysis.servers[serverType]()}
      </span>
      {#if target.kind === 'docker'}
        <span class="source-display mono">docker logs {target.container}</span>
      {:else}
        <input class="path-input" type="text" bind:value={logPath} placeholder={DEFAULT_PATHS[serverType]} />
      {/if}
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
  {/if}
</div>

<!-- ===================== Profiles Manager Modal ===================== -->
{#if showProfilesModal}
  <div class="modal-overlay" role="presentation" onclick={() => (showProfilesModal = false)}>
    <div class="modal-content glass profiles-modal" role="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()}>
      <div class="modal-header">
        <h3>{$LL.loganalysis.manageProfiles()}</h3>
        <button class="icon-btn-compact" onclick={() => (showProfilesModal = false)}><X size={16} /></button>
      </div>

      <div class="modal-body flex-col gap-md">
        <div class="profiles-list-section">
          <h4>{$LL.loganalysis.profilesTitle()}</h4>
          <div class="profiles-grid">
            {#each profiles as p}
              <div class="profile-item glass" class:active={p.id === activeProfileId}>
                <div class="profile-details">
                  <span class="profile-title">{p.name}</span>
                  <span class="profile-subtitle mono">{$LL.loganalysis.servers[p.serverType]()} · {p.target.kind === 'host' ? $LL.loganalysis.sourceHost() : p.target.container}</span>
                </div>
                <div class="profile-actions">
                  <button class="row-btn" onclick={() => openEditProfile(p)} title={$LL.common.edit()}><FileCode size={13} /></button>
                  <button class="row-btn danger" onclick={() => deleteProfile(p.id)} title={$LL.common.delete()}><Trash2 size={13} /></button>
                </div>
              </div>
            {/each}
          </div>
          <button class="secondary btn-compact mt-sm" onclick={openAddProfile}>
            <Plus size={14} /> {$LL.loganalysis.addProfile()}
          </button>
        </div>
      </div>

      {#if showProfilesModal && (profileFormId && !profiles.some(p => p.id === profileFormId) || isEditingProfile)}
        <div class="profile-form-section glass mt-md p-md">
          <h4>{isEditingProfile ? $LL.loganalysis.editProfile() : $LL.loganalysis.addProfile()}</h4>

          <div class="form-group mt-sm">
            <label for="form-name">{$LL.loganalysis.profileName()}</label>
            <input id="form-name" type="text" bind:value={profileFormName} placeholder={$LL.loganalysis.profileNamePlaceholder()} />
          </div>

          <div class="form-group">
            <label>{$LL.loganalysis.serverType()}</label>
            <div class="server-chips">
              {#each SERVER_TYPES as st}
                <button type="button" class="server-chip" class:active={profileFormServer === st} onclick={() => { profileFormServer = st; onFormServerChange(); }}>
                  {$LL.loganalysis.servers[st]()}
                </button>
              {/each}
            </div>
          </div>

          <div class="form-group">
            <label>{$LL.loganalysis.source()}</label>
            <div class="target-options">
              <label class="target-radio" class:active={profileFormKind === 'host'}>
                <input type="radio" name="form-kind" value="host" bind:group={profileFormKind} />
                <HardDrive size={14} /> <span>{$LL.loganalysis.sourceHost()}</span>
              </label>
              <label class="target-radio" class:active={profileFormKind === 'docker'}>
                <input type="radio" name="form-kind" value="docker" bind:group={profileFormKind} />
                <Container size={14} /> <span>{$LL.loganalysis.sourceDocker()}</span>
              </label>
            </div>
          </div>

          {#if profileFormKind === 'docker'}
            <div class="form-group">
              <label for="form-container">{$LL.loganalysis.selectContainer()}</label>
              <div class="select-row">
                <select id="form-container" bind:value={profileFormContainer}>
                  {#each containers as c}
                    <option value={c}>{c}</option>
                  {/each}
                </select>
                <button class="secondary btn-compact" onclick={loadContainers} disabled={loadingContainers}>
                  <RefreshCw size={14} class={loadingContainers ? 'spin' : ''} />
                </button>
              </div>
              <span class="field-hint">{$LL.loganalysis.dockerLogsNote()}</span>
            </div>
          {:else}
            <div class="form-group">
              <label for="form-path">{$LL.loganalysis.logPath()}</label>
              <input id="form-path" class="mono" type="text" bind:value={profileFormLogPath} oninput={() => (pathTouched = true)} placeholder={DEFAULT_PATHS[profileFormServer]} />
            </div>
          {/if}

          <div class="modal-actions mt-md">
            <button class="secondary" onclick={() => { isEditingProfile = false; profileFormId = ''; }}>{$LL.common.cancel()}</button>
            <button class="primary" onclick={saveProfileForm} disabled={!profileFormName.trim()}><Save size={14} /> {$LL.common.save()}</button>
          </div>
        </div>
      {/if}
    </div>
  </div>
{/if}

<SudoModal bind:open={showSudoModal} onSuccess={() => { const a = pendingAction; pendingAction = null; if (a) a(); }} onCancel={() => (pendingAction = null)} />

<style>
  .header-actions { display: flex; gap: 6px; flex-wrap: wrap; }
  .profile-selector { display: flex; align-items: center; gap: 8px; padding: 4px 10px; border-radius: var(--radius-sm); border: 1px solid var(--border-color); }
  .ps-label { font-size: 0.72rem; color: var(--text-muted); font-weight: 500; text-transform: uppercase; letter-spacing: 0.05em; }
  .profile-select { background: transparent; border: none; color: var(--accent-amber); font-size: 0.8rem; font-weight: 600; cursor: pointer; outline: none; padding-right: 4px; }
  .profile-select option { background: var(--bg-primary); color: var(--text-primary); }

  .control-bar { display: flex; gap: 10px; align-items: center; padding: 10px; border-radius: var(--radius-md); flex-shrink: 0; flex-wrap: wrap; }
  .server-badge { display: inline-flex; align-items: center; gap: 6px; padding: 5px 10px; font-size: 0.76rem; font-weight: 600; color: var(--accent-amber); background: var(--bg-active); border: 1px solid rgba(245,158,11,0.25); border-radius: var(--radius-sm); white-space: nowrap; }
  .source-display { flex: 1; min-width: 200px; font-family: var(--font-mono); font-size: 0.8rem; color: var(--text-muted); padding: 7px 0; }
  .path-input { flex: 1; min-width: 200px; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--radius-sm); padding: 7px 10px; color: var(--text-primary); font-size: 0.82rem; font-family: var(--font-mono); }
  .lines-label { display: flex; align-items: center; gap: 6px; font-size: 0.74rem; color: var(--text-muted); }
  .lines-input { width: 100px; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--radius-sm); padding: 7px 8px; color: var(--text-primary); font-size: 0.8rem; }
  .placeholder { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; color: var(--text-muted); border-radius: var(--radius-md); }
  .results { flex: 1; overflow: auto; display: flex; flex-direction: column; gap: 12px; }
  .stat-row { display: flex; gap: 12px; flex-wrap: wrap; }
  .stat-card { padding: 14px 20px; border-radius: var(--radius-md); display: flex; flex-direction: column; gap: 4px; min-width: 140px; }
  .stat-num { font-size: 1.6rem; font-weight: 700; color: var(--accent-amber); font-variant-numeric: tabular-nums; user-select: text; cursor: text; }
  .stat-label { font-size: 0.72rem; color: var(--text-muted); text-transform: uppercase; letter-spacing: 0.05em; }
  .status-chips { flex: 1; display: flex; flex-wrap: wrap; gap: 8px; padding: 12px; border-radius: var(--radius-md); align-content: flex-start; }
  .status-chip { display: flex; flex-direction: column; align-items: center; padding: 6px 12px; border-radius: var(--radius-sm); border: 1px solid var(--border-color); min-width: 64px; user-select: text; cursor: text; }
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
  .bar-label { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-secondary); font-family: var(--font-mono); user-select: text; cursor: text; }
  .bar-track { height: 8px; background: var(--bg-primary); border-radius: 4px; overflow: hidden; pointer-events: none; }
  .bar-fill { height: 100%; background: var(--accent-primary); border-radius: 4px; }
  .bar-count { font-variant-numeric: tabular-nums; color: var(--text-muted); text-align: right; user-select: text; cursor: text; }
  .empty { color: var(--text-muted); font-size: 0.76rem; }

  /* Setup view */
  .setup-container { display: flex; align-items: center; justify-content: center; padding: 40px 20px; min-height: 70vh; }
  .setup-card { width: 480px; max-width: 94vw; padding: 30px; border-radius: var(--radius-lg); display: flex; flex-direction: column; gap: 16px; box-shadow: var(--shadow-md); }
  .setup-header { text-align: center; display: flex; flex-direction: column; align-items: center; gap: 10px; margin-bottom: 10px; }
  .setup-header h2 { font-size: 1.4rem; font-weight: 700; color: var(--text-primary); }
  .setup-header p { font-size: 0.82rem; color: var(--text-muted); line-height: 1.5; }
  :global(.setup-header .accent) { color: var(--accent-amber); }
  .server-chips { display: flex; gap: 6px; flex-wrap: wrap; }
  .server-chip { flex: 1; min-width: 70px; background: transparent; border: 1px solid var(--border-color); color: var(--text-secondary); padding: 8px 10px; font-size: 0.78rem; border-radius: var(--radius-sm); cursor: pointer; transition: all 0.15s ease; }
  .server-chip:hover { background: var(--bg-hover); }
  .server-chip.active { background: var(--bg-active); color: var(--accent-amber); border-color: rgba(245,158,11,0.35); font-weight: 600; }
  .target-options { display: flex; gap: 12px; margin-top: 4px; }
  .target-radio { flex: 1; display: flex; align-items: center; justify-content: center; gap: 8px; padding: 10px; border: 1px solid var(--border-color); border-radius: var(--radius-sm); cursor: pointer; font-size: 0.82rem; color: var(--text-secondary); transition: all 0.2s ease; }
  .target-radio:hover { background: var(--bg-hover); }
  .target-radio.active { border-color: rgba(245,158,11,0.35); background: var(--bg-active); color: var(--accent-amber); }
  .target-radio input { margin: 0; }
  .select-row { display: flex; gap: 8px; }
  .select-row select { flex: 1; }
  .field-hint { font-size: 0.7rem; color: var(--text-muted); margin-top: 2px; }
  .warn-hint { color: var(--accent-amber); }
  .setup-actions { margin-top: 10px; display: flex; justify-content: flex-end; }

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
