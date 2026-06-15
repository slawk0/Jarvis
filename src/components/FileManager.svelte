<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { 
    Folder, 
    File, 
    ArrowLeft, 
    RefreshCw, 
    Plus, 
    Trash2, 
    Edit, 
    CornerDownLeft, 
    Save, 
    X,
    FileCode,
    Download,
    Upload,
    KeyRound
  } from 'lucide-svelte';

  let currentPath = $state('');
  let lastLoadedPath = $state('');
  let files = $state<any[]>([]);
  let isLoading = $state(false);
  let errorMsg = $state('');

  // Stan edytora tekstu
  let editingFile = $state<string | null>(null);
  let editingContent = $state('');
  let editorElement: HTMLDivElement | null = $state(null);
  let editorInstance: any = null;
  let editorSaveStatus = $state<'saved' | 'saving' | 'dirty' | 'error'>('saved');
  let autoSaveEnabled = $state(true);
  let saveTimeout: any = null;

  // Modale i kreatory
  let showNewFileModal = $state(false);
  let showNewDirModal = $state(false);
  let showRenameModal = $state(false);
  let newItemName = $state('');
  let uploadInput = $state<HTMLInputElement | null>(null);
  let selectedFile = $state<any>(null);

  // Menu kontekstowe
  let showContextMenu = $state(false);
  let contextMenuX = $state(0);
  let contextMenuY = $state(0);
  let contextMenuFile = $state<any>(null);

  // Modal uprawnień
  let showPermModal = $state(false);
  let permFile = $state<any>(null);
  let permOwnerRead = $state(false);
  let permOwnerWrite = $state(false);
  let permOwnerExec = $state(false);
  let permGroupRead = $state(false);
  let permGroupWrite = $state(false);
  let permGroupExec = $state(false);
  let permOthersRead = $state(false);
  let permOthersWrite = $state(false);
  let permOthersExec = $state(false);
  let permOctal = $state('644');
  let permRecursive = $state(false);

  function formatBytes(bytes: number) {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
  }

  function formatPermissions(perm: number | null) {
    if (perm === null) return '---';
    // Formatowanie octal (np. 16877 -> 755)
    return (perm & 0o777).toString(8);
  }

  async function loadDirectory() {
    isLoading = true;
    errorMsg = '';
    try {
      const result: any = await invoke('sftp_list', { path: currentPath });
      files = result;
      lastLoadedPath = currentPath;
    } catch (err: any) {
      errorMsg = 'Nie można odczytać katalogu: ' + err.toString();
      currentPath = lastLoadedPath; // Reset pola ścieżki do ostatnio poprawnie wczytanej
    } finally {
      isLoading = false;
    }
  }

  async function handleFolderClick(name: string) {
    const separator = lastLoadedPath.endsWith('/') ? '' : '/';
    currentPath = `${lastLoadedPath}${separator}${name}`;
    await loadDirectory();
  }

  async function handleGoBack() {
    if (lastLoadedPath === '/' || lastLoadedPath === '') return;
    const parts = lastLoadedPath.split('/');
    parts.pop();
    currentPath = parts.join('/') || '/';
    await loadDirectory();
  }

  async function handleRefresh() {
    await loadDirectory();
  }

  // Funkcje Monaco Editor
  function detectLanguage(fileName: string): string {
    const ext = fileName.split('.').pop()?.toLowerCase();
    switch (ext) {
      case 'js': return 'javascript';
      case 'ts': return 'typescript';
      case 'json': return 'json';
      case 'html': return 'html';
      case 'css': return 'css';
      case 'sh':
      case 'bash': return 'shell';
      case 'yaml':
      case 'yml': return 'yaml';
      case 'conf':
      case 'config':
      case 'nginx': return 'nginx';
      case 'py': return 'python';
      case 'md': return 'markdown';
      default: return 'plaintext';
    }
  }

  async function handleEditFile(file: any) {
    isLoading = true;
    errorMsg = '';
    const separator = lastLoadedPath.endsWith('/') ? '' : '/';
    const filePath = `${lastLoadedPath}${separator}${file.name}`;
    
    try {
      const content: string = await invoke('sftp_read', { path: filePath });
      editingFile = filePath;
      editingContent = content;
      editorSaveStatus = 'saved';

      // Inicjalizacja Monaco Editor
      setTimeout(() => {
        if (editorElement) {
          // Ustawienie środowiska Monaco do pobierania workerów inline (nieblokująca kompilacja)
          (window as any).MonacoEnvironment = {
            getWorkerUrl: function () {
              return `data:text/javascript;charset=utf-8,${encodeURIComponent(`
                self.MonacoEnvironment = {
                  baseUrl: 'https://cdnjs.cloudflare.com/ajax/libs/monaco-editor/0.43.0/min/'
                };
                importScripts('https://cdnjs.cloudflare.com/ajax/libs/monaco-editor/0.43.0/min/vs/base/worker/workerMain.js');
              `)}`;
            }
          };

          // Dynamiczny import monaco-editor
          import('monaco-editor').then((monaco) => {
            if (editorInstance) {
              editorInstance.dispose();
            }
            editorInstance = monaco.editor.create(editorElement!, {
              value: content,
              language: detectLanguage(file.name),
              theme: 'vs-dark',
              automaticLayout: true,
              fontSize: 14,
              fontFamily: '"JetBrains Mono", Consolas, monospace',
              minimap: { enabled: false },
            });

            // Monitor zmian dla auto-zapisu
            editorInstance.onDidChangeModelContent(() => {
              if (!autoSaveEnabled) {
                editorSaveStatus = 'dirty';
                return;
              }
              editorSaveStatus = 'dirty';
              if (saveTimeout) clearTimeout(saveTimeout);
              
              saveTimeout = setTimeout(async () => {
                if (!editingFile) return;
                editorSaveStatus = 'saving';
                try {
                  const currentVal = editorInstance.getValue();
                  await invoke('sftp_write', { path: editingFile, content: currentVal });
                  editorSaveStatus = 'saved';
                } catch (err: any) {
                  editorSaveStatus = 'error';
                  errorMsg = 'Błąd automatycznego zapisu: ' + err.toString();
                }
              }, 1500);
            });
          });
        }
      }, 100);

    } catch (err: any) {
      errorMsg = 'Błąd odczytu pliku: ' + err.toString();
    } finally {
      isLoading = false;
    }
  }

  async function saveFile() {
    if (!editingFile || !editorInstance) return;
    isLoading = true;
    errorMsg = '';
    const content = editorInstance.getValue();
    try {
      await invoke('sftp_write', { path: editingFile, content });
      editorSaveStatus = 'saved';
      await loadDirectory();
    } catch (err: any) {
      errorMsg = 'Błąd zapisu pliku: ' + err.toString();
      editorSaveStatus = 'error';
    } finally {
      isLoading = false;
    }
  }

  function closeEditor() {
    if (saveTimeout) {
      clearTimeout(saveTimeout);
      saveTimeout = null;
    }
    editingFile = null;
    if (editorInstance) {
      editorInstance.dispose();
      editorInstance = null;
    }
  }

  // CRUD pliki
  async function createFile() {
    if (!newItemName) return;
    const separator = lastLoadedPath.endsWith('/') ? '' : '/';
    const filePath = `${lastLoadedPath}${separator}${newItemName}`;
    try {
      await invoke('sftp_write', { path: filePath, content: '' });
      showNewFileModal = false;
      newItemName = '';
      await loadDirectory();
    } catch (err: any) {
      errorMsg = 'Błąd tworzenia pliku: ' + err.toString();
    }
  }

  async function createDirectory() {
    if (!newItemName) return;
    const separator = lastLoadedPath.endsWith('/') ? '' : '/';
    const dirPath = `${lastLoadedPath}${separator}${newItemName}`;
    try {
      await invoke('sftp_create_dir', { path: dirPath });
      showNewDirModal = false;
      newItemName = '';
      await loadDirectory();
    } catch (err: any) {
      errorMsg = 'Błąd tworzenia folderu: ' + err.toString();
    }
  }

  async function renameItem() {
    if (!newItemName || !selectedFile) return;
    const separator = lastLoadedPath.endsWith('/') ? '' : '/';
    const src = `${lastLoadedPath}${separator}${selectedFile.name}`;
    const dest = `${lastLoadedPath}${separator}${newItemName}`;
    try {
      await invoke('sftp_rename', { src, dest });
      showRenameModal = false;
      newItemName = '';
      selectedFile = null;
      await loadDirectory();
    } catch (err: any) {
      errorMsg = 'Błąd zmiany nazwy: ' + err.toString();
    }
  }

  async function deleteItem(file: any) {
    const separator = lastLoadedPath.endsWith('/') ? '' : '/';
    const path = `${lastLoadedPath}${separator}${file.name}`;
    if (confirm(`Czy na pewno chcesz usunąć ${file.is_dir ? 'katalog' : 'plik'} "${file.name}"?`)) {
      try {
        await invoke('sftp_delete', { path, isDir: file.is_dir });
        await loadDirectory();
      } catch (err: any) {
        errorMsg = 'Błąd usuwania: ' + err.toString();
      }
    }
  }

  // Obsługa pobierania
  async function downloadFile(file: any) {
    const separator = lastLoadedPath.endsWith('/') ? '' : '/';
    const path = `${lastLoadedPath}${separator}${file.name}`;
    try {
      const content: string = await invoke('sftp_read', { path });
      
      // Standardowy Web-download
      const blob = new Blob([content], { type: 'text/plain;charset=utf-8' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = file.name;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
    } catch (err: any) {
      errorMsg = 'Błąd pobierania pliku: ' + err.toString();
    }
  }

  // Obsługa wysyłania (Upload)
  function handleUpload(event: Event) {
    const input = event.target as HTMLInputElement;
    if (!input.files || input.files.length === 0) return;
    
    const file = input.files[0];
    const reader = new FileReader();
    reader.onload = async (e) => {
      const content = e.target?.result as string;
      const separator = lastLoadedPath.endsWith('/') ? '' : '/';
      const filePath = `${lastLoadedPath}${separator}${file.name}`;
      isLoading = true;
      try {
        await invoke('sftp_write', { path: filePath, content });
        await loadDirectory();
      } catch (err: any) {
        errorMsg = 'Błąd wysyłania pliku: ' + err.toString();
      } finally {
        isLoading = false;
      }
    };
    reader.readAsText(file);
    input.value = '';
  }

  function handleContextMenu(event: MouseEvent, file: any) {
    event.preventDefault();
    contextMenuFile = file;
    contextMenuX = event.clientX;
    contextMenuY = event.clientY;
    showContextMenu = true;
  }

  function closeContextMenu() {
    showContextMenu = false;
  }

  function updateOctalFromCheckboxes() {
    let owner = (permOwnerRead ? 4 : 0) + (permOwnerWrite ? 2 : 0) + (permOwnerExec ? 1 : 0);
    let group = (permGroupRead ? 4 : 0) + (permGroupWrite ? 2 : 0) + (permGroupExec ? 1 : 0);
    let others = (permOthersRead ? 4 : 0) + (permOthersWrite ? 2 : 0) + (permOthersExec ? 1 : 0);
    permOctal = `${owner}${group}${others}`;
  }

  function updateCheckboxesFromOctal(val: string) {
    if (!val || val.length < 3) return;
    const clean = val.slice(-3);
    if (!/^[0-7]{3}$/.test(clean)) return;
    const owner = parseInt(clean[0], 10);
    const group = parseInt(clean[1], 10);
    const others = parseInt(clean[2], 10);

    permOwnerRead = (owner & 4) !== 0;
    permOwnerWrite = (owner & 2) !== 0;
    permOwnerExec = (owner & 1) !== 0;

    permGroupRead = (group & 4) !== 0;
    permGroupWrite = (group & 2) !== 0;
    permGroupExec = (group & 1) !== 0;

    permOthersRead = (others & 4) !== 0;
    permOthersWrite = (others & 2) !== 0;
    permOthersExec = (others & 1) !== 0;
  }

  function openPermissionsModal(file: any) {
    permFile = file;
    const initialPerm = file.permissions;
    if (initialPerm !== null) {
      const octVal = (initialPerm & 0o777).toString(8);
      permOctal = octVal.padStart(3, '0');
    } else {
      permOctal = '644';
    }
    updateCheckboxesFromOctal(permOctal);
    permRecursive = false;
    showPermModal = true;
    showContextMenu = false;
  }

  async function savePermissions() {
    if (!permFile) return;
    isLoading = true;
    errorMsg = '';
    const separator = lastLoadedPath.endsWith('/') ? '' : '/';
    const filePath = `${lastLoadedPath}${separator}${permFile.name}`;
    const escapedPath = "'" + filePath.replace(/'/g, "'\\''") + "'";
    const recFlag = (permRecursive && permFile.is_dir) ? '-R ' : '';
    const cmd = `chmod ${recFlag}${permOctal} ${escapedPath}`;

    try {
      await invoke('exec_custom_command', { cmd, useSudo: false });
      showPermModal = false;
      await loadDirectory();
    } catch (err: any) {
      const errStr = err.toString();
      if (errStr.includes('SUDO_PASSWORD_REQUIRED') || errStr.toLowerCase().includes('permission denied')) {
        try {
          const hasSudo: boolean = await invoke('has_sudo_password');
          if (hasSudo) {
            await invoke('exec_custom_command', { cmd, useSudo: true });
            showPermModal = false;
            await loadDirectory();
            isLoading = false;
            return;
          }
        } catch { /* ignore */ }

        const sudoPass = prompt('Wymagane hasło sudo do zmiany uprawnień:');
        if (sudoPass) {
          try {
            await invoke('set_sudo_password', { password: sudoPass });
            await invoke('exec_custom_command', { cmd, useSudo: true });
            showPermModal = false;
            await loadDirectory();
          } catch (sudoErr: any) {
            errorMsg = 'Błąd hasła sudo lub wykonania: ' + sudoErr.toString();
          }
        } else {
          errorMsg = 'Błąd: wymagane hasło sudo';
        }
      } else {
        errorMsg = 'Błąd zmiany uprawnień: ' + err.toString();
      }
    } finally {
      isLoading = false;
    }
  }

  onMount(async () => {
    try {
      const home: string = await invoke('sftp_get_home_dir');
      currentPath = home;
      lastLoadedPath = home;
    } catch (err: any) {
      console.warn("Failed to get home dir, falling back to /", err);
      currentPath = '/';
      lastLoadedPath = '/';
    }
    loadDirectory();
  });

  onDestroy(() => {
    if (editorInstance) {
      editorInstance.dispose();
    }
    if (saveTimeout) {
      clearTimeout(saveTimeout);
    }
  });
</script>

<div class="file-manager fade-in">
  {#if editingFile}
    <!-- Ekran edytora plików -->
    <div class="editor-view">
      <header class="editor-header">
        <div class="editor-title">
          <FileCode size={18} class="accent-amber-text" />
          <span>Edytujesz: <strong>{editingFile.split('/').pop()}</strong></span>
          <span class="path-badge mono-val">{editingFile}</span>
          
          {#if editorSaveStatus === 'saved'}
            <span class="save-status-badge saved">● Zapisano</span>
          {:else if editorSaveStatus === 'saving'}
            <span class="save-status-badge saving">● Zapisywanie...</span>
          {:else if editorSaveStatus === 'dirty'}
            <span class="save-status-badge dirty">● Niezapisane zmiany</span>
          {:else if editorSaveStatus === 'error'}
            <span class="save-status-badge error">● Błąd zapisu</span>
          {/if}
        </div>
        <div class="editor-actions">
          <label class="auto-save-toggle">
            <input type="checkbox" bind:checked={autoSaveEnabled} />
            <span>Auto-zapis</span>
          </label>
          <button class="primary" onclick={saveFile} disabled={isLoading || editorSaveStatus === 'saving'}>
            <Save size={16} /> Zapisz
          </button>
          <button class="secondary" onclick={closeEditor}>
            <X size={16} /> Zamknij
          </button>
        </div>
      </header>
      <div bind:this={editorElement} class="editor-container"></div>
    </div>
  {:else}
    <!-- Ekran przeglądarki plików -->
    <header class="fm-header">
      <div class="title-area">
        <h1>Menedżer Plików (SFTP)</h1>
        <p class="subtitle">Zarządzaj strukturą plików na serwerze Linux</p>
      </div>
      {#if errorMsg}
        <div class="error-badge">{errorMsg}</div>
      {/if}
    </header>

    <!-- Pasek nawigacji ścieżki -->
    <div class="nav-bar glass">
      <button class="secondary btn-icon" onclick={handleGoBack} disabled={currentPath === '/'}>
        <ArrowLeft size={16} />
      </button>
      <input 
        type="text" 
        class="path-input mono-val" 
        bind:value={currentPath} 
        onkeydown={(e) => e.key === 'Enter' && loadDirectory()} 
      />
      <button class="secondary btn-icon" onclick={handleRefresh} disabled={isLoading}>
        <RefreshCw size={16} class={isLoading ? 'spin' : ''} />
      </button>
      
      <div class="actions-group">
        <input bind:this={uploadInput} type="file" hidden onchange={handleUpload} />
        <button class="secondary" onclick={() => uploadInput?.click()}>
          <Upload size={16} /> Wyślij plik
        </button>
        <button class="secondary" onclick={() => showNewFileModal = true}>
          <Plus size={16} /> Nowy plik
        </button>
        <button class="secondary" onclick={() => showNewDirModal = true}>
          <Plus size={16} /> Nowy folder
        </button>
      </div>
    </div>

    <!-- Lista plików -->
    <div class="files-table-container glass">
      {#if isLoading && files.length === 0}
        <div class="loading-state">
          <RefreshCw class="spin" size={32} />
          <p>Ładowanie struktury plików...</p>
        </div>
      {:else}
        <table class="files-table">
          <thead>
            <tr>
              <th style="width: 40%;">Nazwa</th>
              <th style="width: 15%;">Rozmiar</th>
              <th style="width: 15%;">Uprawnienia</th>
              <th style="width: 30%; text-align: right;">Akcje</th>
            </tr>
          </thead>
          <tbody>
            {#if currentPath !== '/'}
              <tr class="folder-row" onclick={handleGoBack}>
                <td class="file-name-cell">
                  <Folder size={18} class="folder-icon" />
                  <span>.. (Wróć)</span>
                </td>
                <td>--</td>
                <td>--</td>
                <td></td>
              </tr>
            {/if}

            {#each files as file}
              <tr class={file.is_dir ? 'folder-row' : 'file-row'} oncontextmenu={(e) => handleContextMenu(e, file)}>
                <td 
                  class="file-name-cell" 
                  onclick={() => file.is_dir ? handleFolderClick(file.name) : handleEditFile(file)}
                >
                  {#if file.is_dir}
                    <Folder size={18} class="folder-icon" />
                  {:else}
                    <File size={18} class="file-icon" />
                  {/if}
                  <span>{file.name}</span>
                </td>
                <td class="mono-val">{file.is_dir ? '--' : formatBytes(file.size)}</td>
                <td><span class="badge warning">{formatPermissions(file.permissions)}</span></td>
                <td class="actions-cell">
                  {#if !file.is_dir}
                    <button class="btn-table" onclick={() => handleEditFile(file)} title="Edytuj">
                      <Edit size={14} />
                    </button>
                    <button class="btn-table" onclick={() => downloadFile(file)} title="Pobierz">
                      <Download size={14} />
                    </button>
                  {/if}
                  <button 
                    class="btn-table" 
                    onclick={() => { 
                      selectedFile = file; 
                      newItemName = file.name; 
                      showRenameModal = true; 
                    }} 
                    title="Zmień nazwę"
                  >
                    <CornerDownLeft size={14} />
                  </button>
                  <button class="btn-table danger-text" onclick={() => deleteItem(file)} title="Usuń">
                    <Trash2 size={14} />
                  </button>
                </td>
              </tr>
            {/each}

            {#if files.length === 0 && !isLoading}
              <tr>
                <td colspan="4" class="empty-state">Katalog jest pusty</td>
              </tr>
            {/if}
          </tbody>
        </table>
      {/if}
    </div>
  {/if}
</div>

<svelte:window onclick={closeContextMenu} />

  <!-- Menu Kontekstowe -->
  {#if showContextMenu}
    <div 
      class="context-menu glass" 
      style="top: {contextMenuY}px; left: {contextMenuX}px;"
      onclick={(e) => e.stopPropagation()}
    >
      {#if !contextMenuFile.is_dir}
        <button class="menu-item" onclick={() => { handleEditFile(contextMenuFile); closeContextMenu(); }}>
          <Edit size={14} /> <span>Edytuj plik</span>
        </button>
        <button class="menu-item" onclick={() => { downloadFile(contextMenuFile); closeContextMenu(); }}>
          <Download size={14} /> <span>Pobierz</span>
        </button>
      {/if}
      <button class="menu-item" onclick={() => openPermissionsModal(contextMenuFile)}>
        <KeyRound size={14} class="accent-amber-text" /> <span>Uprawnienia (chmod)</span>
      </button>
      <button class="menu-item" onclick={() => { 
        selectedFile = contextMenuFile; 
        newItemName = contextMenuFile.name; 
        showRenameModal = true; 
        closeContextMenu(); 
      }}>
        <CornerDownLeft size={14} /> <span>Zmień nazwę</span>
      </button>
      <hr class="menu-divider" />
      <button class="menu-item danger-text" onclick={() => { deleteItem(contextMenuFile); closeContextMenu(); }}>
        <Trash2 size={14} /> <span>Usuń</span>
      </button>
    </div>
  {/if}

  <!-- Modal Uprawnienia -->
  {#if showPermModal}
    <div class="modal-overlay">
      <div class="modal-content glass perm-modal">
        <h3>Uprawnienia dla: <span class="mono-val" style="color: var(--accent-amber);">{permFile?.name}</span></h3>
        
        <div class="perm-grid">
          <div class="perm-col">
            <h4>Właściciel</h4>
            <label><input type="checkbox" bind:checked={permOwnerRead} onchange={updateOctalFromCheckboxes} /> Odczyt (r)</label>
            <label><input type="checkbox" bind:checked={permOwnerWrite} onchange={updateOctalFromCheckboxes} /> Zapis (w)</label>
            <label><input type="checkbox" bind:checked={permOwnerExec} onchange={updateOctalFromCheckboxes} /> Wykonanie (x)</label>
          </div>
          
          <div class="perm-col">
            <h4>Grupa</h4>
            <label><input type="checkbox" bind:checked={permGroupRead} onchange={updateOctalFromCheckboxes} /> Odczyt (r)</label>
            <label><input type="checkbox" bind:checked={permGroupWrite} onchange={updateOctalFromCheckboxes} /> Zapis (w)</label>
            <label><input type="checkbox" bind:checked={permGroupExec} onchange={updateOctalFromCheckboxes} /> Wykonanie (x)</label>
          </div>
          
          <div class="perm-col">
            <h4>Inni</h4>
            <label><input type="checkbox" bind:checked={permOthersRead} onchange={updateOctalFromCheckboxes} /> Odczyt (r)</label>
            <label><input type="checkbox" bind:checked={permOthersWrite} onchange={updateOctalFromCheckboxes} /> Zapis (w)</label>
            <label><input type="checkbox" bind:checked={permOthersExec} onchange={updateOctalFromCheckboxes} /> Wykonanie (x)</label>
          </div>
        </div>

        <div class="form-group octal-group">
          <label for="octal-input">Wartość oktalna</label>
          <input 
            id="octal-input"
            type="text" 
            maxlength="4" 
            bind:value={permOctal} 
            oninput={(e: any) => updateCheckboxesFromOctal(e.target.value)}
            class="mono-val"
          />
        </div>

        {#if permFile?.is_dir}
          <label class="recursive-label">
            <input type="checkbox" bind:checked={permRecursive} />
            <span>Zastosuj rekurencyjnie (chmod -R)</span>
          </label>
        {/if}

        <div class="modal-actions">
          <button class="primary" onclick={savePermissions} disabled={isLoading}>
            {#if isLoading}
              <RefreshCw size={14} class="spin" /> Zapisywanie...
            {:else}
              Zapisz
            {/if}
          </button>
          <button class="secondary" onclick={() => showPermModal = false}>Anuluj</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Modal Nowy Plik -->
  {#if showNewFileModal}
    <div class="modal-overlay">
      <div class="modal-content glass">
        <h3>Utwórz nowy plik</h3>
        <input type="text" placeholder="Nazwa pliku (np. config.json)" bind:value={newItemName} />
        <div class="modal-actions">
          <button class="primary" onclick={createFile}>Utwórz</button>
          <button class="secondary" onclick={() => { showNewFileModal = false; newItemName = ''; }}>Anuluj</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Modal Nowy Folder -->
  {#if showNewDirModal}
    <div class="modal-overlay">
      <div class="modal-content glass">
        <h3>Utwórz nowy folder</h3>
        <input type="text" placeholder="Nazwa folderu" bind:value={newItemName} />
        <div class="modal-actions">
          <button class="primary" onclick={createDirectory}>Utwórz</button>
          <button class="secondary" onclick={() => { showNewDirModal = false; newItemName = ''; }}>Anuluj</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Modal Zmiana Nazwy -->
  {#if showRenameModal}
    <div class="modal-overlay">
      <div class="modal-content glass">
        <h3>Zmień nazwę / Przenieś</h3>
        <input type="text" placeholder="Nowa nazwa" bind:value={newItemName} />
        <div class="modal-actions">
          <button class="primary" onclick={renameItem}>Zmień</button>
          <button class="secondary" onclick={() => { showRenameModal = false; newItemName = ''; selectedFile = null; }}>Anuluj</button>
        </div>
      </div>
    </div>
  {/if}

<style>
  .file-manager {
    padding: 30px;
    display: flex;
    flex-direction: column;
    gap: 24px;
    height: 100%;
    overflow: hidden;
  }

  .fm-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-shrink: 0;
  }

  .title-area h1 {
    font-size: 2rem;
    color: white;
  }

  .subtitle {
    color: var(--text-secondary);
    font-size: 0.9rem;
    margin-top: 4px;
  }

  .error-badge {
    background: var(--accent-red-glow);
    border: 1px solid rgba(244, 63, 94, 0.3);
    padding: 8px 16px;
    border-radius: var(--radius-sm);
    color: #ff8595;
    font-size: 0.85rem;
  }

  /* Nav bar */
  .nav-bar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    border-radius: var(--radius-md);
    flex-shrink: 0;
  }

  .btn-icon {
    width: 38px;
    height: 38px;
    padding: 0;
  }

  .path-input {
    flex: 1;
    font-family: var(--font-mono);
    font-size: 0.85rem;
  }

  .actions-group {
    display: flex;
    gap: 8px;
  }

  /* Files table */
  .files-table-container {
    flex: 1;
    overflow-y: auto;
    border-radius: var(--radius-md);
  }

  .files-table {
    width: 100%;
    border-collapse: collapse;
    text-align: left;
  }

  .files-table th, .files-table td {
    padding: 14px 20px;
    border-bottom: 1px solid var(--border-color);
  }

  .files-table th {
    font-size: 0.8rem;
    text-transform: uppercase;
    color: var(--text-muted);
    font-weight: 600;
    letter-spacing: 0.05em;
    position: sticky;
    top: 0;
    background: var(--bg-secondary);
    z-index: 1;
  }

  .files-table tr {
    transition: var(--transition-fast);
  }

  .folder-row {
    cursor: pointer;
  }

  .folder-row:hover {
    background: var(--bg-hover);
  }

  .file-row:hover {
    background: rgba(255, 255, 255, 0.02);
  }

  .file-name-cell {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 0.95rem;
    color: var(--text-primary);
  }

  .folder-icon {
    color: var(--accent-amber);
  }

  .file-icon {
    color: var(--text-secondary);
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

  .empty-state {
    text-align: center;
    color: var(--text-muted);
    font-size: 0.9rem;
    padding: 40px !important;
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

  .modal-content h3 {
    color: white;
    font-size: 1.1rem;
  }

  .modal-content input {
    width: 100%;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }

  /* Editor View */
  .editor-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 100%;
  }

  .editor-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding-bottom: 16px;
    border-bottom: 1px solid var(--border-color);
    flex-shrink: 0;
  }

  .editor-title {
    display: flex;
    align-items: center;
    gap: 12px;
    color: white;
    font-size: 1rem;
  }

  .path-badge {
    background: var(--bg-hover);
    border: 1px solid var(--border-color);
    padding: 4px 10px;
    border-radius: 4px;
    font-family: var(--font-mono);
    font-size: 0.8rem;
    color: var(--text-secondary);
  }

  .editor-actions {
    display: flex;
    gap: 8px;
  }

  .editor-container {
    flex: 1;
    width: 100%;
    border-radius: var(--radius-sm);
    margin-top: 16px;
    overflow: hidden;
    border: 1px solid var(--border-color);
  }

  .save-status-badge {
    font-size: 0.8rem;
    padding: 4px 8px;
    border-radius: 4px;
    font-weight: 500;
  }
  .save-status-badge.saved {
    color: #10b981;
    background: rgba(16, 185, 129, 0.1);
  }
  .save-status-badge.saving {
    color: var(--accent-amber);
    background: var(--accent-amber-glow);
  }
  .save-status-badge.dirty {
    color: var(--accent-rust);
    background: var(--accent-rust-glow);
  }
  .save-status-badge.error {
    color: #ef4444;
    background: rgba(239, 68, 68, 0.1);
  }

  .auto-save-toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.85rem;
    color: var(--text-secondary);
    cursor: pointer;
    margin-right: 12px;
    user-select: none;
  }
  .auto-save-toggle input {
    width: 14px;
    height: 14px;
    cursor: pointer;
    margin: 0;
  }

  /* Context Menu */
  .context-menu {
    position: fixed;
    z-index: 1000;
    min-width: 180px;
    background: rgba(20, 20, 25, 0.85);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-md);
    box-shadow: 0 10px 25px rgba(0, 0, 0, 0.5);
    padding: 6px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .menu-item {
    background: transparent;
    border: none;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    font-size: 0.88rem;
    color: var(--text-secondary);
    border-radius: var(--radius-sm);
    cursor: pointer;
    text-align: left;
    transition: var(--transition-fast);
    width: 100%;
  }

  .menu-item:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .menu-item.danger-text:hover {
    background: var(--accent-red-glow) !important;
    color: var(--accent-red) !important;
  }

  .menu-divider {
    border: none;
    border-top: 1px solid var(--border-color);
    margin: 4px 0;
  }

  /* Permissions Modal */
  .perm-modal {
    width: 480px !important;
  }

  .perm-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 16px;
    margin: 10px 0 20px 0;
  }

  .perm-col {
    display: flex;
    flex-direction: column;
    gap: 10px;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    padding: 12px;
  }

  .perm-col h4 {
    font-size: 0.85rem;
    color: white;
    margin-bottom: 4px;
    border-bottom: 1px solid var(--border-color);
    padding-bottom: 6px;
  }

  .perm-col label {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.85rem;
    color: var(--text-secondary);
    cursor: pointer;
  }

  .perm-col input[type="checkbox"] {
    width: 14px;
    height: 14px;
    cursor: pointer;
  }

  .octal-group {
    margin-bottom: 16px;
  }

  .octal-group input {
    width: 100px;
    font-size: 1.1rem;
    text-align: center;
    letter-spacing: 0.1em;
  }

  .recursive-label {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.85rem;
    color: var(--text-secondary);
    cursor: pointer;
    margin-bottom: 8px;
  }

  .recursive-label input {
    width: 14px;
    height: 14px;
  }
</style>
