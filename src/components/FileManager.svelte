<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { Save, X, FileCode, RefreshCw } from 'lucide-svelte';
  import SftpFileBrowser from './sftp/SftpFileBrowser.svelte';
  import SftpTransferPanel from './sftp/SftpTransferPanel.svelte';
  import type { FileInfo } from '$lib/sftp/types';
  import { getRemotePath } from '$lib/sftp/pathUtils';
  import { registerBackHandler } from '$lib/backNavigation.svelte';

  let errorMsg = $state('');
  let browserPath = $state('/');
  let browserRef: SftpFileBrowser | undefined = $state();

  // Edytor Monaco
  let editingFile = $state<string | null>(null);
  let editorElement: HTMLDivElement | null = $state(null);
  let editorInstance: any = null;
  let editorSaveStatus = $state<'saved' | 'saving' | 'dirty' | 'error'>('saved');
  let autoSaveEnabled = $state(true);
  let saveTimeout: ReturnType<typeof setTimeout> | null = null;
  let isLoading = $state(false);

  // Modale
  let showNewFileModal = $state(false);
  let showNewDirModal = $state(false);
  let showRenameModal = $state(false);
  let newItemName = $state('');
  let selectedFile = $state<FileInfo | null>(null);

  // Modal uprawnień
  let showPermModal = $state(false);
  let permFile = $state<FileInfo | null>(null);
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

  const MAX_EDIT_BYTES = 5 * 1024 * 1024;

  function detectLanguage(fileName: string): string {
    const ext = fileName.split('.').pop()?.toLowerCase();
    switch (ext) {
      case 'js':
        return 'javascript';
      case 'ts':
        return 'typescript';
      case 'json':
        return 'json';
      case 'html':
        return 'html';
      case 'css':
        return 'css';
      case 'sh':
      case 'bash':
        return 'shell';
      case 'yaml':
      case 'yml':
        return 'yaml';
      case 'conf':
      case 'config':
      case 'nginx':
        return 'nginx';
      case 'py':
        return 'python';
      case 'md':
        return 'markdown';
      default:
        return 'plaintext';
    }
  }

  async function handleEdit(filePath: string, file: FileInfo) {
    if (!filePath) return;
    if (file.size > MAX_EDIT_BYTES) {
      errorMsg = `Plik jest za duży do edycji (${Math.round(file.size / 1024 / 1024)} MB). Użyj pobierania.`;
      return;
    }

    isLoading = true;
    errorMsg = '';
    try {
      const content: string = await invoke('sftp_read', { path: filePath });
      editingFile = filePath;
      editorSaveStatus = 'saved';

      setTimeout(() => {
        if (editorElement) {
          (window as any).MonacoEnvironment = {
            getWorkerUrl: function () {
              return `data:text/javascript;charset=utf-8,${encodeURIComponent(`
                self.MonacoEnvironment = {
                  baseUrl: 'https://cdnjs.cloudflare.com/ajax/libs/monaco-editor/0.43.0/min/'
                };
                importScripts('https://cdnjs.cloudflare.com/ajax/libs/monaco-editor/0.43.0/min/vs/base/worker/workerMain.js');
              `)}`;
            },
          };

          import('monaco-editor').then((monaco) => {
            if (editorInstance) editorInstance.dispose();
            editorInstance = monaco.editor.create(editorElement!, {
              value: content,
              language: detectLanguage(file.name),
              theme: 'vs-dark',
              automaticLayout: true,
              fontSize: 14,
              fontFamily: '"JetBrains Mono", Consolas, monospace',
              minimap: { enabled: false },
            });

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
                } catch (err: unknown) {
                  editorSaveStatus = 'error';
                  errorMsg = 'Błąd automatycznego zapisu: ' + String(err);
                }
              }, 1500);
            });
          });
        }
      }, 100);
    } catch (err: unknown) {
      errorMsg = 'Błąd odczytu pliku: ' + String(err);
    } finally {
      isLoading = false;
    }
  }

  async function saveFile() {
    if (!editingFile || !editorInstance) return;
    isLoading = true;
    errorMsg = '';
    try {
      await invoke('sftp_write', { path: editingFile, content: editorInstance.getValue() });
      editorSaveStatus = 'saved';
    } catch (err: unknown) {
      errorMsg = 'Błąd zapisu pliku: ' + String(err);
      editorSaveStatus = 'error';
    } finally {
      isLoading = false;
    }
  }

  function closeEditor() {
    if (saveTimeout) clearTimeout(saveTimeout);
    editingFile = null;
    if (editorInstance) {
      editorInstance.dispose();
      editorInstance = null;
    }
  }

  async function createFile() {
    if (!newItemName) return;
    const filePath = getRemotePath(browserPath, newItemName);
    try {
      await invoke('sftp_write', { path: filePath, content: '' });
      showNewFileModal = false;
      newItemName = '';
      await browserRef?.refresh();
    } catch (err: unknown) {
      errorMsg = 'Błąd tworzenia pliku: ' + String(err);
    }
  }

  async function createDirectory() {
    if (!newItemName) return;
    const dirPath = getRemotePath(browserPath, newItemName);
    try {
      await invoke('sftp_create_dir', { path: dirPath });
      showNewDirModal = false;
      newItemName = '';
      await browserRef?.refresh();
    } catch (err: unknown) {
      errorMsg = 'Błąd tworzenia folderu: ' + String(err);
    }
  }

  async function renameItem() {
    if (!newItemName || !selectedFile) return;
    const src = getRemotePath(browserPath, selectedFile.name);
    const dest = getRemotePath(browserPath, newItemName);
    try {
      await invoke('sftp_rename', { src, dest });
      showRenameModal = false;
      newItemName = '';
      selectedFile = null;
      await browserRef?.refresh();
    } catch (err: unknown) {
      errorMsg = 'Błąd zmiany nazwy: ' + String(err);
    }
  }

  function updateOctalFromCheckboxes() {
    const owner =
      (permOwnerRead ? 4 : 0) + (permOwnerWrite ? 2 : 0) + (permOwnerExec ? 1 : 0);
    const group =
      (permGroupRead ? 4 : 0) + (permGroupWrite ? 2 : 0) + (permGroupExec ? 1 : 0);
    const others =
      (permOthersRead ? 4 : 0) + (permOthersWrite ? 2 : 0) + (permOthersExec ? 1 : 0);
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

  function openPermissionsModal(file: FileInfo) {
    permFile = file;
    if (file.permissions !== null) {
      permOctal = (file.permissions & 0o777).toString(8).padStart(3, '0');
    } else {
      permOctal = '644';
    }
    updateCheckboxesFromOctal(permOctal);
    permRecursive = false;
    showPermModal = true;
  }

  async function savePermissions() {
    if (!permFile) return;
    isLoading = true;
    errorMsg = '';
    const filePath = getRemotePath(browserPath, permFile.name);
    const escapedPath = "'" + filePath.replace(/'/g, "'\\''") + "'";
    const recFlag = permRecursive && permFile.is_dir ? '-R ' : '';
    const cmd = `chmod ${recFlag}${permOctal} ${escapedPath}`;

    try {
      await invoke('exec_custom_command', { cmd, useSudo: false });
      showPermModal = false;
      await browserRef?.refresh();
    } catch (err: unknown) {
      const errStr = String(err);
      if (errStr.includes('SUDO_PASSWORD_REQUIRED') || errStr.toLowerCase().includes('permission denied')) {
        try {
          const hasSudo: boolean = await invoke('has_sudo_password');
          if (hasSudo) {
            await invoke('exec_custom_command', { cmd, useSudo: true });
            showPermModal = false;
            await browserRef?.refresh();
            isLoading = false;
            return;
          }
        } catch {
          /* ignore */
        }
        const sudoPass = prompt('Wymagane hasło sudo do zmiany uprawnień:');
        if (sudoPass) {
          try {
            await invoke('set_sudo_password', { password: sudoPass });
            await invoke('exec_custom_command', { cmd, useSudo: true });
            showPermModal = false;
            await browserRef?.refresh();
          } catch (sudoErr: unknown) {
            errorMsg = 'Błąd hasła sudo lub wykonania: ' + String(sudoErr);
          }
        } else {
          errorMsg = 'Błąd: wymagane hasło sudo';
        }
      } else {
        errorMsg = 'Błąd zmiany uprawnień: ' + errStr;
      }
    } finally {
      isLoading = false;
    }
  }

  onMount(() => {
    return registerBackHandler({
      id: 'file-manager',
      priority: 100,
      canGoBack: () =>
        showPermModal ||
        showRenameModal ||
        showNewFileModal ||
        showNewDirModal ||
        !!editingFile,
      goBack: () => {
        if (showPermModal) {
          showPermModal = false;
          return;
        }
        if (showRenameModal) {
          showRenameModal = false;
          newItemName = '';
          selectedFile = null;
          return;
        }
        if (showNewFileModal) {
          showNewFileModal = false;
          newItemName = '';
          return;
        }
        if (showNewDirModal) {
          showNewDirModal = false;
          newItemName = '';
          return;
        }
        if (editingFile) closeEditor();
      },
      label: () => {
        if (showPermModal) return 'Zamknij okno uprawnień';
        if (showRenameModal) return 'Anuluj zmianę nazwy';
        if (showNewFileModal) return 'Anuluj tworzenie pliku';
        if (showNewDirModal) return 'Anuluj tworzenie folderu';
        if (editingFile) return 'Zamknij edytor pliku';
        return 'Cofnij w menedżerze plików';
      },
    });
  });

  onDestroy(() => {
    if (editorInstance) editorInstance.dispose();
    if (saveTimeout) clearTimeout(saveTimeout);
  });
</script>

<div class="file-manager manager-shell fade-in">
  <div class="browser-layer" class:hidden={!!editingFile}>
    {#if errorMsg}
      <div class="error-badge">{errorMsg}</div>
    {/if}

    <SftpFileBrowser
      bind:this={browserRef}
      onEdit={handleEdit}
      onChmod={openPermissionsModal}
      onRename={(file) => {
        selectedFile = file;
        newItemName = file.name;
        showRenameModal = true;
      }}
      onNewFile={() => (showNewFileModal = true)}
      onNewDir={() => (showNewDirModal = true)}
      onError={(msg) => (errorMsg = msg)}
      onPathChange={(p) => (browserPath = p)}
    />
  </div>

  {#if editingFile}
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
  {/if}
</div>

<SftpTransferPanel />

{#if showPermModal && permFile}
  <div class="modal-overlay">
    <div class="modal-content glass perm-modal">
      <h3>Uprawnienia dla: <span class="mono-val accent-name">{permFile.name}</span></h3>
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
          oninput={(e) => updateCheckboxesFromOctal((e.target as HTMLInputElement).value)}
          class="mono-val"
        />
      </div>
      {#if permFile.is_dir}
        <label class="recursive-label">
          <input type="checkbox" bind:checked={permRecursive} />
          <span>Zastosuj rekurencyjnie (chmod -R)</span>
        </label>
      {/if}
      <div class="modal-actions">
        <button class="primary" onclick={savePermissions} disabled={isLoading}>
          {#if isLoading}<RefreshCw size={14} class="spin" /> Zapisywanie...{:else}Zapisz{/if}
        </button>
        <button class="secondary" onclick={() => (showPermModal = false)}>Anuluj</button>
      </div>
    </div>
  </div>
{/if}

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
    position: relative;
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 100%;
  }

  .browser-layer {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    width: 100%;
  }

  .browser-layer.hidden {
    display: none;
  }

  .error-badge {
    flex-shrink: 0;
    background: var(--accent-red-glow);
    border: 1px solid rgba(244, 63, 94, 0.3);
    padding: 8px 16px;
    border-radius: var(--radius-sm);
    color: #ff8595;
    font-size: 0.85rem;
    max-width: 50%;
  }

  .editor-view {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 100%;
    background: var(--bg-primary, #0f1117);
    z-index: 1;
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
    flex-wrap: wrap;
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
    align-items: center;
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
  .save-status-badge.saved { color: #10b981; background: rgba(16, 185, 129, 0.1); }
  .save-status-badge.saving { color: var(--accent-amber); background: var(--accent-amber-glow); }
  .save-status-badge.dirty { color: var(--accent-rust); background: var(--accent-rust-glow); }
  .save-status-badge.error { color: #ef4444; background: rgba(239, 68, 68, 0.1); }

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
  .auto-save-toggle input { width: 14px; height: 14px; cursor: pointer; margin: 0; }

  .modal-overlay {
    position: fixed;
    inset: 0;
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

  .modal-content h3 { color: white; font-size: 1.1rem; margin: 0; }
  .modal-content input { width: 100%; }
  .modal-actions { display: flex; justify-content: flex-end; gap: 10px; }

  .perm-modal { width: 480px !important; }
  .accent-name { color: var(--accent-amber); }

  .perm-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 16px;
    margin: 10px 0 20px;
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

  .perm-col input[type='checkbox'] { width: 14px; height: 14px; cursor: pointer; }

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
  }

  .recursive-label input { width: 14px; height: 14px; }

  .spin { animation: spin 1s linear infinite; }
  @keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
</style>
