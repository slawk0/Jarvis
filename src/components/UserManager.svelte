<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { Users, UserPlus, Trash2, KeyRound, RefreshCw, Check, ShieldAlert, Settings2, Plus } from 'lucide-svelte';
  import SortableTh from './ui/SortableTh.svelte';
  import { applySort, nextSort, type SortState } from '$lib/sort/sortUtils';

  let users = $state<any[]>([]);
  let groups = $state<any[]>([]);
  let isLoading = $state(false);
  let errorMsg = $state('');
  let showSystemAccounts = $state(false);

  // Zarządzanie Sudo
  let showSudoModal = $state(false);
  let sudoPassword = $state('');
  let pendingAction: (() => Promise<void>) | null = null;
  let sudoError = $state('');

  // Modale
  let showCreateUserModal = $state(false);
  let showCreateGroupModal = $state(false);
  let showChangePassModal = $state(false);
  let showGroupsModal = $state(false);

  // Zmienne formularzy
  let newUsername = $state('');
  let newUserShell = $state('/bin/bash');
  let newUserHome = $state('');
  
  let newGroupName = $state('');

  let targetUser = $state<any>(null);
  let targetPassword = $state('');

  let selectedUserGroups = $state<string[]>([]);
  let availableGroups = $state<any[]>([]);

  async function loadData() {
    isLoading = true;
    errorMsg = '';
    try {
      // 1. Pobierz Użytkowników z /etc/passwd
      const passwdOut: string = await invoke('exec_custom_command', {
        cmd: 'cat /etc/passwd',
        useSudo: false
      });
      
      const parsedUsers = passwdOut.trim().split('\n').map(line => {
        const parts = line.split(':');
        if (parts.length < 7) return null;
        return {
          username: parts[0],
          uid: parseInt(parts[2]),
          gid: parseInt(parts[3]),
          info: parts[4],
          home: parts[5],
          shell: parts[6]
        };
      }).filter(Boolean);

      // 2. Pobierz Grupy z /etc/group
      const groupOut: string = await invoke('exec_custom_command', {
        cmd: 'cat /etc/group',
        useSudo: false
      });
      
      const parsedGroups = groupOut.trim().split('\n').map(line => {
        const parts = line.split(':');
        if (parts.length < 4) return null;
        return {
          name: parts[0],
          gid: parseInt(parts[2]),
          members: parts[3] ? parts[3].split(',') : []
        };
      }).filter(Boolean);

      users = parsedUsers;
      groups = parsedGroups;
    } catch (err: any) {
      errorMsg = 'Błąd wczytywania danych użytkowników: ' + err.toString();
    } finally {
      isLoading = false;
    }
  }

  function getVisibleUsers() {
    if (showSystemAccounts) {
      return users;
    }
    return users.filter(u => u.uid >= 1000 || u.uid === 0);
  }

  type UserSortCol = 'name' | 'home';
  type GroupSortCol = 'name' | 'members';
  let userSort = $state<SortState<UserSortCol>>({ column: 'name', direction: 'asc' });
  let groupSort = $state<SortState<GroupSortCol>>({ column: 'name', direction: 'asc' });

  const sortedUsers = $derived(
    applySort(getVisibleUsers(), userSort, {
      name: (u) => u.username || '',
      home: (u) => u.home || '',
    }),
  );

  const visibleGroups = $derived(
    groups.filter(g => showSystemAccounts || g.gid >= 1000 || g.name === 'sudo' || g.name === 'docker'),
  );

  const sortedGroups = $derived(
    applySort(visibleGroups, groupSort, {
      name: (g) => g.name || '',
      members: (g) => (g.members || []).join(', '),
    }),
  );

  function setUserSort(column: string) {
    userSort = nextSort(userSort, column as UserSortCol);
  }

  function setGroupSort(column: string) {
    groupSort = nextSort(groupSort, column as GroupSortCol);
  }

  async function handleActionWithSudo(action: () => Promise<void>) {
    const run = async () => {
      try {
        await action();
      } catch (err: any) {
        if (err.toString() === 'SUDO_PASSWORD_REQUIRED') {
          pendingAction = run;
          showSudoModal = true;
        } else {
          errorMsg = 'Błąd wykonania polecenia: ' + err.toString();
        }
      }
    };
    await run();
  }

  async function submitSudoPassword() {
    sudoError = '';
    try {
      await invoke('set_sudo_password', { password: sudoPassword });
      showSudoModal = false;
      sudoPassword = '';
      if (pendingAction) {
        const action = pendingAction;
        pendingAction = null;
        await action();
      }
    } catch (err: any) {
      sudoError = err.toString();
    }
  }

  // Funkcje CRUD Użytkowników
  async function createUser() {
    if (!newUsername) return;
    
    const action = async () => {
      isLoading = true;
      errorMsg = '';
      const homeArg = newUserHome ? `-d "${newUserHome}"` : '-m';
      const cmd = `useradd ${homeArg} -s "${newUserShell}" "${newUsername}"`;
      
      await invoke('exec_custom_command', { cmd, useSudo: true });
      showCreateUserModal = false;
      newUsername = '';
      newUserHome = '';
      await loadData();
    };

    await handleActionWithSudo(action);
  }

  async function deleteUser(username: string) {
    if (confirm(`Czy na pewno chcesz usunąć użytkownika "${username}" wraz z katalogiem domowym?`)) {
      const action = async () => {
        isLoading = true;
        errorMsg = '';
        await invoke('exec_custom_command', { 
          cmd: `userdel -r "${username}"`, 
          useSudo: true 
        });
        await loadData();
      };
      await handleActionWithSudo(action);
    }
  }

  async function changePassword() {
    if (!targetUser || !targetPassword) return;
    
    const action = async () => {
      isLoading = true;
      errorMsg = '';
      // Bezpieczny chpasswd w strumieniu wejściowym
      await invoke('exec_custom_command', {
        cmd: `echo "${targetUser.username}:${targetPassword}" | chpasswd`,
        useSudo: true
      });
      showChangePassModal = false;
      targetPassword = '';
      targetUser = null;
      alert('Hasło zostało pomyślnie zmienione.');
    };
    
    await handleActionWithSudo(action);
  }

  // Funkcje Grup
  async function createGroup() {
    if (!newGroupName) return;
    const action = async () => {
      isLoading = true;
      errorMsg = '';
      await invoke('exec_custom_command', {
        cmd: `groupadd "${newGroupName}"`,
        useSudo: true
      });
      showCreateGroupModal = false;
      newGroupName = '';
      await loadData();
    };
    await handleActionWithSudo(action);
  }

  async function deleteGroup(groupName: string) {
    if (confirm(`Czy na pewno chcesz usunąć grupę "${groupName}"?`)) {
      const action = async () => {
        isLoading = true;
        errorMsg = '';
        await invoke('exec_custom_command', {
          cmd: `groupdel "${groupName}"`,
          useSudo: true
        });
        await loadData();
      };
      await handleActionWithSudo(action);
    }
  }

  async function openGroupsModal(user: any) {
    targetUser = user;
    // Znajdź grupy do których należy user
    selectedUserGroups = groups
      .filter(g => g.members.includes(user.username) || (user.gid === g.gid))
      .map(g => g.name);
    
    showGroupsModal = true;
  }

  async function saveUserGroups() {
    if (!targetUser) return;
    
    const action = async () => {
      isLoading = true;
      errorMsg = '';
      
      // 1. Ustal wszystkie grupy, do których należy użytkownik obecnie
      const currentGroups = groups
        .filter(g => g.members.includes(targetUser.username))
        .map(g => g.name);
      
      // 2. Dodaj użytkownika do nowo zaznaczonych grup
      for (const group of selectedUserGroups) {
        if (!currentGroups.includes(group)) {
          await invoke('exec_custom_command', {
            cmd: `usermod -aG "${group}" "${targetUser.username}"`,
            useSudo: true
          });
        }
      }
      
      // 3. Usuń użytkownika z grup, które zostały odznaczone
      for (const group of currentGroups) {
        if (!selectedUserGroups.includes(group)) {
          // Unikaj usunięcia użytkownika z jego grupy podstawowej
          const groupObj = groups.find(g => g.name === group);
          if (groupObj && groupObj.gid !== targetUser.gid) {
            await invoke('exec_custom_command', {
              cmd: `gpasswd -d "${targetUser.username}" "${group}"`,
              useSudo: true
            });
          }
        }
      }

      showGroupsModal = false;
      targetUser = null;
      await loadData();
    };
    
    await handleActionWithSudo(action);
  }

  onMount(() => {
    loadData();
  });
</script>

<div class="user-manager manager-shell fade-in">
  <header class="manager-header">
    <h1 class="page-title">Użytkownicy i Grupy</h1>
    {#if errorMsg}
      <div class="error-badge">{errorMsg}</div>
    {/if}
  </header>

  <!-- Pasek operacji -->
  <div class="ops-bar glass">
    <label class="toggle-checkbox">
      <input type="checkbox" bind:checked={showSystemAccounts} />
      <span>Pokaż konta systemowe (UID &lt; 1000)</span>
    </label>
    <button class="secondary" onclick={loadData} disabled={isLoading}>
      <RefreshCw size={16} class={isLoading ? 'spin' : ''} /> Odśwież
    </button>
    <button class="primary" onclick={() => showCreateUserModal = true}>
      <UserPlus size={16} /> Nowy Użytkownik
    </button>
    <button class="secondary" onclick={() => showCreateGroupModal = true}>
      <Plus size={16} /> Nowa Grupa
    </button>
  </div>

  <div class="split-view">
    <!-- Lista użytkowników -->
    <div class="table-container glass users-section">
      <h3>Lista Użytkowników</h3>
      {#if isLoading && users.length === 0}
        <div class="loading-state">
          <RefreshCw class="spin" size={32} />
          <p>Wczytywanie kont...</p>
        </div>
      {:else}
        <table class="users-table">
          <thead>
            <tr>
              <SortableTh label="Nazwa (UID)" column="name" activeColumn={userSort.column} direction={userSort.direction} onsort={setUserSort} width="30%" />
              <SortableTh label="Katalog Domowy & Powłoka" column="home" activeColumn={userSort.column} direction={userSort.direction} onsort={setUserSort} width="35%" />
              <th style="width: 35%; text-align: right; padding: 14px 16px; font-size: 0.8rem; text-transform: uppercase; color: var(--text-muted); font-weight: 600;">Akcje</th>
            </tr>
          </thead>
          <tbody>
            {#each sortedUsers as user}
              <tr>
                <td>
                  <span class="user-name"><strong>{user.username}</strong></span>
                  <span class="uid-tag mono-val">UID: {user.uid}</span>
                </td>
                <td>
                  <span class="home-dir mono-val">{user.home}</span>
                  <span class="shell-code mono-val"><code>{user.shell}</code></span>
                </td>
                <td class="actions-cell">
                  <button class="btn-table" onclick={() => { targetUser = user; showChangePassModal = true; }} title="Zmień hasło">
                    <KeyRound size={14} />
                  </button>
                  <button class="btn-table" onclick={() => openGroupsModal(user)} title="Zarządzaj grupami">
                    <Settings2 size={14} />
                  </button>
                  {#if user.uid !== 0 && user.username !== 'slawek'}
                    <button class="btn-table danger-text" onclick={() => deleteUser(user.username)} title="Usuń użytkownika">
                      <Trash2 size={14} />
                    </button>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>

    <!-- Lista grup -->
    <div class="table-container glass groups-section">
      <h3>Grupy Systemowe</h3>
      {#if isLoading && groups.length === 0}
        <div class="loading-state">
          <RefreshCw class="spin" size={32} />
        </div>
      {:else}
        <table class="groups-table">
          <thead>
            <tr>
              <SortableTh label="Nazwa Grupy (GID)" column="name" activeColumn={groupSort.column} direction={groupSort.direction} onsort={setGroupSort} width="40%" />
              <SortableTh label="Członkowie" column="members" activeColumn={groupSort.column} direction={groupSort.direction} onsort={setGroupSort} width="40%" />
              <th style="width: 20%; text-align: right; padding: 14px 16px; font-size: 0.8rem; text-transform: uppercase; color: var(--text-muted); font-weight: 600;">Usuń</th>
            </tr>
          </thead>
          <tbody>
            {#each sortedGroups as group}
              <tr>
                <td>
                  <span class="group-name"><strong>{group.name}</strong></span>
                  <span class="gid-tag mono-val">GID: {group.gid}</span>
                </td>
                <td class="members-cell mono-val">
                  {group.members.length > 0 ? group.members.join(', ') : '(brak)'}
                </td>
                <td class="actions-cell">
                  {#if group.gid >= 1000}
                    <button class="btn-table danger-text" onclick={() => deleteGroup(group.name)} title="Usuń grupę">
                      <Trash2 size={14} />
                    </button>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>
  </div>

  <!-- Sudo Password Prompt Modal -->
  {#if showSudoModal}
    <div class="modal-overlay">
      <div class="modal-content glass">
        <div class="modal-header-icon">
          <KeyRound size={32} class="accent-amber-text" />
        </div>
        <h3>Wymagane uwierzytelnienie Sudo</h3>
        <p class="modal-desc">Ta operacja wymaga uprawnień roota. Wprowadź swoje hasło użytkownika (sudo):</p>
        <input 
          type="password" 
          placeholder="Wpisz hasło..." 
          bind:value={sudoPassword} 
          onkeydown={(e) => e.key === 'Enter' && submitSudoPassword()}
        />
        {#if sudoError}
          <span class="error-text">{sudoError}</span>
        {/if}
        <div class="modal-actions">
          <button class="primary" onclick={submitSudoPassword}>Zatwierdź</button>
          <button class="secondary" onclick={() => { showSudoModal = false; sudoPassword = ''; pendingAction = null; }}>Anuluj</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Modal Nowy Użytkownik -->
  {#if showCreateUserModal}
    <div class="modal-overlay">
      <div class="modal-content glass">
        <h3>Utwórz nowego użytkownika</h3>
        
        <div class="form-group">
          <label for="new-username">Nazwa użytkownika</label>
          <input id="new-username" type="text" placeholder="jan" bind:value={newUsername} />
        </div>

        <div class="form-group">
          <label for="new-user-shell">Domyślna powłoka</label>
          <select id="new-user-shell" bind:value={newUserShell}>
            <option value="/bin/bash">/bin/bash (Bash)</option>
            <option value="/bin/sh">/bin/sh (Standard Sh)</option>
            <option value="/usr/bin/zsh">/usr/bin/zsh (Zsh)</option>
            <option value="/usr/sbin/nologin">/usr/sbin/nologin (Bez logowania)</option>
          </select>
        </div>

        <div class="form-group">
          <label for="new-user-home">Katalog domowy (Opcjonalnie)</label>
          <input id="new-user-home" type="text" placeholder="Automatyczny (/home/nazwa)" bind:value={newUserHome} />
        </div>

        <div class="modal-actions">
          <button class="primary" onclick={createUser} disabled={!newUsername}>Utwórz użytkownika</button>
          <button class="secondary" onclick={() => { showCreateUserModal = false; newUsername = ''; }}>Anuluj</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Modal Nowa Grupa -->
  {#if showCreateGroupModal}
    <div class="modal-overlay">
      <div class="modal-content glass">
        <h3>Utwórz nową grupę</h3>
        <input type="text" placeholder="Nazwa grupy (np. developers)" bind:value={newGroupName} />
        <div class="modal-actions">
          <button class="primary" onclick={createGroup} disabled={!newGroupName}>Utwórz</button>
          <button class="secondary" onclick={() => { showCreateGroupModal = false; newGroupName = ''; }}>Anuluj</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Modal Zmiana Hasła -->
  {#if showChangePassModal}
    <div class="modal-overlay">
      <div class="modal-content glass">
        <h3>Zmień hasło dla <strong>{targetUser?.username}</strong></h3>
        <input type="password" placeholder="Wpisz nowe hasło..." bind:value={targetPassword} />
        <div class="modal-actions">
          <button class="primary" onclick={changePassword} disabled={!targetPassword}>Zmień hasło</button>
          <button class="secondary" onclick={() => { showChangePassModal = false; targetPassword = ''; targetUser = null; }}>Anuluj</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Modal Zarządzania Grupami Użytkownika -->
  {#if showGroupsModal}
    <div class="modal-overlay">
      <div class="modal-content glass groups-select-modal">
        <h3>Grupy użytkownika <strong>{targetUser?.username}</strong></h3>
        <p class="modal-desc">Zaznacz grupy, do których ma należeć użytkownik:</p>
        <div class="groups-checkbox-list">
          {#each groups.filter(g => g.gid >= 1000 || g.name === 'sudo' || g.name === 'docker') as group}
            <label class="group-checkbox-item">
              <input 
                type="checkbox" 
                value={group.name} 
                checked={selectedUserGroups.includes(group.name)} 
                onchange={(e) => {
                  const target = e.target as HTMLInputElement;
                  if (target.checked) {
                    selectedUserGroups = [...selectedUserGroups, group.name];
                  } else {
                    selectedUserGroups = selectedUserGroups.filter(g => g !== group.name);
                  }
                }}
              />
              <span>{group.name} (GID: {group.gid})</span>
            </label>
          {/each}
        </div>
        <div class="modal-actions">
          <button class="primary" onclick={saveUserGroups}>Zapisz grupy</button>
          <button class="secondary" onclick={() => { showGroupsModal = false; targetUser = null; }}>Anuluj</button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .user-manager {
    /* uses .manager-shell */
  }

  .error-badge {
    background: var(--accent-red-glow);
    border: 1px solid rgba(244, 63, 94, 0.3);
    padding: 8px 16px;
    border-radius: var(--radius-sm);
    color: #ff8595;
    font-size: 0.85rem;
  }

  /* Ops Bar */
  .ops-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border-radius: var(--radius-md);
    flex-shrink: 0;
  }

  .toggle-checkbox {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.85rem;
    color: var(--text-secondary);
    cursor: pointer;
    flex: 1;
  }

  /* Split View */
  .split-view {
    display: grid;
    grid-template-columns: 1.5fr 1fr;
    gap: 24px;
    flex: 1;
    overflow: hidden;
  }

  .table-container {
    display: flex;
    flex-direction: column;
    padding: 24px;
    border-radius: var(--radius-md);
    overflow: hidden;
  }

  .table-container h3 {
    font-size: 1.1rem;
    color: white;
    border-bottom: 1px solid var(--border-color);
    padding-bottom: 12px;
    margin-bottom: 16px;
    flex-shrink: 0;
  }

  .users-table, .groups-table {
    width: 100%;
    border-collapse: collapse;
    text-align: left;
  }

  .users-table th, .users-table td,
  .groups-table th, .groups-table td {
    padding: 12px 16px;
    border-bottom: 1px solid var(--border-color);
  }

  .users-table th, .groups-table th {
    font-size: 0.75rem;
    text-transform: uppercase;
    color: var(--text-muted);
    font-weight: 600;
    letter-spacing: 0.05em;
    position: sticky;
    top: 0;
    background: var(--bg-secondary);
    z-index: 1;
  }

  .users-table tr, .groups-table tr {
    transition: var(--transition-fast);
  }

  .users-table tr:hover, .groups-table tr:hover {
    background: rgba(255, 255, 255, 0.02);
  }

  .user-name, .group-name {
    display: block;
    color: white;
    font-size: 0.95rem;
  }

  .uid-tag, .gid-tag {
    font-size: 0.75rem;
    color: var(--text-muted);
    margin-top: 2px;
    display: inline-block;
  }

  .home-dir {
    display: block;
    font-size: 0.85rem;
    color: var(--text-secondary);
  }

  .shell-code {
    font-size: 0.75rem;
    color: var(--accent-amber);
    margin-top: 2px;
    display: inline-block;
  }

  .members-cell {
    font-size: 0.85rem;
    color: var(--text-secondary);
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .actions-cell {
    text-align: right;
    display: flex;
    justify-content: flex-end;
    gap: 6px;
  }

  .btn-table {
    background: transparent;
    border: none;
    padding: 6px;
    border-radius: 4px;
    color: var(--text-secondary);
    cursor: pointer;
    transition: var(--transition-fast);
  }

  .btn-table:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .danger-text:hover {
    color: var(--accent-red) !important;
    background: var(--accent-red-glow) !important;
  }

  .loading-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 16px;
    color: var(--text-secondary);
  }

  .spin {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  /* Modals */
  .modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .modal-content {
    width: 400px;
    padding: 24px;
    border-radius: var(--radius-md);
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .groups-select-modal {
    width: 450px;
  }

  .modal-desc {
    font-size: 0.9rem;
    color: var(--text-secondary);
    line-height: 1.4;
  }

  .groups-checkbox-list {
    max-height: 250px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 8px;
    border: 1px solid var(--border-color);
    padding: 12px;
    border-radius: var(--radius-sm);
    background: var(--bg-input);
  }

  .group-checkbox-item {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 0.9rem;
    color: var(--text-primary);
    cursor: pointer;
    padding: 6px;
    border-radius: 4px;
    transition: var(--transition-fast);
  }

  .group-checkbox-item:hover {
    background: var(--bg-hover);
  }

  .group-checkbox-item input {
    width: auto;
  }

  .error-text {
    color: var(--accent-red);
    font-size: 0.8rem;
    margin-top: -8px;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .form-group label {
    font-size: 0.8rem;
    color: var(--text-secondary);
    font-weight: 500;
  }
</style>
