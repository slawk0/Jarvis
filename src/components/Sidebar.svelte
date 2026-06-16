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
  } from 'lucide-svelte';
  import type { ServerProfile } from '$lib/admin/types';

  let {
    activeTab = $bindable(),
    onDisconnect,
    hostname = 'Serwer',
    onTabSelect = (_tab: string) => {},
    profiles = [] as ServerProfile[],
    currentProfileId = '',
    onSwitchProfile = (_id: string) => {},
    isSwitching = false,
  } = $props();

  let showProfileMenu = $state(false);

  const menuItems = [
    { id: 'dashboard', label: 'Dashboard', icon: LayoutDashboard },
    { id: 'maintenance', label: 'Konserwacja', icon: Wrench },
    { id: 'backups', label: 'Backupy', icon: Database },
    { id: 'network', label: 'Sieć / Porty', icon: Network },
    { id: 'runbooks', label: 'Runbooki', icon: BookOpen },
    { id: 'files', label: 'Pliki (SFTP)', icon: FolderClosed },
    { id: 'services', label: 'Usługi (Systemd)', icon: Settings },
    { id: 'docker', label: 'Docker', icon: Box },
    { id: 'cron', label: 'Zadania (Cron)', icon: Calendar },
    { id: 'users', label: 'Użytkownicy', icon: Users },
    { id: 'firewall', label: 'Zapora (UFW)', icon: Shield },
    { id: 'crowdsec', label: 'CrowdSec', icon: ShieldAlert },
    { id: 'pangolin', label: 'Pangolin Proxy', icon: Globe },
    { id: 'logs', label: 'Logi', icon: FileText },
    { id: 'terminal', label: 'Terminal', icon: Terminal },
  ];

  function toggleProfileMenu() {
    if (profiles.length <= 1) return;
    showProfileMenu = !showProfileMenu;
  }

  function selectProfile(id: string) {
    showProfileMenu = false;
    if (id !== currentProfileId) onSwitchProfile(id);
  }

  const currentProfile = $derived(profiles.find((p) => p.id === currentProfileId));
</script>

<svelte:window onclick={() => (showProfileMenu = false)} />

<aside class="sidebar glass">
  <div class="brand">
    <div class="logo-circle">J</div>
    <div class="brand-info">
      <span class="brand-name">JARVIS</span>
      <button
        class="server-switcher"
        class:clickable={profiles.length > 1}
        onclick={(e) => { e.stopPropagation(); toggleProfileMenu(); }}
        title={profiles.length > 1 ? 'Przełącz serwer' : hostname}
      >
        {#if isSwitching}
          <Loader2 size={12} class="spin" />
        {:else}
          <span class="status-dot"></span>
        {/if}
        <span class="switcher-label">
          {currentProfile?.label || hostname}
        </span>
        {#if profiles.length > 1}
          <ChevronDown size={12} class="chev" />
        {/if}
      </button>
    </div>
  </div>

  {#if showProfileMenu && profiles.length > 1}
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
      <button 
        class="nav-item {activeTab === item.id ? 'active' : ''}" 
        onclick={() => onTabSelect(item.id)}
      >
        <item.icon size={18} class="nav-icon" />
        <span class="nav-label">{item.label}</span>
      </button>
    {/each}
  </nav>

  <div class="sidebar-footer">
    <div class="telemetry-hud">
      <div class="hud-row">
        <span class="hud-label">HOST:</span>
        <span class="hud-val" title={hostname}>{hostname.length > 14 ? hostname.slice(0, 14) + '...' : hostname}</span>
      </div>
      <div class="hud-row">
        <span class="hud-label">STATUS:</span>
        <span class="hud-status nominal">
          <span class="heartbeat"></span> {isSwitching ? 'PRZEŁĄCZANIE' : 'ONLINE'}
        </span>
      </div>
      <div class="hud-row">
        <span class="hud-label">TUNNEL:</span>
        <span class="hud-val secure">SSH/SFTP</span>
      </div>
    </div>

    <button class="nav-item logout" onclick={onDisconnect}>
      <LogOut size={16} class="nav-icon" />
      <span class="nav-label">Rozłącz</span>
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
  }

  .brand {
    padding: 20px;
    display: flex;
    align-items: center;
    gap: 12px;
    border-bottom: 1px solid var(--border-color);
  }

  .logo-circle {
    width: 30px;
    height: 30px;
    border-radius: var(--radius-sm);
    background: linear-gradient(135deg, var(--accent-amber), var(--accent-rust));
    color: #0c0d12;
    font-weight: 800;
    font-size: 1.1rem;
    font-family: var(--font-display);
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 0 12px rgba(245, 158, 11, 0.25);
    flex-shrink: 0;
  }

  .brand-info {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }

  .brand-name {
    font-family: var(--font-display);
    font-size: 1rem;
    font-weight: 800;
    letter-spacing: 0.08em;
    color: white;
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
    transition: var(--transition-fast);
  }

  .nav-item:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .nav-item.active {
    background: var(--bg-active);
    color: var(--accent-amber);
    border: 1px solid rgba(245, 158, 11, 0.2);
    font-weight: 600;
  }

  .nav-icon { flex-shrink: 0; }
  .nav-label { font-size: 0.85rem; }

  .sidebar-footer {
    padding: 16px 12px;
    border-top: 1px solid var(--border-color);
    background: rgba(0, 0, 0, 0.15);
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

  .heartbeat {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--accent-green);
    display: inline-block;
    animation: heartbeat-pulse 1.8s infinite ease-in-out;
  }

  @keyframes heartbeat-pulse {
    0% { transform: scale(0.9); opacity: 0.6; }
    50% { transform: scale(1.1); opacity: 1; }
    100% { transform: scale(0.9); opacity: 0.6; }
  }

  .logout { color: var(--text-secondary); }
  .logout:hover {
    background: var(--accent-red-glow);
    border-color: rgba(239, 68, 68, 0.2);
    color: var(--accent-red);
  }

  .spin { animation: spin 1s linear infinite; }
  @keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
</style>
