<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { Play, Trash2, Radar, Activity, Route, Search as SearchIcon, Globe, Network } from 'lucide-svelte';
  import { LL } from '$lib/i18n/i18n-svelte';
  import { get } from 'svelte/store';
  import { stickToBottom } from '$lib/stickToBottom';
  import { shQuote } from '$lib/exec/target';
  import { formatInvokeError } from '$lib/i18n/backendErrors';
  import { notifications } from '$lib/notifications.svelte';

  type Tool = 'ping' | 'traceroute' | 'dns' | 'http' | 'mtr' | 'port';

  let tool = $state<Tool>('ping');
  let target = $state('');
  let recordType = $state('A');
  let port = $state('443');
  let output = $state('');
  let running = $state(false);

  const tools: { id: Tool; icon: any }[] = [
    { id: 'ping', icon: Activity },
    { id: 'traceroute', icon: Route },
    { id: 'dns', icon: SearchIcon },
    { id: 'http', icon: Globe },
    { id: 'mtr', icon: Radar },
    { id: 'port', icon: Network },
  ];

  function buildCmd(): string | null {
    const t = target.trim();
    if (!t) return null;
    const q = shQuote(t);
    switch (tool) {
      case 'ping':
        return `ping -c 4 -w 15 ${q}`;
      case 'traceroute':
        return `traceroute -w 2 -q 1 ${q} 2>/dev/null || tracepath ${q}`;
      case 'dns':
        return `dig ${shQuote(recordType)} +noall +answer +stats ${q} 2>/dev/null || nslookup -type=${shQuote(recordType)} ${q}`;
      case 'http':
        return `curl -sS -m 20 -i -L ${q}`;
      case 'mtr':
        return `mtr -r -c 5 ${q} 2>/dev/null || (echo "mtr not installed, falling back to traceroute" && traceroute -w 2 -q 1 ${q})`;
      case 'port':
        return `nc -zv -w 4 ${q} ${shQuote(port)} 2>&1 || (command -v nc >/dev/null || echo "netcat (nc) not installed")`;
    }
  }

  async function run() {
    const cmd = buildCmd();
    if (!cmd) {
      notifications.error(get(LL).netdiag.needTarget());
      return;
    }
    running = true;
    output = `$ ${cmd}\n`;
    const eventId = Math.random().toString(36).substring(7);
    const unStdout = await listen<string>(`exec-stdout-${eventId}`, (e) => (output += e.payload));
    const unStderr = await listen<string>(`exec-stderr-${eventId}`, (e) => (output += e.payload));
    try {
      await invoke('exec_custom_command_stream', { cmd, useSudo: false, eventId });
    } catch (err) {
      output += `\n${formatInvokeError(err)}`;
    } finally {
      unStdout();
      unStderr();
      running = false;
    }
  }

  function clear() {
    output = '';
  }
</script>

<div class="netdiag manager-shell fade-in">
  <header class="manager-header">
    <h1 class="page-title">{$LL.netdiag.title()}</h1>
  </header>

  <div class="tool-tabs">
    {#each tools as tt}
      <button class="tool-tab" class:active={tool === tt.id} onclick={() => (tool = tt.id)}>
        <tt.icon size={14} /> {$LL.netdiag.tools[tt.id]()}
      </button>
    {/each}
  </div>

  <div class="run-bar glass">
    <input class="target-input" type="text" placeholder={$LL.netdiag.targetPlaceholder()} bind:value={target} onkeydown={(e) => e.key === 'Enter' && !running && run()} />
    {#if tool === 'dns'}
      <select bind:value={recordType} class="extra-select">
        {#each ['A', 'AAAA', 'MX', 'TXT', 'NS', 'CNAME', 'SOA', 'PTR'] as rt}
          <option value={rt}>{rt}</option>
        {/each}
      </select>
    {/if}
    {#if tool === 'port'}
      <input class="port-input" type="text" placeholder={$LL.netdiag.portPlaceholder()} bind:value={port} />
    {/if}
    <button class="primary btn-compact" disabled={running} onclick={run}>
      <Play size={14} /> {running ? $LL.netdiag.running() : $LL.common.run()}
    </button>
    <button class="secondary btn-compact" disabled={!output} onclick={clear}>
      <Trash2 size={14} /> {$LL.common.clear()}
    </button>
  </div>

  <pre class="output glass" use:stickToBottom>{output || $LL.netdiag.emptyHint()}</pre>
</div>

<style>
  .tool-tabs { display: flex; gap: 6px; flex-wrap: wrap; flex-shrink: 0; }
  .tool-tab { display: flex; align-items: center; gap: 6px; background: transparent; border: 1px solid var(--border-color); color: var(--text-secondary); padding: 6px 12px; font-size: 0.78rem; border-radius: var(--radius-sm); cursor: pointer; }
  .tool-tab:hover { background: var(--bg-hover); color: var(--text-primary); }
  .tool-tab.active { background: var(--bg-active); color: var(--accent-amber); border-color: rgba(245, 158, 11, 0.25); }
  .run-bar { display: flex; gap: 8px; align-items: center; padding: 10px; border-radius: var(--radius-md); flex-shrink: 0; }
  .target-input { flex: 1; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--radius-sm); padding: 7px 10px; color: var(--text-primary); font-size: 0.85rem; font-family: var(--font-mono); }
  .extra-select, .port-input { background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--radius-sm); padding: 7px 10px; color: var(--text-primary); font-size: 0.82rem; }
  .port-input { width: 90px; font-family: var(--font-mono); }
  .output { flex: 1; overflow: auto; margin: 0; padding: 12px; border-radius: var(--radius-md); font-family: var(--font-mono); font-size: 0.78rem; color: var(--text-secondary); white-space: pre-wrap; word-break: break-word; line-height: 1.5; }
</style>
