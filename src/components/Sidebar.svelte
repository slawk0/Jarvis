<script lang="ts">
  import { 
    LayoutDashboard, 
    FolderClosed, 
    Settings, 
    Calendar, 
    Users, 
    Shield, 
    ShieldAlert, 
    FileText, 
    Terminal,
    Box,
    Globe,
    LogOut,
    Wrench,
    Database,
    Network,
    BookOpen,
    ChevronDown,
    Loader2,
    Server,
    PanelLeftClose,
    PanelLeftOpen,
    HardDrive,
    Globe2,
    Cpu,
    Table,
    KeyRound,
    Radar,
    Timer,
    BarChart3,
    FolderArchive,
    Search,
    X,
  } from 'lucide-svelte';
  import type { ServerProfile } from '$lib/admin/types';
  import { getNavLabel, TAB_IDS, NAV_CATEGORIES } from '$lib/nav';
  import { getVersion } from '@tauri-apps/api/app';
  import { onMount } from 'svelte';

  let appVersion = $state('...');

  onMount(async () => {
    try {
      appVersion = await getVersion();
    } catch (e) {
      appVersion = 'dev';
    }
  });

  let {
    activeTab = '',
    collapsed = $bindable(false),
    onDisconnect,
    hostname = '',
    onTabSelect = (_tab: string) => {},
    profiles = [] as ServerProfile[],
    currentProfileId = '',
    onSwitchProfile = (_id: string) => {},
    isSwitching = false,
    onCustomDragStart,
    isOnline = true,
    isReconnecting = false,
    onReconnect = () => {},
  } = $props();

  let showProfileMenu = $state(false);
  let searchQuery = $state('');

  const filteredCategories = $derived.by(() => {
    const query = searchQuery.toLowerCase().trim();
    if (!query) {
      return NAV_CATEGORIES;
    }
    return NAV_CATEGORIES.map(cat => {
      const matchedItems = cat.items.filter(id => {
        const label = getNavLabel(id).toLowerCase();
        return label.includes(query) || id.toLowerCase().includes(query);
      });
      return { ...cat, items: matchedItems };
    }).filter(cat => cat.items.length > 0);
  });

  const menuItems = TAB_IDS.map((id) => ({
    id,
    icon: {
      dashboard: LayoutDashboard,
      maintenance: Wrench,
      backups: Database,
      restic: FolderArchive,
      network: Network,
      runbooks: BookOpen,
      files: FolderClosed,
      services: Settings,
      docker: Box,
      cron: Calendar,
      users: Users,
      firewall: Shield,
      crowdsec: ShieldAlert,
      pangolin: Globe,
      logs: FileText,
      loganalysis: BarChart3,
      terminal: Terminal,
      disks: HardDrive,
      webserver: Globe2,
      processes: Cpu,
      database: Table,
      envvars: KeyRound,
      netdiag: Radar,
      timers: Timer,
    }[id],
  }));

  const displayHostname = $derived(hostname || 'Remote Server');

  function toggleProfileMenu() {
    if (profiles.length <= 1) return;
    showProfileMenu = !showProfileMenu;
  }

  function selectProfile(id: string) {
    showProfileMenu = false;
    if (id !== currentProfileId) onSwitchProfile(id);
  }

  function toggleCollapse() {
    collapsed = !collapsed;
  }

  function handlePointerDown(e: PointerEvent, tabId: string) {
    if (onCustomDragStart) {
      onCustomDragStart(e, 'tab', tabId);
    }
  }

  const currentProfile = $derived(profiles.find((p) => p.id === currentProfileId));
</script>

<svelte:window onclick={() => (showProfileMenu = false)} />

<aside class="sidebar glass" class:collapsed>
  <div class="brand">
    <button class="logo-circle" onclick={toggleCollapse} title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}>J</button>
    {#if !collapsed}
      <div class="search-container">
        <div class="search-icon">
          <Search size={14} />
        </div>
        <input 
          type="text" 
          placeholder="Search..." 
          bind:value={searchQuery}
          class="sidebar-search"
          aria-label="Search navigation"
        />
        {#if searchQuery}
          <button class="clear-search" onclick={() => searchQuery = ''} aria-label="Clear search">
            <X size={14} />
          </button>
        {/if}
      </div>
    {/if}
  </div>

  <nav class="nav-menu">
    {#if filteredCategories.length === 0}
      <div class="no-results">No matches found</div>
    {:else}
      {#each filteredCategories as cat, catIndex}
        {#if !collapsed}
          <div class="category-header">{cat.label}</div>
        {:else if catIndex > 0}
          <div class="category-divider"></div>
        {/if}
        {#each cat.items as itemId}
          {@const item = menuItems.find(i => i.id === itemId)}
          {#if item}
            {@const label = getNavLabel(item.id)}
            <div 
              class="nav-item {activeTab === item.id ? 'active' : ''}" 
              onclick={() => onTabSelect(item.id)}
              onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onTabSelect(item.id); } }}
              onpointerdown={(e: PointerEvent) => handlePointerDown(e, item.id)}
              role="button"
              tabindex="0"
              title={collapsed ? label : ''}
            >
              <item.icon size={18} class="nav-icon" />
              {#if !collapsed}
                <span class="nav-label">{label}</span>
              {/if}
            </div>
          {/if}
        {/each}
      {/each}
    {/if}
  </nav>

  <div class="sidebar-footer">
    {#if showProfileMenu && profiles.length > 1 && !collapsed}
      <div class="profile-dropdown" onclick={(e) => e.stopPropagation()}>
        {#each profiles as p}
          <button
            class="profile-option {p.id === currentProfileId ? 'active' : ''}"
            onclick={() => selectProfile(p.id)}
          >
            <Server size={14} />
            <div class="opt-info">
              <span class="opt-label">{p.label}</span>
              <span class="opt-host">{p.username}@{p.host}</span>
            </div>
          </button>
        {/each}
      </div>
    {/if}

    {#if !collapsed}
      <div class="telemetry-hud">
        <div 
          class="hud-row"
          class:clickable={profiles.length > 1}
          onclick={(e) => { if (profiles.length > 1) { e.stopPropagation(); toggleProfileMenu(); } }}
          onkeydown={(e) => { if (profiles.length > 1 && (e.key === 'Enter' || e.key === ' ')) { e.preventDefault(); e.stopPropagation(); toggleProfileMenu(); } }}
          role={profiles.length > 1 ? "button" : undefined}
          tabindex={profiles.length > 1 ? 0 : -1}
          title={profiles.length > 1 ? 'Switch server profile' : undefined}
        >
          <span class="hud-label">Host</span>
          <span class="hud-val" class:has-dropdown={profiles.length > 1} title={displayHostname}>
            {currentProfile?.label || displayHostname}
            {#if profiles.length > 1}
              <ChevronDown size={10} class="hud-chev" />
            {/if}
          </span>
        </div>
        <div class="hud-row">
          <span class="hud-label">Status</span>
          <span class="hud-status" class:nominal={isOnline} class:offline={!isOnline}>
            {isSwitching ? 'Switching' : (isOnline ? 'Online' : 'Offline')}
          </span>
        </div>
        {#if !isOnline}
          <div class="hud-row reconnect-row">
            <button
              type="button"
              class="reconnect-btn"
              onclick={(e) => { e.stopPropagation(); onReconnect(); }}
              disabled={isReconnecting}
            >
              {#if isReconnecting}
                <Loader2 size={10} class="spin" /> Reconnecting...
              {:else}
                Reconnect
              {/if}
            </button>
          </div>
        {/if}
        <div class="hud-row">
          <span class="hud-label">Connection</span>
          <span class="hud-val secure">SSH (enc)</span>
        </div>
      </div>
    {/if}

    <button class="nav-item collapse-toggle" onclick={toggleCollapse} title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}>
      {#if collapsed}
        <PanelLeftOpen size={16} class="nav-icon" />
      {:else}
        <PanelLeftClose size={16} class="nav-icon" />
        <span class="nav-label">Collapse</span>
      {/if}
    </button>

    <button class="nav-item logout" onclick={onDisconnect} title={collapsed ? 'Disconnect' : ''}>
      <LogOut size={16} class="nav-icon" />
      {#if !collapsed}
        <span class="nav-label">Disconnect</span>
      {/if}
    </button>

    <div class="version-display" title="Running version v{appVersion}">
      <span>v{appVersion}</span>
    </div>
  </div>
</aside>

<style>
  .sidebar {
    width: 240px;
    height: 100vh;
    display: flex;
    flex-direction: column;
    border-right: 1px solid var(--border-color);
    background: var(--bg-primary);
    flex-shrink: 0;
    position: relative;
    transition: width 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    overflow: hidden;
  }

  .sidebar.collapsed {
    width: 56px;
  }

  .brand {
    padding: 20px;
    display: flex;
    align-items: center;
    gap: 12px;
    border-bottom: 1px solid var(--border-color);
    min-height: 72px;
  }

  .sidebar.collapsed .brand {
    padding: 14px;
    justify-content: center;
  }

  .logo-circle {
    width: 30px;
    height: 30px;
    min-width: 30px;
    border-radius: var(--radius-sm);
    background: var(--accent-primary);
    color: var(--text-primary);
    font-weight: 800;
    font-size: 1.1rem;
    font-family: var(--font-display);
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: none;
    flex-shrink: 0;
    border: 1px solid var(--border-white);
    cursor: pointer;
    padding: 0;
    transition: transform 0.1s ease;
  }

  .logo-circle:hover {
    transform: scale(1.06);
  }

  .logo-circle:active {
    transform: scale(0.96);
  }

  .search-container {
    position: relative;
    flex: 1;
    display: flex;
    align-items: center;
  }

  .search-icon {
    position: absolute;
    left: 10px;
    height: 100%;
    display: flex;
    align-items: center;
    color: var(--text-muted);
    pointer-events: none;
  }

  .sidebar-search {
    width: 100%;
    height: 32px;
    padding: 6px 28px 6px 30px !important;
    font-size: 0.8rem;
    background: var(--bg-element) !important;
    border: 1px solid var(--border-color) !important;
    border-radius: var(--radius-sm);
    color: var(--text-primary);
  }

  .sidebar-search:focus {
    border-color: var(--accent-primary) !important;
    box-shadow: 0 0 0 2px var(--accent-muted) !important;
  }

  .clear-search {
    position: absolute;
    right: 6px;
    background: transparent;
    border: none;
    padding: 2px;
    color: var(--text-muted);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: color 0.15s ease, transform 0.1s ease;
  }

  .clear-search:hover {
    color: var(--text-primary);
  }

  .clear-search:active {
    transform: scale(0.96);
  }

  .category-header {
    font-size: 0.65rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
    padding: 14px 14px 4px;
    user-select: none;
  }

  .category-divider {
    height: 1px;
    background: rgba(255, 255, 255, 0.06);
    margin: 8px 12px;
  }

  .no-results {
    font-size: 0.8rem;
    color: var(--text-muted);
    padding: 20px 14px;
    text-align: center;
    font-style: italic;
  }

  .profile-dropdown {
    position: absolute;
    bottom: calc(100% + 4px);
    left: 10px;
    right: 10px;
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    z-index: 50;
    box-shadow: 0 -8px 24px rgba(0, 0, 0, 0.5);
    max-height: 200px;
    overflow-y: auto;
  }

  .profile-option {
    width: 100%;
    background: transparent;
    border: none;
    padding: 10px 12px;
    display: flex;
    align-items: center;
    gap: 10px;
    text-align: left;
    color: var(--text-secondary);
    border-bottom: 1px solid var(--border-color);
    border-radius: 0;
  }

  .profile-option:last-child { border-bottom: none; }
  .profile-option:hover { background: var(--bg-hover); color: white; }
  .profile-option.active { background: var(--bg-active); color: var(--accent-amber); }

  .opt-info { display: flex; flex-direction: column; min-width: 0; }
  .opt-label { font-size: 0.82rem; font-weight: 600; }
  .opt-host { font-size: 0.68rem; color: var(--text-muted); }

  .nav-menu {
    flex: 1;
    padding: 16px 10px;
    display: flex;
    flex-direction: column;
    gap: 3px;
    overflow-y: auto;
  }

  .sidebar.collapsed .nav-menu {
    padding: 10px 6px;
    align-items: center;
  }

  .nav-item {
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    padding: 10px 14px;
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    cursor: pointer;
    text-align: left;
    transition: background 0.1s ease, color 0.1s ease, border-color 0.1s ease;
    white-space: nowrap;
    -webkit-user-drag: element;
    user-select: none;
  }

  .sidebar.collapsed .nav-item {
    padding: 10px;
    justify-content: center;
    width: 40px;
    min-height: 40px;
  }

  .nav-item:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .nav-item.active {
    background: var(--accent-muted);
    color: var(--text-primary);
    border-left: 3px solid var(--accent-primary);
    border-top: none;
    border-right: none;
    border-bottom: none;
    border-top-left-radius: 0;
    border-bottom-left-radius: 0;
    font-weight: 600;
    padding-left: 11px;
  }

  .sidebar.collapsed .nav-item.active {
    padding-left: 7px;
  }

  .nav-menu .nav-item {
    cursor: grab;
  }

  .nav-menu .nav-item:active {
    cursor: grabbing;
  }

  .nav-icon { flex-shrink: 0; }
  .nav-label { font-size: 0.85rem; }

  .sidebar-footer {
    padding: 16px 12px;
    border-top: 1px solid var(--border-color);
    background: rgba(0, 0, 0, 0.15);
    position: relative;
  }

  .sidebar.collapsed .sidebar-footer {
    padding: 10px 6px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }



  .telemetry-hud {
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    padding: 10px;
    margin-bottom: 12px;
    font-family: var(--font-mono);
    font-size: 0.7rem;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .hud-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .hud-label { color: var(--text-muted); }
  .hud-val { color: var(--text-secondary); font-weight: 500; }
  .hud-val.secure { color: var(--accent-amber); }

  .hud-status {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
  }

  .hud-status.nominal { color: var(--accent-green); }
  .hud-status.offline { color: var(--accent-red); }

  .collapse-toggle {
    color: var(--text-muted);
    margin-bottom: 4px;
  }

  .collapse-toggle:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .logout { color: var(--text-secondary); }
  .logout:hover {
    background: var(--accent-red-glow);
    border-color: rgba(239, 68, 68, 0.2);
    color: var(--accent-red);
  }

  .reconnect-row {
    justify-content: center;
    margin-top: 2px;
  }

  .reconnect-btn {
    width: 100%;
    background: var(--accent-muted);
    border: 1px solid var(--accent-primary);
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    padding: 4px 8px;
    font-size: 0.65rem;
    font-family: var(--font-mono);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    transition: background 0.1s ease, color 0.1s ease;
  }

  .reconnect-btn:hover:not(:disabled) {
    background: var(--accent-primary);
    color: white;
  }

  .reconnect-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .hud-row.clickable {
    cursor: pointer;
    padding: 2px 4px;
    margin: -2px -4px;
    border-radius: var(--radius-sm);
    transition: background 0.15s ease, color 0.15s ease, transform 0.1s ease;
  }

  .hud-row.clickable:hover {
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-primary);
  }

  .hud-row.clickable:active {
    transform: scale(0.98);
  }

  .hud-val.has-dropdown {
    color: var(--accent-primary) !important;
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }

  .hud-chev {
    opacity: 0.8;
  }

  .version-display {
    padding: 6px 12px;
    font-size: 0.7rem;
    color: var(--text-muted);
    text-align: center;
    font-family: var(--font-mono);
    border-top: 1px solid var(--border-color);
    margin-top: 8px;
    user-select: none;
    opacity: 0.6;
  }
</style>
