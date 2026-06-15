<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { Terminal as Xterm } from '@xterm/xterm';
  import { FitAddon } from '@xterm/addon-fit';
  import { Terminal as TerminalIcon, ExternalLink, RefreshCw } from 'lucide-svelte';
  import '@xterm/xterm/css/xterm.css';

  let { profileId } = $props();

  let terminalContainer = $state<HTMLDivElement | null>(null);
  let term: any = null;
  let fitAddon: any = null;
  let unsubscribeStdout: (() => void) | null = null;
  let isLoading = $state(false);
  let errorMsg = $state('');

  async function initTerminal() {
    isLoading = true;
    errorMsg = '';
    
    try {
      // 1. Uruchom sesję w Rust
      await invoke('start_terminal');
      
      // 2. Inicjalizacja xterm.js
      if (term) {
        term.dispose();
      }

      term = new Xterm({
        cursorBlink: true,
        fontFamily: 'Consolas, "Courier New", monospace',
        fontSize: 14,
        theme: {
          background: '#07090f',
          foreground: '#f3f4f6',
          cursor: '#00d2ff',
          black: '#000000',
          red: '#f43f5e',
          green: '#10b981',
          yellow: '#f59e0b',
          blue: '#3b82f6',
          magenta: '#a855f7',
          cyan: '#00d2ff',
          white: '#ffffff',
        }
      });

      fitAddon = new FitAddon();
      term.loadAddon(fitAddon);
      term.open(terminalContainer!);
      fitAddon.fit();

      // Przekazywanie wpisywanych znaków do Rusta
      term.onData((data: string) => {
        invoke('send_terminal_input', { input: data }).ok;
      });

      // 3. Słuchaj strumienia stdout z Rusta
      if (unsubscribeStdout) unsubscribeStdout();
      unsubscribeStdout = await listen<string>('terminal-stdout', (event) => {
        if (term) {
          term.write(event.payload);
        }
      });

      // Małe powitanie
      term.writeln('\x1b[1;36m[Jarvis SSH Terminal - Inicjalizacja...]\x1b[0m');

      // Obsługa zmiany rozmiaru okna
      window.addEventListener('resize', handleResize);

    } catch (err: any) {
      errorMsg = 'Nie udało się otworzyć terminala: ' + err.toString();
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
      await invoke('open_external_terminal', { profileId });
    } catch (err: any) {
      errorMsg = 'Nie udało się otworzyć zewnętrznego terminala: ' + err.toString();
    }
  }

  onMount(() => {
    initTerminal();
  });

  onDestroy(() => {
    if (unsubscribeStdout) {
      unsubscribeStdout();
    }
    if (term) {
      term.dispose();
    }
    window.removeEventListener('resize', handleResize);
  });
</script>

<div class="terminal-view fade-in">
  <header class="term-header">
    <div class="title-area">
      <h1>Konsola SSH</h1>
      <p class="subtitle">Wbudowany terminal do bezpośredniej pracy na serwerze</p>
    </div>
    {#if errorMsg}
      <div class="error-badge">{errorMsg}</div>
    {/if}
    <div class="actions">
      <button class="secondary" onclick={initTerminal} disabled={isLoading} title="Zrestartuj sesję">
        <RefreshCw size={16} class={isLoading ? 'spin' : ''} /> Zrestartuj
      </button>
      <button class="secondary" onclick={openExternal} title="Otwórz Windows Terminal">
        <ExternalLink size={16} /> Zewnętrzny terminal
      </button>
    </div>
  </header>

  <!-- Kontener xterm.js -->
  <div class="terminal-wrapper glass">
    <div bind:this={terminalContainer} class="terminal-element"></div>
  </div>
</div>

<style>
  .terminal-view {
    padding: 30px;
    display: flex;
    flex-direction: column;
    gap: 24px;
    height: 100%;
    overflow: hidden;
  }

  .term-header {
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

  .actions {
    display: flex;
    gap: 10px;
  }

  /* Wrapper terminala */
  .terminal-wrapper {
    flex: 1;
    border-radius: var(--radius-md);
    background: #07090f;
    padding: 16px;
    border: 1px solid var(--border-color);
    overflow: hidden;
    display: flex;
  }

  .terminal-element {
    width: 100%;
    height: 100%;
  }

  /* Dostosowanie xterm container */
  :global(.xterm) {
    padding: 4px;
    height: 100%;
  }

  :global(.xterm-viewport) {
    background-color: #07090f !important;
  }

  .spin {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
