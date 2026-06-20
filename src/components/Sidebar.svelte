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
  } from 'lucide-svelte';
  import type { ServerProfile } from '$lib/admin/types';
  import { LL } from '$lib/i18n/i18n-svelte';
  import { getNavLabel, TAB_IDS } from '$lib/i18n/nav';
  import { getCurrentLocale, setAppLocale } from '$lib/i18n/localeStore.svelte';
  import { get } from 'svelte/store';
  import type { Locales } from '$lib/i18n/i18n-types';

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
  let currentLocale = $state<Locales>(getCurrentLocale());

  const menuItems = TAB_IDS.map((id) => ({
    id,
    icon: {
      dashboard: LayoutDashboard,
      maintenance: Wrench,
      backups: Database,
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

  const displayHostname = $derived(hostname || get(LL).shell.defaultServerLabel());

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

  async function switchLocale(locale: Locales) {
    if (locale === currentLocale) return;
    await setAppLocale(locale);
    currentLocale = locale;
  }

  const currentProfile = $derived(profiles.find((p) => p.id === currentProfileId));
</script>

<svelte:window onclick={() => (showProfileMenu = false)} />

<aside class="sidebar glass" class:collapsed>
  <div class="brand">
    <button class="logo-circle" onclick={toggleCollapse} title={collapsed ? $LL.sidebar.expandSidebar() : $LL.sidebar.collapseSidebar()}>J</button>
    {#if !collapsed}
      <div class="brand-info">
        <span class="brand-name">{$LL.sidebar.brandName()}</span>
        <button
          class="server-switcher"
          class:clickable={profiles.length > 1}
          onclick={(e) => { e.stopPropagation(); toggleProfileMenu(); }}
          title={profiles.length > 1 ? $LL.sidebar.switchServer() : displayHostname}
        >
          {#if isSwitching}
            <Loader2 size={12} class="spin" />
          {:else}
            <span class="status-dot" class:offline={!isOnline}></span>
          {/if}
          <span class="switcher-label">
            {currentProfile?.label || displayHostname}
          </span>
          {#if profiles.length > 1}
            <ChevronDown size={12} class="chev" />
          {/if}
        </button>
      </div>
    {/if}
  </div>

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

  <nav class="nav-menu">
    {#each menuItems as item}
      {@const label = getNavLabel(get(LL), item.id)}
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
    {/each}
  </nav>

  <div class="sidebar-footer">
    {#if !collapsed}
      <div class="locale-picker" role="group" aria-label={$LL.locale.label()}>
        <button
          type="button"
          class="locale-btn"
          class:active={currentLocale === 'en'}
          onclick={() => switchLocale('en')}
        >EN</button>
        <span class="locale-sep">|</span>
        <button
          type="button"
          class="locale-btn"
          class:active={currentLocale === 'pl'}
          onclick={() => switchLocale('pl')}
        >PL</button>
      </div>

      <div class="telemetry-hud">
        <div class="hud-row">
          <span class="hud-label">{$LL.sidebar.hostLabel()}</span>
          <span class="hud-val" title={displayHostname}>{displayHostname.length > 14 ? displayHostname.slice(0, 14) + '...' : displayHostname}</span>
        </div>
        <div class="hud-row">
          <span class="hud-label">{$LL.sidebar.statusLabel()}</span>
          <span class="hud-status" class:nominal={isOnline} class:offline={!isOnline}>
            {isSwitching ? $LL.common.switching() : (isOnline ? $LL.common.online() : $LL.common.offline())}
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
                <Loader2 size={10} class="spin" /> {$LL.common.connecting()}
              {:else}
                {$LL.common.reconnect()}
              {/if}
            </button>
          </div>
        {/if}
        <div class="hud-row">
          <span class="hud-label">{$LL.sidebar.tunnelLabel()}</span>
          <span class="hud-val secure">{$LL.sidebar.tunnelValue()}</span>
        </div>
      </div>
    {/if}

    <button class="nav-item collapse-toggle" onclick={toggleCollapse} title={collapsed ? $LL.sidebar.expandSidebarFooter() : $LL.sidebar.collapseSidebarFooter()}>
      {#if collapsed}
        <PanelLeftOpen size={16} class="nav-icon" />
      {:else}
        <PanelLeftClose size={16} class="nav-icon" />
        <span class="nav-label">{$LL.sidebar.collapse()}</span>
      {/if}
    </button>

    <button class="nav-item logout" onclick={onDisconnect} title={collapsed ? $LL.sidebar.disconnect() : ''}>
      <LogOut size={16} class="nav-icon" />
      {#if !collapsed}
        <span class="nav-label">{$LL.sidebar.disconnect()}</span>
      {/if}
    </button>
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

  .brand-info {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
    opacity: 1;
    transition: opacity 0.15s ease;
  }

  .brand-name {
    font-family: var(--font-display);
    font-size: 1rem;
    font-weight: 800;
    letter-spacing: 0.08em;
    color: white;
    white-space: nowrap;
  }

  .server-switcher {
    background: transparent;
    border: none;
    padding: 0;
    margin-top: 2px;
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.7rem;
    color: var(--text-secondary);
    cursor: default;
    text-align: left;
    width: 100%;
    white-space: nowrap;
  }

  .server-switcher.clickable {
    cursor: pointer;
  }

  .server-switcher.clickable:hover {
    color: var(--accent-amber);
  }

  .switcher-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
  }

  .status-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background-color: var(--accent-green);
    box-shadow: 0 0 6px var(--accent-green);
    flex-shrink: 0;
  }

  .status-dot.offline {
    background-color: var(--accent-red);
    box-shadow: 0 0 6px var(--accent-red);
  }

  .chev { flex-shrink: 0; opacity: 0.6; }

  .profile-dropdown {
    position: absolute;
    top: 72px;
    left: 10px;
    right: 10px;
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    z-index: 50;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
    max-height: 240px;
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
  }

  .sidebar.collapsed .sidebar-footer {
    padding: 10px 6px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }

  .locale-picker {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    margin-bottom: 10px;
    font-family: var(--font-mono);
    font-size: 0.72rem;
  }

  .locale-btn {
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 2px 4px;
    font: inherit;
    font-weight: 600;
    letter-spacing: 0.04em;
  }

  .locale-btn:hover {
    color: var(--text-primary);
  }

  .locale-btn.active {
    color: var(--accent-amber);
  }

  .locale-sep {
    color: var(--text-muted);
    opacity: 0.5;
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

</style>
