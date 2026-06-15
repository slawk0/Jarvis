<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { Terminal as Xterm } from '@xterm/xterm';
  import { FitAddon } from '@xterm/addon-fit';
  import { Terminal as TerminalIcon, ExternalLink, RefreshCw } from 'lucide-svelte';
  import '@xterm/xterm/css/xterm.css';
  import { registerBackHandler } from '$lib/backNavigation.svelte';

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
  }: {
    profileId: string;
    containerSession?: ContainerSession | null;
    onExitContainer?: () => void;
  } = $props();

  let terminalContainer = $state<HTMLDivElement | null>(null);
  let term: any = null;
  let fitAddon: any = null;
  let unsubscribeStdout: (() => void) | null = null;
  let isLoading = $state(false);
  let errorMsg = $state('');
  let activeContainer = $state<ContainerSession | null>(null);

  $effect(() => {
    activeContainer = containerSession ?? null;
  });

  async function initTerminal() {
    isLoading = true;
    errorMsg = '';

    try {
      await invoke('stop_terminal').catch(() => {});

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

      term.onData((data: string) => {
        invoke('send_terminal_input', { input: data });
      });

      unsubscribeStdout = await listen<string>('terminal-stdout', (event) => {
        if (term) {
          term.write(event.payload);
        }
      });

      await invoke('start_terminal', {
        containerId: activeContainer?.containerId ?? null,
        useSudo: activeContainer?.useSudo ?? false,
        shell: activeContainer?.shell ?? null,
      });

      if (activeContainer) {
        term.writeln(`\x1b[1;33m[Jarvis — shell kontenera: ${activeContainer.containerName}]\x1b[0m`);
      } else {
        term.writeln('\x1b[1;33m[Jarvis SSH Terminal — Inicjalizacja...]\x1b[0m');
      }

      window.addEventListener('resize', handleResize);
    } catch (err: any) {
      const errText = err.toString();
      if (errText.includes('SUDO_PASSWORD_REQUIRED')) {
        errorMsg = 'Wymagane hasło sudo — wróć do Dockera i podaj hasło, albo włącz dostęp bez sudo.';
      } else {
        errorMsg = 'Nie udało się otworzyć terminala: ' + errText;
      }
    } finally {
      isLoading = false;
    }
  }

  function handleResize() {
    if (fitAddon) {
      fitAddon.fit();
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
    } catch (err: any) {
      const errText = err.toString();
      if (errText.includes('SUDO_PASSWORD_REQUIRED')) {
        errorMsg = 'Wymagane hasło sudo — wróć do Dockera i podaj hasło, albo włącz dostęp bez sudo.';
      } else {
        errorMsg = 'Nie udało się otworzyć zewnętrznego terminala: ' + errText;
      }
    }
  }

  function switchToServerShell() {
    activeContainer = null;
    onExitContainer();
    initTerminal();
  }

  onMount(() => {
    activeContainer = containerSession ?? null;
    initTerminal();
    return registerBackHandler({
      id: 'terminal-container',
      priority: 75,
      canGoBack: () => !!activeContainer,
      goBack: () => {
        if (activeContainer) switchToServerShell();
      },
      label: 'Wróć do shella serwera',
    });
  });

  onDestroy(() => {
    if (unsubscribeStdout) {
      unsubscribeStdout();
    }
    if (term) {
      term.dispose();
    }
    window.removeEventListener('resize', handleResize);
    invoke('stop_terminal').catch(() => {});
  });
</script>

<div class="terminal-view manager-shell fade-in">
  <header class="manager-header term-header">
    <div class="title-area">
      {#if activeContainer}
        <h1 class="page-title">Shell: {activeContainer.containerName}</h1>
      {:else}
        <h1 class="page-title">Konsola SSH</h1>
      {/if}
    </div>
    {#if errorMsg}
      <div class="error-badge">{errorMsg}</div>
    {/if}
    <div class="actions">
      {#if activeContainer}
        <button class="secondary" onclick={switchToServerShell} disabled={isLoading} title="Wróć do shella serwera">
          <TerminalIcon size={16} /> Shell serwera
        </button>
      {/if}
      <button class="secondary" onclick={initTerminal} disabled={isLoading} title="Zrestartuj sesję">
        <RefreshCw size={16} class={isLoading ? 'spin' : ''} /> Zrestartuj
      </button>
      <button class="secondary" onclick={openExternal} title={activeContainer ? 'Otwórz shell kontenera w Windows Terminal' : 'Otwórz Windows Terminal'}>
        <ExternalLink size={16} /> Zewnętrzny terminal
      </button>
    </div>
  </header>

  <div class="terminal-wrapper glass">
    <div bind:this={terminalContainer} class="terminal-element"></div>
  </div>
</div>

<style>
  .terminal-view {
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

  .error-badge {
    background: var(--accent-red-glow);
    border: 1px solid rgba(244, 63, 94, 0.3);
    padding: 8px 16px;
    border-radius: var(--radius-sm);
    color: #ff8595;
    font-size: 0.85rem;
  }

  .actions {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
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

  .spin {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
