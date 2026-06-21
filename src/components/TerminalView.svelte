<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { Terminal as Xterm } from '@xterm/xterm';
  import { FitAddon } from '@xterm/addon-fit';
  import {
    Terminal as TerminalIcon,
    ExternalLink,
    RefreshCw,
    Bookmark,
    Play,
    Plus,
    Trash2,
    Edit3,
    Clipboard,
    FileCode,
    Check,
    Save,
    X,
    Eye,
    Search
  } from 'lucide-svelte';
  import '@xterm/xterm/css/xterm.css';
  import { registerBackHandler } from '$lib/backNavigation.svelte';
  import { get } from 'svelte/store';
    import { notifications } from '$lib/notifications.svelte';
  import {
    formatInvokeError,
    isSudoPasswordRequired,
  } from '$lib/backendErrors';
  import PathAutocomplete from './ui/PathAutocomplete.svelte';
  import SudoModal from './SudoModal.svelte';
  import { validateContent } from '$lib/syntaxValidator';

  type ContainerSession = {
    containerId: string;
    containerName: string;
    useSudo: boolean;
    shell: string;
  };

  let {
    profileId,
    containerSession = null,
    onExitContainer = () => {},
    sessionId = crypto.randomUUID(),
    visible = true,
  }: {
    profileId: string;
    containerSession?: ContainerSession | null;
    onExitContainer?: () => void;
    sessionId?: string;
    visible?: boolean;
  } = $props();

  let terminalContainer = $state<HTMLDivElement | null>(null);
  let term: any = null;
  let fitAddon: any = null;
  let unsubscribeStdout: (() => void) | null = null;
  let isLoading = $state(false);
  let errorMsg = $state('');

  $effect(() => {
    if (errorMsg) {
      notifications.error(errorMsg);
      errorMsg = '';
    }
  });
  let activeContainer = $state<ContainerSession | null>(null);
  let resizeObserver: ResizeObserver | null = null;

  // Saved Commands State
  let showSavedCommands = $state(false);
  let savedCommands = $state<Array<{ id: string; label: string; command: string }>>([]);
  let searchQuery = $state('');
  let commandLabel = $state('');
  let commandText = $state('');
  let editingCommandId = $state<string | null>(null);
  let showAddCommandForm = $state(false);

  // Context Menu State
  let showContextMenu = $state(false);
  let contextMenuX = $state(0);
  let contextMenuY = $state(0);
  let selectedText = $state('');
  let isPathSelection = $state(false);

  // File Editor State
  let showFileModal = $state(false);
  let filePathToEdit = $state('');
  let useSudoForEdit = $state(false);
  let editingFile = $state<string | null>(null);
  let editorSaveStatus = $state<'saved' | 'saving' | 'dirty' | 'error'>('saved');
  let syntaxError = $state<string | null>(null);
  let editorElement = $state<HTMLDivElement | null>(null);
  let editorInstance = $state<any>(null);
  let usedSudoForRead = $state(false);
  let homeDir = $state('');

  let showSudoModal = $state(false);
  let pendingAction = $state<(() => Promise<void>) | null>(null);

  $effect(() => {
    activeContainer = containerSession ?? null;
  });

  // When this pane becomes visible again after being hidden (kept alive), the terminal
  // container was display:none with zero size — re-fit xterm and relayout the editor.
  $effect(() => {
    if (visible) {
      requestAnimationFrame(() => {
        try { fitAddon?.fit(); } catch (_) { /* ignore */ }
        try { editorInstance?.layout?.(); } catch (_) { /* ignore */ }
      });
    }
  });

  async function initTerminal() {
    isLoading = true;
    errorMsg = '';

    try {
      await invoke('stop_terminal', { sessionId }).catch(() => {});

      if (unsubscribeStdout) {
        unsubscribeStdout();
        unsubscribeStdout = null;
      }
      if (term) {
        term.dispose();
        term = null;
      }

      term = new Xterm({
        cursorBlink: true,
        fontFamily: '"JetBrains Mono", Consolas, monospace',
        fontSize: 14,
        theme: {
          background: '#07080a',
          foreground: '#e2e4e9',
          cursor: '#f59e0b',
          black: '#000000',
          red: '#ef4444',
          green: '#10b981',
          yellow: '#f59e0b',
          blue: '#d97706',
          magenta: '#c2410c',
          cyan: '#f59e0b',
          white: '#ffffff',
        }
      });

      fitAddon = new FitAddon();
      term.loadAddon(fitAddon);
      term.open(terminalContainer!);
      fitAddon.fit();

      term.attachCustomKeyEventHandler((e: KeyboardEvent) => {
        if (e.ctrlKey) {
          const key = e.key.toLowerCase();
          if (
            key === 'n' ||
            key === 'w' ||
            key === '1' ||
            key === '2' ||
            key === '3' ||
            key === '4' ||
            key === 'tab' ||
            (e.shiftKey && key === 't') ||
            (e.shiftKey && key === 'h') ||
            (e.altKey && key === 'b')
          ) {
            return false;
          }
        }
        return true;
      });

      term.onData((data: string) => {
        invoke('send_terminal_input', { sessionId, input: data });
      });

      term.onResize((size: { cols: number; rows: number }) => {
        invoke('resize_terminal', { sessionId, cols: size.cols, rows: size.rows }).catch(() => {});
      });

      const eventName = `terminal-stdout-${sessionId}`;
      unsubscribeStdout = await listen<string>(eventName, (event) => {
        if (term) {
          term.write(event.payload);
        }
      });

      await invoke('start_terminal', {
        sessionId,
        containerId: activeContainer?.containerId ?? null,
        useSudo: activeContainer?.useSudo ?? false,
        shell: activeContainer?.shell ?? null,
        cols: term.cols,
        rows: term.rows,
      });

      if (activeContainer) {
        term.writeln(`\x1b[1;33m[Jarvis Docker Terminal — Container: ${activeContainer.containerName}]\x1b[0m`);
      } else {
        term.writeln(`\x1b[1;33m$[Jarvis SSH Terminal — Initializing…]\x1b[0m`);
      }

      // Use ResizeObserver instead of window resize for split-panel support
      setupResizeObserver();
    } catch (err: unknown) {
      if (isSudoPasswordRequired(err)) {
        errorMsg = "Sudo password required — return to Docker and enter password, or enable passwordless access.";
      } else {
        errorMsg = `Failed to open terminal: ${formatInvokeError(err)}`;
      }
    } finally {
      isLoading = false;
    }
  }

  function setupResizeObserver() {
    if (resizeObserver) {
      resizeObserver.disconnect();
    }
    if (terminalContainer) {
      resizeObserver = new ResizeObserver(() => {
        if (fitAddon) {
          requestAnimationFrame(() => {
            try {
              fitAddon.fit();
            } catch (_) {
              // ignore fit errors during rapid resizing
            }
          });
        }
      });
      resizeObserver.observe(terminalContainer);
    }
  }

  async function openExternal() {
    try {
      await invoke('open_external_terminal', {
        profileId,
        containerId: activeContainer?.containerId ?? null,
        useSudo: activeContainer?.useSudo ?? false,
        shell: activeContainer?.shell ?? null,
      });
    } catch (err: unknown) {
      if (isSudoPasswordRequired(err)) {
        errorMsg = "Sudo password required — return to Docker and enter password, or enable passwordless access.";
      } else {
        errorMsg = `Failed to open external terminal: ${formatInvokeError(err)}`;
      }
    }
  }

  function switchToServerShell() {
    activeContainer = null;
    onExitContainer();
    initTerminal();
  }

  // Saved Commands Storage & Handlers
  function saveCommandsToStorage() {
    const storageKey = `jarvis_saved_commands_${profileId || 'global'}`;
    localStorage.setItem(storageKey, JSON.stringify(savedCommands));
  }

  function handleSaveCommand(e: Event) {
    e.preventDefault();
    if (!commandLabel.trim() || !commandText.trim()) return;

    if (editingCommandId) {
      savedCommands = savedCommands.map(cmd =>
        cmd.id === editingCommandId
          ? { ...cmd, label: commandLabel.trim(), command: commandText.trim() }
          : cmd
      );
      editingCommandId = null;
    } else {
      savedCommands = [
        ...savedCommands,
        {
          id: crypto.randomUUID(),
          label: commandLabel.trim(),
          command: commandText.trim()
        }
      ];
    }

    commandLabel = '';
    commandText = '';
    showAddCommandForm = false;
    saveCommandsToStorage();
  }

  function startEditCommand(cmd: { id: string; label: string; command: string }) {
    editingCommandId = cmd.id;
    commandLabel = cmd.label;
    commandText = cmd.command;
    showAddCommandForm = true;
  }

  function deleteCommand(id: string) {
    savedCommands = savedCommands.filter(cmd => cmd.id !== id);
    saveCommandsToStorage();
    if (editingCommandId === id) {
      editingCommandId = null;
      commandLabel = '';
      commandText = '';
      showAddCommandForm = false;
    }
  }

  function executeCommand(command: string) {
    if (!term) return;
    invoke('send_terminal_input', { sessionId, input: command + '\r' });
    term.focus();
  }

  // Context Menu Handlers
  function handleContextMenu(e: MouseEvent) {
    e.preventDefault();
    if (!term) return;

    selectedText = term.getSelection().trim();
    isPathSelection = selectedText.length > 2 &&
                      !/\s/.test(selectedText) &&
                      (selectedText.startsWith('/') || selectedText.includes('/') || selectedText.includes('.'));

    contextMenuX = e.clientX;
    contextMenuY = e.clientY;
    showContextMenu = true;
  }

  function closeContextMenu() {
    showContextMenu = false;
  }

  function handleCopy() {
    if (selectedText) {
      navigator.clipboard.writeText(selectedText);
    }
    closeContextMenu();
  }

  async function handlePaste() {
    try {
      const text = await navigator.clipboard.readText();
      if (text && term) {
        invoke('send_terminal_input', { sessionId, input: text });
        term.focus();
      }
    } catch (err) {
      console.error('Failed to read clipboard:', err);
    }
    closeContextMenu();
  }

  function handleSelectAll() {
    if (term) {
      term.selectAll();
    }
    closeContextMenu();
  }

  function handleClearTerminal() {
    if (term) {
      term.clear();
      term.focus();
    }
    closeContextMenu();
  }

  function handleEditSelectedPath() {
    if (selectedText) {
      openFileForEditing(selectedText);
    }
    closeContextMenu();
  }

  function cleanPath(path: string): string {
    let cleaned = path.trim();
    if (cleaned === '~') {
      return homeDir || '~';
    }
    if (cleaned.startsWith('~/')) {
      const baseHome = homeDir || '~';
      return baseHome.endsWith('/') 
        ? baseHome + cleaned.slice(2) 
        : baseHome + '/' + cleaned.slice(2);
    }
    return cleaned;
  }

  function getCurrentTerminalPath(): string {
    if (!term) return '';
    const buffer = term.buffer.active;
    const activeLineIndex = buffer.cursorY + buffer.baseY;

    // Scan up to 15 lines upwards from the active cursor line
    const maxScanLines = Math.min(15, activeLineIndex + 1);

    for (let i = 0; i < maxScanLines; i++) {
      const lineIndex = activeLineIndex - i;
      const line = buffer.getLine(lineIndex);
      if (!line) continue;
      const text = line.translateToString(true).trim();
      if (!text) continue;

      // Regex 1: user@host:path$ or root@host:path# or user@host:path%
      const sshPromptRegex = /(?:^|[\s])([a-zA-Z0-9_.-]+@[a-zA-Z0-9_.-]+):([^$#%>\s]+)\s*[$#%>]/;
      const match1 = text.match(sshPromptRegex);
      if (match1) {
        return cleanPath(match1[2]);
      }

      // Regex 2: [user@host path]$ or [user@host path]#
      const bracketPromptRegex = /\[[a-zA-Z0-9_.-]+@[a-zA-Z0-9_.-]+\s+([^\]]+)\]\s*[$#%>]/;
      const match2 = text.match(bracketPromptRegex);
      if (match2) {
        return cleanPath(match2[1]);
      }

      // Regex 3: [ path ]$ or [ path ]#
      const bracketSpacePromptRegex = /\[\s*([^\s\]]+)\s*\]\s*[$#%>]/;
      const match3 = text.match(bracketSpacePromptRegex);
      if (match3) {
        return cleanPath(match3[1]);
      }

      // Regex 4: /etc/nginx # or / # or /app $
      const simplePromptRegex = /(?:^|[\s])(\/(?:[a-zA-Z0-9_.-]+\/?)*|~)\s*[$#%>]/;
      const match4 = text.match(simplePromptRegex);
      if (match4) {
        return cleanPath(match4[1]);
      }

      // Regex 5: Windows style C:\Users\username>
      const winPromptRegex = /(?:^|[\s])([a-zA-Z]:\\[^>]*)\s*>/;
      const match5 = text.match(winPromptRegex);
      if (match5) {
        return match5[1];
      }
    }

    return '';
  }

  function openEditFileModal() {
    const currentPath = getCurrentTerminalPath();
    if (currentPath) {
      filePathToEdit = currentPath.endsWith('/') ? currentPath : currentPath + '/';
    } else {
      filePathToEdit = '';
    }
    showFileModal = true;
  }

  // File Editor Functions
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

  async function openFileForEditing(filePath: string) {
    if (!filePath) return;
    isLoading = true;
    errorMsg = '';
    showFileModal = false;

    const run = async () => {
      try {
        let content = '';
        usedSudoForRead = useSudoForEdit;

        if (usedSudoForRead) {
          const escapedPath = "'" + filePath.replace(/'/g, "'\\''") + "'";
          const b64: string = await invoke('exec_custom_command', {
            cmd: `sudo cat ${escapedPath} | base64`,
            useSudo: true
          });
          const cleanB64 = b64.replace(/\s/g, '');
          content = decodeURIComponent(escape(atob(cleanB64)));
        } else {
          content = await invoke('sftp_read', { path: filePath });
        }

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
                language: detectLanguage(filePath.split('/').pop() || ''),
                theme: 'vs-dark',
                automaticLayout: true,
                fontSize: 14,
                fontFamily: '"JetBrains Mono", Consolas, monospace',
                minimap: { enabled: false },
              });

              // Initial syntax check
              syntaxError = validateContent(monaco, editorInstance.getModel(), filePath);

              editorInstance.onDidChangeModelContent(() => {
                syntaxError = validateContent(monaco, editorInstance.getModel(), filePath);
                editorSaveStatus = 'dirty';
              });

              editorInstance.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => {
                saveFile();
              });
            });
          }
        }, 100);
      } catch (err: unknown) {
        if (isSudoPasswordRequired(err)) {
          pendingAction = run;
          showSudoModal = true;
        } else {
          errorMsg = `Failed to read file: ${formatInvokeError(err)}`;
        }
      } finally {
        isLoading = false;
      }
    };

    await run();
  }

  async function saveFile() {
    if (!editingFile || !editorInstance) return;
    const file = editingFile;
    isLoading = true;
    errorMsg = '';
    editorSaveStatus = 'saving';
    
    const run = async () => {
      try {
        const content = editorInstance.getValue();
        if (usedSudoForRead) {
          const escapedPath = "'" + file.replace(/'/g, "'\\''") + "'";
          const b64 = btoa(unescape(encodeURIComponent(content)));
          await invoke('exec_custom_command', {
            cmd: `echo ${b64} | base64 -d | sudo tee ${escapedPath} > /dev/null`,
            useSudo: true
          });
        } else {
          await invoke('sftp_write', { path: file, content });
        }
        editorSaveStatus = 'saved';
      } catch (err: unknown) {
        if (isSudoPasswordRequired(err)) {
          pendingAction = run;
          showSudoModal = true;
        } else {
          errorMsg = `Failed to write file: ${formatInvokeError(err)}`;
          editorSaveStatus = 'error';
        }
      } finally {
        isLoading = false;
      }
    };

    await run();
  }

  function closeEditor() {
    editingFile = null;
    syntaxError = null;
    if (editorInstance) {
      editorInstance.dispose();
      editorInstance = null;
    }
    if (term) {
      setTimeout(() => {
        term.focus();
        if (fitAddon) {
          fitAddon.fit();
        }
      }, 100);
    }
  }

  onMount(() => {
    activeContainer = containerSession ?? null;
    initTerminal();

    invoke<string>('sftp_get_home_dir')
      .then((path) => {
        homeDir = path;
      })
      .catch((err) => {
        console.warn('Failed to get home dir:', err);
      });

    // Load saved commands
    const storageKey = `jarvis_saved_commands_${profileId || 'global'}`;
    const stored = localStorage.getItem(storageKey);
    if (stored) {
      savedCommands = JSON.parse(stored);
    } else {
      savedCommands = [
        { id: '1', label: 'Disk Usage', command: 'df -h' },
        { id: '2', label: 'Memory Usage', command: 'free -m' },
        { id: '3', label: 'Active Containers', command: 'docker ps' },
        { id: '4', label: 'System Info', command: 'uname -a' },
        { id: '5', label: 'List Files (Detailed)', command: 'ls -la' }
      ];
      localStorage.setItem(storageKey, JSON.stringify(savedCommands));
    }

    return registerBackHandler({
      id: `terminal-container-${sessionId}`,
      priority: 75,
      canGoBack: () => !!activeContainer || !!editingFile,
      goBack: () => {
        if (editingFile) {
          closeEditor();
        } else if (activeContainer) {
          switchToServerShell();
        }
      },
      label: "Back to server shell",
    });
  });

  onDestroy(() => {
    if (unsubscribeStdout) {
      unsubscribeStdout();
    }
    if (resizeObserver) {
      resizeObserver.disconnect();
    }
    if (term) {
      term.dispose();
    }
    if (editorInstance) {
      editorInstance.dispose();
    }
    invoke('stop_terminal', { sessionId }).catch(() => {});
  });
</script>

<svelte:window 
  onclick={closeContextMenu} 
  onkeydown={(e) => {
    if (e.key === 'Escape') {
      closeContextMenu();
      showFileModal = false;
    }
  }}
/>

<div class="terminal-view manager-shell fade-in">
  {#if editingFile}
    <div class="editor-view glass">
      <header class="editor-header">
        <div class="editor-title">
          <FileCode size={18} class="accent-blue-text" />
          <span>{`Editing ${editingFile.split('/').pop()}`}</span>
          <span class="path-badge mono-val">{editingFile}</span>
          {#if editorSaveStatus === 'saved'}
            <span class="save-status-badge saved">Saved</span>
          {:else if editorSaveStatus === 'saving'}
            <span class="save-status-badge saving">Saving...</span>
          {:else if editorSaveStatus === 'dirty'}
            <span class="save-status-badge dirty">Unsaved changes</span>
          {:else if editorSaveStatus === 'error'}
            <span class="save-status-badge error">Error saving</span>
          {/if}
          {#if syntaxError}
            <span class="save-status-badge error" title={syntaxError}>● Syntax error</span>
          {/if}
        </div>
        <div class="editor-actions">
          <button class="primary" onclick={saveFile} disabled={isLoading || editorSaveStatus === 'saving'}>
            <Save size={16} /> Save
          </button>
          <button class="secondary" onclick={closeEditor}>
            <X size={16} /> Close
          </button>
        </div>
      </header>
      <div bind:this={editorElement} class="editor-container"></div>
    </div>
  {/if}

  <div class="terminal-main-layout" class:hidden={!!editingFile}>
    <header class="manager-header term-header">
      <div class="title-area">
        {#if activeContainer}
          <h1 class="page-title">{`Shell: ${activeContainer.containerName}`}</h1>
        {:else}
          <h1 class="page-title">SSH console</h1>
        {/if}
      </div>
      <div class="actions">
        {#if activeContainer}
          <button class="secondary" onclick={switchToServerShell} disabled={isLoading} title="Back to server shell">
            <TerminalIcon size={16} /> Server shell
          </button>
        {/if}
        <button class="secondary" onclick={initTerminal} disabled={isLoading} title="Restart session">
          <RefreshCw size={16} class={isLoading ? 'spin' : ''} /> Restart
        </button>
        <button class="secondary" onclick={openEditFileModal} title="Edit File">
          <FileCode size={16} /> Edit File
        </button>
        <button class="secondary" class:active={showSavedCommands} onclick={() => (showSavedCommands = !showSavedCommands)} title="Saved Commands">
          <Bookmark size={16} /> Saved Commands
        </button>
        <button class="secondary" onclick={openExternal} title={activeContainer ? "Open container shell in Windows Terminal" : "Open Windows Terminal"}>
          <ExternalLink size={16} /> External terminal
        </button>
      </div>
    </header>

    <div class="terminal-workspace">
      <div class="terminal-wrapper glass" oncontextmenu={handleContextMenu}>
        <div bind:this={terminalContainer} class="terminal-element"></div>
      </div>

      {#if showSavedCommands}
        <div class="saved-commands-panel glass">
          <div class="panel-header">
            <h3>Saved Commands</h3>
            <button class="icon-btn" onclick={() => { showAddCommandForm = !showAddCommandForm; editingCommandId = null; commandLabel = ''; commandText = ''; }} title="Add Command">
              {#if showAddCommandForm}
                <X size={16} />
              {:else}
                <Plus size={16} />
              {/if}
            </button>
          </div>

          {#if showAddCommandForm}
            <form class="command-form" onsubmit={handleSaveCommand}>
              <h4>{editingCommandId ? "Edit Command" : "Add Command"}</h4>
              <div class="form-group">
                <label for="cmd-label">Name/Label</label>
                <input id="cmd-label" bind:value={commandLabel} placeholder="e.g. List containers" required />
              </div>
              <div class="form-group">
                <label for="cmd-text">Command</label>
                <textarea id="cmd-text" bind:value={commandText} placeholder="e.g. docker ps" rows="3" required></textarea>
              </div>
              <div class="form-actions">
                <button type="submit" class="primary">
                  <Check size={14} /> Save
                </button>
                <button type="button" class="secondary" onclick={() => { showAddCommandForm = false; editingCommandId = null; }}>
                  Close
                </button>
              </div>
            </form>
          {/if}

          <div class="panel-search">
            <span class="search-icon-wrapper"><Search size={16} /></span>
            <input type="text" bind:value={searchQuery} placeholder="Filter results…" />
          </div>

          <div class="commands-list">
            {#each savedCommands.filter(c => c.label.toLowerCase().includes(searchQuery.toLowerCase()) || c.command.toLowerCase().includes(searchQuery.toLowerCase())) as cmd (cmd.id)}
              <div class="command-card">
                <div class="command-info" onclick={() => executeCommand(cmd.command)} role="button" tabindex="0" onkeydown={(e) => e.key === 'Enter' && executeCommand(cmd.command)}>
                  <div class="command-card-title">{cmd.label}</div>
                  <code class="command-card-text">{cmd.command}</code>
                </div>
                <div class="command-actions">
                  <button class="icon-btn play-btn" onclick={() => executeCommand(cmd.command)} title="Run">
                    <Play size={14} />
                  </button>
                  <button class="icon-btn edit-btn" onclick={() => startEditCommand(cmd)} title="Edit">
                    <Edit3 size={14} />
                  </button>
                  <button class="icon-btn delete-btn" onclick={() => deleteCommand(cmd.id)} title="Delete">
                    <Trash2 size={14} />
                  </button>
                </div>
              </div>
            {:else}
              <div class="no-commands">
                No saved commands yet. Click Add to create one!
              </div>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  </div>
</div>

{#if showContextMenu}
  <div 
    class="context-menu glass" 
    style="position: fixed; left: {contextMenuX}px; top: {contextMenuY}px; z-index: 1000;"
  >
    {#if selectedText}
      <button class="menu-item" onclick={handleCopy}>
        <Clipboard size={14} /> Copy Selection
      </button>
      {#if isPathSelection}
        <button class="menu-item highlight" onclick={handleEditSelectedPath}>
          <FileCode size={14} /> Open Selected Path in Editor
        </button>
      {/if}
      <hr class="menu-separator" />
    {/if}
    <button class="menu-item" onclick={handlePaste}>
      <Clipboard size={14} /> Paste
    </button>
    <button class="menu-item" onclick={handleSelectAll}>
      <Eye size={14} /> Select All
    </button>
    <hr class="menu-separator" />
    <button class="menu-item danger" onclick={handleClearTerminal}>
      <Trash2 size={14} /> Clear Terminal
    </button>
  </div>
{/if}

{#if showFileModal}
  <div class="modal-overlay">
    <div class="modal-content glass file-modal">
      <h3>Open File for Editing</h3>
      
      <div class="form-group">
        <label for="file-path">File Path</label>
        <PathAutocomplete 
          bind:value={filePathToEdit} 
          placeholder="/etc/nginx/nginx.conf" 
          onlyDirs={false} 
        />
      </div>

      <div class="form-group checkbox-group">
        <label>
          <input type="checkbox" bind:checked={useSudoForEdit} />
          <span>Use Sudo</span>
        </label>
      </div>

      <div class="modal-actions">
        <button class="primary" onclick={() => openFileForEditing(filePathToEdit)} disabled={!filePathToEdit || isLoading}>
          Open Editor
        </button>
        <button class="secondary" onclick={() => { showFileModal = false; errorMsg = ''; }}>
          Close
        </button>
      </div>
    </div>
  </div>
{/if}

<SudoModal
  bind:open={showSudoModal}
  onSuccess={() => {
    if (pendingAction) {
      const action = pendingAction;
      pendingAction = null;
      action();
    }
  }}
/>


<style>
  .terminal-view {
    position: relative;
    /* uses .manager-shell */
  }

  .term-header {
    flex-wrap: wrap;
  }

  .term-header .actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .actions {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }

  button.secondary.active {
    background: var(--accent-muted) !important;
    border-color: var(--accent-primary) !important;
  }

  .terminal-main-layout {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    gap: 8px;
    width: 100%;
    height: 100%;
  }

  .terminal-main-layout.hidden {
    position: absolute !important;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    padding: inherit;
    box-sizing: border-box;
    visibility: hidden;
    pointer-events: none;
  }

  .terminal-workspace {
    display: flex;
    gap: 16px;
    flex: 1;
    min-height: 0;
    width: 100%;
    position: relative;
  }

  .terminal-wrapper {
    flex: 1;
    border-radius: var(--radius-sm);
    background: #07080a;
    padding: 12px;
    border: 1px solid var(--border-color);
    overflow: hidden;
    display: flex;
  }

  .terminal-element {
    width: 100%;
    height: 100%;
  }

  :global(.xterm) {
    padding: 4px;
    height: 100%;
  }

  :global(.xterm-viewport) {
    background-color: #07080a !important;
  }

  .saved-commands-panel {
    width: 320px;
    display: flex;
    flex-direction: column;
    background: var(--bg-surface);
    border: 1px solid var(--border-white) !important;
    border-radius: var(--radius-sm);
    padding: 16px;
    gap: 12px;
    height: 100%;
    min-height: 0;
    box-shadow: none;
    animation: slideIn 0.2s cubic-bezier(0.16, 1, 0.3, 1);
  }

  @keyframes slideIn {
    from {
      opacity: 0;
      transform: translateX(20px);
    }
    to {
      opacity: 1;
      transform: translateX(0);
    }
  }

  .panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .panel-header h3 {
    font-size: 1rem;
    margin: 0;
    color: var(--text-primary);
  }

  .panel-search {
    position: relative;
    display: flex;
    align-items: center;
  }

  .panel-search .search-icon-wrapper {
    position: absolute;
    left: 10px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--text-muted);
    pointer-events: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .panel-search input {
    width: 100%;
    padding: 6px 12px 6px 36px !important;
    font-size: 0.85rem;
    background: var(--bg-element);
    border: 1px solid var(--border-white);
  }

  .commands-list {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-right: 2px;
  }

  .command-card {
    display: flex;
    flex-direction: column;
    background: var(--bg-element);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-sm);
    padding: 10px;
    gap: 8px;
    transition: border-color var(--transition-fast), background-color var(--transition-fast);
  }

  .command-card:hover {
    border-color: var(--accent-primary);
    background: rgba(29, 78, 216, 0.05);
  }

  .command-info {
    cursor: pointer;
    display: flex;
    flex-direction: column;
    gap: 4px;
    outline: none;
  }

  .command-card-title {
    font-weight: 600;
    font-size: 0.85rem;
    color: var(--text-primary);
  }

  .command-card-text {
    font-family: var(--font-mono);
    font-size: 0.75rem;
    color: var(--text-muted);
    background: rgba(0, 0, 0, 0.2);
    padding: 4px 6px;
    border-radius: 2px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    border: 1px solid rgba(255, 255, 255, 0.05);
  }

  .command-card:hover .command-card-text {
    color: var(--text-primary);
  }

  .command-actions {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    padding-top: 6px;
  }

  .icon-btn {
    background: transparent !important;
    border: 1px solid transparent !important;
    border-radius: var(--radius-sm);
    color: var(--text-muted);
    padding: 4px;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    transition: var(--transition-fast);
  }

  .icon-btn:hover {
    color: var(--text-primary);
    background: var(--bg-hover) !important;
    border-color: var(--border-white) !important;
  }

  .play-btn:hover {
    color: var(--accent-green);
    background: var(--accent-green-glow) !important;
    border-color: rgba(16, 185, 129, 0.3) !important;
  }

  .delete-btn:hover {
    color: var(--accent-red);
    background: var(--accent-red-glow) !important;
    border-color: rgba(239, 68, 68, 0.3) !important;
  }

  .command-form {
    background: var(--bg-element);
    border: 1px solid var(--border-white);
    border-radius: var(--radius-sm);
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    animation: fadeIn var(--transition-fast);
  }

  .command-form h4 {
    font-size: 0.85rem;
    margin: 0 0 4px 0;
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .form-group label {
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  .form-group input, .form-group textarea {
    padding: 6px 10px !important;
    font-size: 0.8rem !important;
  }

  .form-actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
    margin-top: 4px;
  }

  .form-actions button {
    padding: 6px 12px !important;
    font-size: 0.75rem !important;
  }

  .no-commands {
    font-size: 0.8rem;
    color: var(--text-muted);
    text-align: center;
    padding: 24px 8px;
    border: 1px dashed var(--border-subtle);
    border-radius: var(--radius-sm);
  }

  /* Context Menu */
  .context-menu {
    background: var(--bg-surface) !important;
    border: 1px solid var(--border-white) !important;
    border-radius: var(--radius-sm);
    padding: 4px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 180px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5) !important;
    animation: menuFade 0.15s cubic-bezier(0.16, 1, 0.3, 1);
  }

  @keyframes menuFade {
    from {
      opacity: 0;
      transform: scale(0.95);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }

  .menu-item {
    background: transparent !important;
    border: 1px solid transparent !important;
    color: var(--text-primary) !important;
    padding: 6px 12px !important;
    font-size: 0.8rem !important;
    font-weight: 500 !important;
    border-radius: 2px !important;
    width: 100%;
    justify-content: flex-start !important;
    display: flex !important;
    align-items: center !important;
    gap: 8px !important;
    cursor: pointer !important;
    transition: var(--transition-fast) !important;
  }

  .menu-item:hover {
    background: var(--bg-hover) !important;
    border-color: var(--border-white) !important;
  }

  .menu-item.highlight {
    color: var(--accent-primary) !important;
    background: var(--bg-hover) !important;
  }

  .menu-item.highlight:hover {
    color: var(--text-primary) !important;
    background: var(--accent-primary) !important;
  }

  .menu-item.danger:hover {
    background: var(--accent-red) !important;
    color: #fff !important;
    border-color: rgba(239, 68, 68, 0.5) !important;
  }

  .menu-separator {
    border: none;
    border-top: 1px solid var(--border-subtle);
    margin: 4px 0;
  }

  /* Editor View */
  .editor-view {
    flex: 1;
    display: flex;
    flex-direction: column;
    border-radius: var(--radius-sm);
    background: #07080a;
    border: 1px solid var(--border-white);
    overflow: hidden;
    height: 100%;
  }

  .editor-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 16px;
    background: var(--bg-surface);
    border-bottom: 1px solid var(--border-subtle);
  }

  .editor-title {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 0.85rem;
    font-weight: 600;
  }

  .path-badge {
    background: rgba(0, 0, 0, 0.3);
    padding: 2px 8px;
    border-radius: var(--radius-sm);
    font-size: 0.75rem;
    color: var(--text-muted);
    border: 1px solid rgba(255, 255, 255, 0.05);
  }

  .save-status-badge {
    font-size: 0.7rem;
    padding: 1px 6px;
    border-radius: 2px;
    font-weight: 600;
    text-transform: uppercase;
  }

  .save-status-badge.saved {
    background: var(--accent-green-glow);
    color: var(--accent-green);
    border: 1px solid rgba(16, 185, 129, 0.3);
  }

  .save-status-badge.saving {
    background: var(--accent-amber-glow);
    color: #f59e0b;
    border: 1px solid rgba(245, 158, 11, 0.3);
  }

  .save-status-badge.dirty {
    background: rgba(255, 255, 255, 0.1);
    color: var(--text-primary);
    border: 1px solid rgba(255, 255, 255, 0.2);
  }

  .save-status-badge.error {
    background: var(--accent-red-glow);
    color: var(--accent-red);
    border: 1px solid rgba(239, 68, 68, 0.3);
  }

  .editor-actions {
    display: flex;
    gap: 8px;
  }

  .editor-actions button {
    padding: 6px 12px !important;
    font-size: 0.75rem !important;
  }

  .editor-container {
    flex: 1;
    min-height: 0;
    width: 100%;
    height: 100%;
  }

  /* Modals */
  .modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1100;
    backdrop-filter: blur(4px);
  }

  .modal-content {
    width: 450px;
    max-width: 90%;
    background: var(--bg-surface) !important;
    border: 1px solid var(--border-white) !important;
    border-radius: var(--radius-sm);
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.6) !important;
  }

  .modal-content h3 {
    font-size: 1.1rem;
    margin: 0;
    font-family: var(--font-display);
  }

  .checkbox-group {
    flex-direction: row;
    align-items: center;
    gap: 8px;
    cursor: pointer;
  }

  .checkbox-group label {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    font-size: 0.85rem;
  }

  .checkbox-group input {
    margin: 0;
    cursor: pointer;
  }

  .modal-error {
    background: var(--accent-red-glow);
    border: 1px solid rgba(239, 68, 68, 0.3);
    padding: 8px 12px;
    border-radius: var(--radius-sm);
    color: #ff8595;
    font-size: 0.8rem;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 8px;
  }
</style>
