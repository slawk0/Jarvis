<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { Save, X, FileCode, RefreshCw } from 'lucide-svelte';
  import SftpFileBrowser from './sftp/SftpFileBrowser.svelte';
  import SftpTransferPanel from './sftp/SftpTransferPanel.svelte';
  import type { FileInfo } from '$lib/sftp/types';
  import { getRemotePath } from '$lib/sftp/pathUtils';
  import SudoModal from './SudoModal.svelte';
  import { registerBackHandler } from '$lib/backNavigation.svelte';
  import { get } from 'svelte/store';
  import { LL } from '$lib/i18n/i18n-svelte';
  import { notifications } from '$lib/notifications.svelte';
  import {
    formatInvokeError,
    isSudoPasswordRequired,
  } from '$lib/i18n/backendErrors';
  import { validateContent } from '$lib/syntaxValidator';

  let { profileId = '', visible = true } = $props();
  let errorMsg = $state('');
  let browserPath = $state('/');
  let browserRef: SftpFileBrowser | undefined = $state();

  export function refresh() { browserRef?.refresh(); }

  $effect(() => {
    if (errorMsg) {
      notifications.error(errorMsg);
      errorMsg = '';
    }
  });

  // Edytor Monaco
  let editingFile = $state<string | null>(null);
  let editorElement: HTMLDivElement | null = $state(null);
  let editorInstance: any = null;
  let editorSaveStatus = $state<'saved' | 'saving' | 'dirty' | 'error'>('saved');
  let syntaxError = $state<string | null>(null);
  let autoSaveEnabled = $state(true);
  let saveTimeout: ReturnType<typeof setTimeout> | null = null;
  let isLoading = $state(false);

  // Modals
  let showNewFileModal = $state(false);
  let showNewDirModal = $state(false);
  let showRenameModal = $state(false);
  let newItemName = $state('');
  let selectedFile = $state<FileInfo | null>(null);

  // Permissions modal
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

  // Sudo modal
  let showSudoModal = $state(false);
  let sudoModalTitle = $state<string | undefined>(undefined);
  let sudoModalDesc = $state<string | undefined>(undefined);
  let pendingSudoAction = $state<(() => Promise<void>) | null>(null);
  let usedSudoForRead = $state(false);

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
      errorMsg = get(LL).files.fileTooLarge({ size: String(Math.round(file.size / 1024 / 1024)) });
      return;
    }

    isLoading = true;
    errorMsg = '';
    try {
      const content: string = await invoke('sftp_read', { path: filePath });
      usedSudoForRead = false;
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

            // Initial syntax check
            syntaxError = validateContent(monaco, editorInstance.getModel(), editingFile!);

            editorInstance.onDidChangeModelContent(() => {
              syntaxError = validateContent(monaco, editorInstance.getModel(), editingFile!);
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
                  if (usedSudoForRead) {
                    const escapedPath = "'" + editingFile.replace(/'/g, "'\\''") + "'";
                    const b64 = btoa(unescape(encodeURIComponent(currentVal)));
                    await invoke('exec_custom_command', { cmd: `echo ${b64} | base64 -d | sudo tee ${escapedPath} > /dev/null`, useSudo: true });
                  } else {
                    await invoke('sftp_write', { path: editingFile, content: currentVal });
                  }
                  editorSaveStatus = 'saved';
                } catch (err: unknown) {
                  editorSaveStatus = 'error';
                  errorMsg = get(LL).files.autoSaveError({ error: formatInvokeError(err) });
                }
              }, 1500);
            });
          });
        }
      }, 100);
    } catch (err: unknown) {
      if (String(err).toLowerCase().includes('permission denied')) {
        try {
          const hasSudo = await invoke<boolean>('has_sudo_password');
          if (hasSudo) {
            await openFileSudo(filePath, file);
            return;
          }
        } catch {}
        
        sudoModalTitle = get(LL).files.sudoRequired();
        sudoModalDesc = undefined;
        pendingSudoAction = async () => { await openFileSudo(filePath, file); };
        showSudoModal = true;
        return;
      }
      errorMsg = get(LL).files.readError({ error: formatInvokeError(err) });
    } finally {
      isLoading = false;
    }
  }

  async function openFileSudo(filePath: string, file: FileInfo) {
    isLoading = true;
    errorMsg = '';
    try {
      const escapedPath = "'" + filePath.replace(/'/g, "'\\''") + "'";
      const content: string = await invoke('exec_custom_command', { cmd: `sudo cat ${escapedPath}`, useSudo: true });
      usedSudoForRead = true;
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

            // Initial syntax check
            syntaxError = validateContent(monaco, editorInstance.getModel(), editingFile!);

            editorInstance.onDidChangeModelContent(() => {
              syntaxError = validateContent(monaco, editorInstance.getModel(), editingFile!);
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
                  const escapedPath = "'" + editingFile.replace(/'/g, "'\\''") + "'";
                  const b64 = btoa(unescape(encodeURIComponent(currentVal)));
                  await invoke('exec_custom_command', { cmd: `echo ${b64} | base64 -d | sudo tee ${escapedPath} > /dev/null`, useSudo: true });
                  editorSaveStatus = 'saved';
                } catch (err: unknown) {
                  editorSaveStatus = 'error';
                  errorMsg = get(LL).files.autoSaveError({ error: formatInvokeError(err) });
                }
              }, 1500);
            });
          });
        }
      }, 100);
    } catch (err: unknown) {
      errorMsg = get(LL).files.readError({ error: formatInvokeError(err) });
    } finally {
      isLoading = false;
    }
  }

  async function saveFile() {
    if (!editingFile || !editorInstance) return;
    isLoading = true;
    errorMsg = '';
    try {
      const content = editorInstance.getValue();
      if (usedSudoForRead) {
        const escapedPath = "'" + editingFile.replace(/'/g, "'\\''") + "'";
        const b64 = btoa(unescape(encodeURIComponent(content)));
        await invoke('exec_custom_command', { cmd: `echo ${b64} | base64 -d | sudo tee ${escapedPath} > /dev/null`, useSudo: true });
      } else {
        await invoke('sftp_write', { path: editingFile, content });
      }
      editorSaveStatus = 'saved';
    } catch (err: unknown) {
      errorMsg = get(LL).files.writeError({ error: formatInvokeError(err) });
      editorSaveStatus = 'error';
    } finally {
      isLoading = false;
    }
  }

  function closeEditor() {
    if (saveTimeout) clearTimeout(saveTimeout);
    editingFile = null;
    syntaxError = null;
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
      errorMsg = get(LL).files.createFileError({ error: formatInvokeError(err) });
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
      errorMsg = get(LL).files.createDirError({ error: formatInvokeError(err) });
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
      errorMsg = get(LL).files.renameError({ error: formatInvokeError(err) });
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
      if (isSudoPasswordRequired(err) || errStr.toLowerCase().includes('permission denied')) {
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
        sudoModalTitle = get(LL).files.sudoRequired();
        sudoModalDesc = undefined;
        pendingSudoAction = async () => {
          try {
            await invoke('exec_custom_command', { cmd, useSudo: true });
            showPermModal = false;
            await browserRef?.refresh();
          } catch (sudoErr: unknown) {
            errorMsg = get(LL).files.sudoError({ error: formatInvokeError(sudoErr) });
          } finally {
            isLoading = false;
          }
        };
        showSudoModal = true;
        return;
      } else {
        errorMsg = get(LL).files.chmodError({ error: formatInvokeError(err) });
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
        const ll = get(LL).backNav.fileManager;
        if (showPermModal) return ll.closePermModal();
        if (showRenameModal) return ll.cancelRename();
        if (showNewFileModal) return ll.cancelNewFile();
        if (showNewDirModal) return ll.cancelNewDir();
        if (editingFile) return ll.closeEditor();
        return ll.default();
      },
    });
  });

  onDestroy(() => {
    if (editorInstance) editorInstance.dispose();
    if (saveTimeout) clearTimeout(saveTimeout);
  });
</script>

<SudoModal
  bind:open={showSudoModal}
  title={sudoModalTitle}
  description={sudoModalDesc}
  onSuccess={() => {
    if (pendingSudoAction) pendingSudoAction();
  }}
/>

<div class="file-manager manager-shell fade-in">
  <div class="browser-layer" class:hidden={!!editingFile}>
    <SftpFileBrowser
      bind:this={browserRef}
      {profileId}
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
          <span>{$LL.files.editing()} <strong>{editingFile.split('/').pop()}</strong></span>
          <span class="path-badge mono-val">{editingFile}</span>
          {#if editorSaveStatus === 'saved'}
            <span class="save-status-badge saved">{$LL.files.savedBadge()}</span>
          {:else if editorSaveStatus === 'saving'}
            <span class="save-status-badge saving">{$LL.files.savingBadge()}</span>
          {:else if editorSaveStatus === 'dirty'}
            <span class="save-status-badge dirty">{$LL.files.dirtyBadge()}</span>
          {:else if editorSaveStatus === 'error'}
            <span class="save-status-badge error">{$LL.files.errorBadge()}</span>
          {/if}
          {#if syntaxError}
            <span class="save-status-badge error" title={syntaxError}>{$LL.files.syntaxErrorBadge()}</span>
          {/if}
        </div>
        <div class="editor-actions">
          <label class="auto-save-toggle">
            <input type="checkbox" bind:checked={autoSaveEnabled} />
            <span>{$LL.files.autoSave()}</span>
          </label>
          <button class="primary" onclick={saveFile} disabled={isLoading || editorSaveStatus === 'saving'}>
            <Save size={16} /> {$LL.files.save()}
          </button>
          <button class="secondary" onclick={closeEditor}>
            <X size={16} /> {$LL.files.close()}
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
      <h3>{$LL.files.permTitle()} <span class="mono-val accent-name">{permFile.name}</span></h3>
      <div class="perm-grid">
        <div class="perm-col">
          <h4>{$LL.common.owner()}</h4>
          <label><input type="checkbox" bind:checked={permOwnerRead} onchange={updateOctalFromCheckboxes} /> {$LL.common.read()}</label>
          <label><input type="checkbox" bind:checked={permOwnerWrite} onchange={updateOctalFromCheckboxes} /> {$LL.common.write()}</label>
          <label><input type="checkbox" bind:checked={permOwnerExec} onchange={updateOctalFromCheckboxes} /> {$LL.common.execute()}</label>
        </div>
        <div class="perm-col">
          <h4>{$LL.common.group()}</h4>
          <label><input type="checkbox" bind:checked={permGroupRead} onchange={updateOctalFromCheckboxes} /> {$LL.common.read()}</label>
          <label><input type="checkbox" bind:checked={permGroupWrite} onchange={updateOctalFromCheckboxes} /> {$LL.common.write()}</label>
          <label><input type="checkbox" bind:checked={permGroupExec} onchange={updateOctalFromCheckboxes} /> {$LL.common.execute()}</label>
        </div>
        <div class="perm-col">
          <h4>{$LL.common.others()}</h4>
          <label><input type="checkbox" bind:checked={permOthersRead} onchange={updateOctalFromCheckboxes} /> {$LL.common.read()}</label>
          <label><input type="checkbox" bind:checked={permOthersWrite} onchange={updateOctalFromCheckboxes} /> {$LL.common.write()}</label>
          <label><input type="checkbox" bind:checked={permOthersExec} onchange={updateOctalFromCheckboxes} /> {$LL.common.execute()}</label>
        </div>
      </div>
      <div class="form-group octal-group">
        <label for="octal-input">{$LL.files.octalValue()}</label>
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
          <span>{$LL.files.recursiveChmod()}</span>
        </label>
      {/if}
      <div class="modal-actions">
        <button class="primary" onclick={savePermissions} disabled={isLoading}>
          {#if isLoading}<RefreshCw size={14} class="spin" /> {$LL.files.savingPerm()}{:else}{$LL.common.save()}{/if}
        </button>
        <button class="secondary" onclick={() => (showPermModal = false)}>{$LL.common.cancel()}</button>
      </div>
    </div>
  </div>
{/if}

{#if showNewFileModal}
  <div class="modal-overlay">
    <div class="modal-content glass">
      <h3>{$LL.files.createFileTitle()}</h3>
      <input type="text" placeholder={$LL.files.createFilePlaceholder()} bind:value={newItemName} />
      <div class="modal-actions">
        <button class="primary" onclick={createFile}>{$LL.common.create()}</button>
        <button class="secondary" onclick={() => { showNewFileModal = false; newItemName = ''; }}>{$LL.common.cancel()}</button>
      </div>
    </div>
  </div>
{/if}

{#if showNewDirModal}
  <div class="modal-overlay">
    <div class="modal-content glass">
      <h3>{$LL.files.createDirTitle()}</h3>
      <input type="text" placeholder={$LL.files.createDirPlaceholder()} bind:value={newItemName} />
      <div class="modal-actions">
        <button class="primary" onclick={createDirectory}>{$LL.common.create()}</button>
        <button class="secondary" onclick={() => { showNewDirModal = false; newItemName = ''; }}>{$LL.common.cancel()}</button>
      </div>
    </div>
  </div>
{/if}

{#if showRenameModal}
  <div class="modal-overlay">
    <div class="modal-content glass">
      <h3>{$LL.files.renameTitle()}</h3>
      <input type="text" placeholder={$LL.files.renamePlaceholder()} bind:value={newItemName} />
      <div class="modal-actions">
        <button class="primary" onclick={renameItem}>{$LL.files.renameBtn()}</button>
        <button class="secondary" onclick={() => { showRenameModal = false; newItemName = ''; selectedFile = null; }}>{$LL.common.cancel()}</button>
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

</style>
