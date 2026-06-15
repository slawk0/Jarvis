<script lang="ts">
  import { 
    LayoutDashboard, 
    FolderClosed, 
    Settings, 
    Calendar, 
    Users, 
    Shield, 
    FileText, 
    Terminal, 
    LogOut 
  } from 'lucide-svelte';

  // Svelte 5 props
  let { activeTab = $bindable(), onDisconnect, hostname = 'Serwer' } = $props();

  const menuItems = [
    { id: 'dashboard', label: 'Dashboard', icon: LayoutDashboard },
    { id: 'files', label: 'Pliki (SFTP)', icon: FolderClosed },
    { id: 'services', label: 'Usługi (Systemd)', icon: Settings },
    { id: 'cron', label: 'Zadania (Cron)', icon: Calendar },
    { id: 'users', label: 'Użytkownicy', icon: Users },
    { id: 'firewall', label: 'Zapora (UFW)', icon: Shield },
    { id: 'logs', label: 'Logi', icon: FileText },
    { id: 'terminal', label: 'Terminal', icon: Terminal },
  ];
</script>

<aside class="sidebar glass">
  <div class="brand">
    <div class="logo-circle">J</div>
    <div class="brand-info">
      <span class="brand-name">JARVIS</span>
      <span class="server-status" title={hostname}>
        <span class="status-dot"></span>
        {hostname.length > 15 ? hostname.slice(0, 15) + '...' : hostname}
      </span>
    </div>
  </div>

  <nav class="nav-menu">
    {#each menuItems as item}
      <button 
        class="nav-item {activeTab === item.id ? 'active' : ''}" 
        onclick={() => activeTab = item.id}
      >
        <item.icon size={18} class="nav-icon" />
        <span class="nav-label">{item.label}</span>
      </button>
    {/each}
  </nav>

  <div class="sidebar-footer">
    <button class="nav-item logout" onclick={onDisconnect}>
      <LogOut size={18} class="nav-icon" />
      <span class="nav-label">Rozłącz</span>
    </button>
  </div>
</aside>

<style>
  .sidebar {
    width: 260px;
    height: 100vh;
    display: flex;
    flex-direction: column;
    border-right: 1px solid var(--border-color);
    background: rgba(10, 13, 22, 0.4);
    flex-shrink: 0;
  }

  .brand {
    padding: 24px;
    display: flex;
    align-items: center;
    gap: 12px;
    border-bottom: 1px solid var(--border-color);
  }

  .logo-circle {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    background: linear-gradient(135deg, var(--accent-blue), var(--accent-purple));
    color: white;
    font-weight: 800;
    font-size: 1.2rem;
    font-family: var(--font-display);
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 0 15px rgba(0, 210, 255, 0.4);
  }

  .brand-info {
    display: flex;
    flex-direction: column;
  }

  .brand-name {
    font-family: var(--font-display);
    font-size: 1.1rem;
    font-weight: 800;
    letter-spacing: 0.05em;
    color: white;
  }

  .server-status {
    font-size: 0.75rem;
    color: var(--text-secondary);
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 2px;
  }

  .status-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background-color: var(--accent-green);
    box-shadow: 0 0 8px var(--accent-green);
  }

  .nav-menu {
    flex: 1;
    padding: 16px 12px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    overflow-y: auto;
  }

  .nav-item {
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    padding: 12px 16px;
    width: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    cursor: pointer;
    text-align: left;
    transition: var(--transition-fast);
  }

  .nav-item:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .nav-item.active {
    background: rgba(59, 130, 246, 0.1);
    color: var(--accent-blue);
    border: 1px solid rgba(59, 130, 246, 0.25);
    font-weight: 600;
  }

  .nav-icon {
    flex-shrink: 0;
  }

  .nav-label {
    font-size: 0.9rem;
  }

  .sidebar-footer {
    padding: 16px 12px;
    border-top: 1px solid var(--border-color);
  }

  .logout {
    color: var(--accent-red);
  }

  .logout:hover {
    background: rgba(244, 63, 94, 0.1);
    color: #fff;
  }
</style>
