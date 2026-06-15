<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { 
    Plus, 
    Trash2, 
    Server, 
    Key, 
    Lock, 
    User, 
    Globe, 
    Settings, 
    ChevronRight, 
    Loader2, 
    AlertCircle,
    Activity
  } from 'lucide-svelte';

  // Komponenty
  import Sidebar from '../components/Sidebar.svelte';
  import Dashboard from '../components/Dashboard.svelte';
  import FileManager from '../components/FileManager.svelte';
  import ServicesManager from '../components/ServicesManager.svelte';
  import CronManager from '../components/CronManager.svelte';
  import UserManager from '../components/UserManager.svelte';
  import FirewallManager from '../components/FirewallManager.svelte';
  import LogViewer from '../components/LogViewer.svelte';
  import TerminalView from '../components/TerminalView.svelte';
  import DockerManager from '../components/DockerManager.svelte';

  // Stany logowania i połączenia
  let isConnected = $state(false);
  let isConnecting = $state(false);
  let connectError = $state('');
  let serverStats = $state<any>(null);
  let activeTab = $state('dashboard');
  let currentHostname = $state('Serwer');
  let currentProfileId = $state('');
  let terminalContainerSession = $state<{ containerId: string; containerName: string; useSudo: boolean; shell: string } | null>(null);

  // Profile serwerów
  let profiles = $state<any[]>([]);
  let showCreateProfile = $state(false);
  
  // Pola formularza profilu
  let profileLabel = $state('');
  let profileHost = $state('');
  let profilePort = $state(22);
  let profileUsername = $state('root');
  let profileAuthType = $state('password'); // 'password' | 'key'
  let profileKeyPath = $state('');
  let profilePassword = $state('');
  let profileKeyPassphrase = $state('');

  async function loadProfiles() {
    try {
      profiles = await invoke('get_profiles');
    } catch (err) {
      console.error('Błąd pobierania profili:', err);
    }
  }

  async function handleSaveProfile() {
    if (!profileLabel || !profileHost || !profileUsername) {
      alert('Wypełnij wymagane pola (Etykieta, Host, Użytkownik)');
      return;
    }

    const newProfile = {
      id: currentProfileId || Date.now().toString(),
      label: profileLabel,
      host: profileHost,
      port: Number(profilePort),
      username: profileUsername,
      auth_type: profileAuthType,
      key_path: profileAuthType === 'key' ? profileKeyPath : null
    };

    try {
      await invoke('save_profile', {
        profile: newProfile,
        password: profilePassword ? profilePassword : null,
        keyPassphrase: profileKeyPassphrase ? profileKeyPassphrase : null
      });

      // Zresetuj formularz
      profileLabel = '';
      profileHost = '';
      profilePort = 22;
      profileUsername = 'root';
      profileAuthType = 'password';
      profileKeyPath = '';
      profilePassword = '';
      profileKeyPassphrase = '';
      currentProfileId = '';
      showCreateProfile = false;

      await loadProfiles();
    } catch (err: any) {
      alert('Nie udało się zapisać profilu: ' + err.toString());
    }
  }

  async function handleConnect(profileId: string) {
    isConnecting = true;
    connectError = '';
    currentProfileId = profileId;
    
    try {
      const stats = await invoke<any>('connect_ssh', { profileId });
      serverStats = stats;
      currentHostname = stats.hostname;
      isConnected = true;
      activeTab = 'dashboard';
    } catch (err: any) {
      connectError = err.toString();
    } finally {
      isConnecting = false;
    }
  }

  async function handleDisconnect() {
    try {
      await invoke('disconnect_ssh');
    } catch (err) {
      console.error(err);
    } finally {
      isConnected = false;
      serverStats = null;
      currentHostname = 'Serwer';
    }
  }

  async function handleDeleteProfile(id: string, event: Event) {
    event.stopPropagation(); // Zapobiegaj kliknięciu w cały wiersz (który łączy)
    if (confirm('Czy na pewno chcesz usunąć ten profil połączenia?')) {
      try {
        await invoke('delete_profile', { id });
        await loadProfiles();
      } catch (err: any) {
        alert(err.toString());
      }
    }
  }

  function editProfile(profile: any, event: Event) {
    event.stopPropagation();
    currentProfileId = profile.id;
    profileLabel = profile.label;
    profileHost = profile.host;
    profilePort = profile.port;
    profileUsername = profile.username;
    profileAuthType = profile.auth_type;
    profileKeyPath = profile.key_path || '';
    profilePassword = ''; // Hasła nie odczytujemy z powrotem z powodów bezpieczeństwa
    profileKeyPassphrase = '';
    showCreateProfile = true;
  }

  onMount(() => {
    loadProfiles();
  });
</script>

<main class="app-container">
  {#if isConnected}
    <!-- GŁÓWNY WORKSPACE APILKACJI -->
    <Sidebar
      bind:activeTab={activeTab}
      hostname={currentHostname}
      onDisconnect={handleDisconnect}
      onTabSelect={(tab: string) => {
        if (tab === 'terminal') terminalContainerSession = null;
      }}
    />
    
    <div class="main-content">
      {#if activeTab === 'dashboard'}
        <Dashboard initialStats={serverStats} />
      {:else}
        {#key activeTab}
          <div class="tab-wrapper">
            {#if activeTab === 'files'}
              <FileManager />
            {:else if activeTab === 'services'}
              <ServicesManager />
            {:else if activeTab === 'docker'}
              <DockerManager onRequestTerminalExec={(session: { containerId: string; containerName: string; useSudo: boolean; shell: string }) => {
                terminalContainerSession = session;
                activeTab = 'terminal';
              }} />
            {:else if activeTab === 'cron'}
              <CronManager />
            {:else if activeTab === 'users'}
              <UserManager />
            {:else if activeTab === 'firewall'}
              <FirewallManager />
            {:else if activeTab === 'logs'}
              <LogViewer />
            {:else if activeTab === 'terminal'}
              <TerminalView
                profileId={currentProfileId}
                containerSession={terminalContainerSession}
                onExitContainer={() => { terminalContainerSession = null; }}
              />
            {/if}
          </div>
        {/key}
      {/if}
    </div>
  {:else}
    <!-- EKRAN LOGOWANIA / PANELU ZARZĄDZANIA PROFILAMI -->
    <div class="login-screen">
      <div class="login-glow"></div>
      
      <div class="login-container glass fade-in">
        <header class="login-header">
          <div class="logo-box">J</div>
          <h1>Jarvis Server Manager</h1>
          <p class="login-subtitle">Kompletne, bezpieczne narzędzie do zarządzania systemami Linux</p>
        </header>

        {#if connectError}
          <div class="login-error">
            <AlertCircle size={18} />
            <span>{connectError}</span>
          </div>
        {/if}

        {#if showCreateProfile}
          <!-- Formularz tworzenia/edycji profilu -->
          <div class="profile-form">
            <h2>{currentProfileId ? 'Edytuj Profil' : 'Dodaj Nowy Profil'}</h2>
            
            <div class="form-group">
              <label for="prof-label">Nazwa profilu (Etykieta)</label>
              <input id="prof-label" type="text" placeholder="Mój serwer VPS" bind:value={profileLabel} />
            </div>

            <div class="form-row">
              <div class="form-group flex-3">
                <label for="prof-host">Adres hosta (IP lub Domena)</label>
                <input id="prof-host" type="text" placeholder="192.168.1.100" bind:value={profileHost} />
              </div>
              <div class="form-group flex-1">
                <label for="prof-port">Port SSH</label>
                <input id="prof-port" type="number" bind:value={profilePort} />
              </div>
            </div>

            <div class="form-group">
              <label for="prof-user">Użytkownik SSH</label>
              <input id="prof-user" type="text" placeholder="root" bind:value={profileUsername} />
            </div>

            <div class="form-group">
              <label for="prof-authtype">Metoda autoryzacji</label>
              <select id="prof-authtype" bind:value={profileAuthType}>
                <option value="password">Hasło tekstowe</option>
                <option value="key">Klucz prywatny SSH</option>
              </select>
            </div>

            {#if profileAuthType === 'password'}
              <div class="form-group">
                <label for="prof-pass">Hasło SSH (zostanie zapisane w Windows Credential Manager)</label>
                <input id="prof-pass" type="password" placeholder="••••••••" bind:value={profilePassword} />
              </div>
            {:else}
              <div class="form-group">
                <label for="prof-keypath">Ścieżka do klucza prywatnego (np. C:\Users\user\.ssh\id_rsa)</label>
                <input id="prof-keypath" type="text" placeholder="C:\Users\Nazwa\.ssh\id_rsa" bind:value={profileKeyPath} />
              </div>
              <div class="form-group">
                <label for="prof-keypass">Hasło do klucza (Passphrase) - jeśli wymagane</label>
                <input id="prof-keypass" type="password" placeholder="••••••••" bind:value={profileKeyPassphrase} />
              </div>
            {/if}

            <div class="profile-form-actions">
              <button class="primary" onclick={handleSaveProfile}>Zapisz Profil</button>
              <button class="secondary" onclick={() => { showCreateProfile = false; currentProfileId = ''; }}>Anuluj</button>
            </div>
          </div>
        {:else}
          <!-- Lista profili połączeń -->
          <div class="profiles-section">
            <div class="section-header">
              <h2>Zapisane Profile</h2>
              <button class="secondary btn-sm" onclick={() => showCreateProfile = true}>
                <Plus size={14} /> Nowy Profil
              </button>
            </div>

            <div class="profiles-list">
              {#each profiles as profile}
                <div 
                  class="profile-card glass {isConnecting && currentProfileId === profile.id ? 'connecting' : ''}" 
                  role="button"
                  tabindex="0"
                  onclick={() => !isConnecting && handleConnect(profile.id)}
                  onkeydown={(e) => e.key === 'Enter' && !isConnecting && handleConnect(profile.id)}
                >
                  <div class="profile-card-left">
                    <div class="server-icon-box">
                      <Server size={18} />
                    </div>
                    <div class="profile-card-info">
                      <span class="profile-label">{profile.label}</span>
                      <span class="profile-details">{profile.username}@{profile.host}:{profile.port}</span>
                    </div>
                  </div>
                  <div class="profile-card-right">
                    {#if isConnecting && currentProfileId === profile.id}
                      <Loader2 class="spin accent-amber-text" size={18} />
                    {:else}
                      <button class="icon-btn-card" onclick={(e) => editProfile(profile, e)} title="Edytuj">
                        <Settings size={14} />
                      </button>
                      <button class="icon-btn-card hover-red" onclick={(e) => handleDeleteProfile(profile.id, e)} title="Usuń">
                        <Trash2 size={14} />
                      </button>
                      <ChevronRight class="chevron-icon" size={18} />
                    {/if}
                  </div>
                </div>
              {/each}

              {#if profiles.length === 0}
                <div class="no-profiles glass">
                  <Server size={36} class="muted-icon" />
                  <p>Brak zapisanych serwerów</p>
                  <button class="primary btn-sm" onclick={() => showCreateProfile = true}>
                    <Plus size={14} /> Utwórz pierwszy profil
                  </button>
                </div>
              {/if}
            </div>
          </div>
        {/if}
      </div>
    </div>
  {/if}
</main>

<style>
  .tab-wrapper {
    height: 100%;
    width: 100%;
    overflow: hidden;
  }

  /* Login screen styles */
  .login-screen {
    position: relative;
    width: 100vw;
    height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    background-color: var(--bg-primary);
    overflow: hidden;
  }

  .login-glow {
    position: absolute;
    width: 600px;
    height: 600px;
    background: radial-gradient(circle, rgba(245, 158, 11, 0.08) 0%, rgba(194, 65, 12, 0.02) 50%, rgba(0,0,0,0) 100%);
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    z-index: 0;
    pointer-events: none;
  }

  .login-container {
    width: 460px;
    padding: 40px;
    border-radius: var(--radius-md);
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    z-index: 1;
    max-height: 90vh;
    overflow-y: auto;
  }

  .login-header {
    text-align: center;
    margin-bottom: 30px;
    display: flex;
    flex-direction: column;
    align-items: center;
  }

  .logo-box {
    width: 48px;
    height: 48px;
    border-radius: var(--radius-sm);
    background: linear-gradient(135deg, var(--accent-amber), var(--accent-rust));
    color: #0c0d12;
    font-weight: 800;
    font-size: 1.5rem;
    font-family: var(--font-display);
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 0 20px rgba(245, 158, 11, 0.3);
    margin-bottom: 16px;
  }

  .login-header h1 {
    font-size: 1.6rem;
    color: white;
    font-weight: 700;
  }

  .login-subtitle {
    font-size: 0.85rem;
    color: var(--text-secondary);
    margin-top: 6px;
    line-height: 1.4;
  }

  .login-error {
    background: var(--accent-red-glow);
    border: 1px solid rgba(239, 68, 68, 0.3);
    border-radius: var(--radius-sm);
    padding: 12px;
    color: #ff8585;
    font-size: 0.85rem;
    margin-bottom: 24px;
    display: flex;
    align-items: flex-start;
    gap: 10px;
  }

  /* Profiles Section */
  .profiles-section {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .profiles-section h2, .profile-form h2 {
    font-size: 1.1rem;
    color: white;
  }

  .profiles-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-height: 400px;
    overflow-y: auto;
    padding-right: 4px;
  }

  .profile-card {
    background: rgba(255, 255, 255, 0.01);
    border: 1px solid var(--border-color);
    padding: 16px;
    border-radius: var(--radius-sm);
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    cursor: pointer;
    text-align: left;
    transition: var(--transition-fast);
  }

  .profile-card:hover {
    background: var(--bg-hover);
    border-color: rgba(245, 158, 11, 0.2);
    transform: translateY(-1px);
  }

  .profile-card:active {
    transform: translateY(0);
  }

  .profile-card.connecting {
    background: var(--bg-active);
    border-color: var(--accent-amber);
    cursor: not-allowed;
  }

  .profile-card-left {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .server-icon-box {
    background: rgba(255, 255, 255, 0.03);
    padding: 10px;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    border: 1px solid var(--border-color);
  }

  .profile-card:hover .server-icon-box {
    color: var(--accent-amber);
    background: rgba(245, 158, 11, 0.08);
    border-color: rgba(245, 158, 11, 0.2);
  }

  .profile-card-info {
    display: flex;
    flex-direction: column;
  }

  .profile-label {
    font-size: 0.95rem;
    color: white;
    font-weight: 600;
  }

  .profile-details {
    font-size: 0.75rem;
    color: var(--text-muted);
    margin-top: 4px;
  }

  .profile-card-right {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .icon-btn-card {
    background: transparent;
    border: none;
    padding: 6px;
    border-radius: 4px;
    color: var(--text-muted);
    cursor: pointer;
    transition: var(--transition-fast);
  }

  .icon-btn-card:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .icon-btn-card.hover-red:hover {
    color: var(--accent-red);
    background: var(--accent-red-glow);
  }

  .chevron-icon {
    color: var(--text-muted);
    transition: var(--transition-fast);
  }

  .profile-card:hover .chevron-icon {
    color: white;
    transform: translateX(2px);
  }

  .no-profiles {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 40px;
    text-align: center;
    gap: 16px;
    border-radius: var(--radius-md);
  }

  .muted-icon {
    color: var(--text-muted);
  }

  .no-profiles p {
    color: var(--text-secondary);
    font-size: 0.9rem;
  }

  /* Profile Form */
  .profile-form {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .profile-form h2 {
    border-bottom: 1px solid var(--border-color);
    padding-bottom: 10px;
  }

  .form-row {
    display: flex;
    gap: 16px;
  }

  .flex-1 { flex: 1; }
  .flex-3 { flex: 3; }

  .profile-form-actions {
    display: flex;
    gap: 10px;
    justify-content: flex-end;
    margin-top: 8px;
  }

  .btn-sm {
    padding: 6px 12px;
    font-size: 0.8rem;
  }

  .spin {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
