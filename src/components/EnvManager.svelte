<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { KeyRound, Box, Eye, EyeOff, Plus, Trash2, RefreshCw, Copy, Check, X } from 'lucide-svelte';
    import { get } from 'svelte/store';
  import { listContainers, shQuote } from '$lib/exec/target';
  import { notifications } from '$lib/notifications.svelte';
  import { formatInvokeError } from '$lib/backendErrors';

  let { profileId = '', visible = true } = $props();

  export function refresh() { loadContainers(); }

  type Tab = 'local' | 'docker';
  let tab = $state<Tab>('local');
  let errorMsg = $state('');

  // ── local mode ──
  type EnvVar = { key: string; value: string; revealed: boolean };
  let localVars = $state<EnvVar[]>([]);
  let localLoading = $state(false);
  let localLoaded = $state(false);

  // add-var inline form
  let showAddForm = $state(false);
  let newKey = $state('');
  let newValue = $state('');
  let persistTarget = $state('~/.bashrc');
  let adding = $state(false);
  const PERSIST_TARGETS = ['~/.bashrc', '~/.profile', '~/.bash_profile', '/etc/environment'];

  // ── docker mode ──
  let containers = $state<string[]>([]);
  let selectedContainer = $state('');
  let dockerVars = $state<EnvVar[]>([]);
  let dockerLoading = $state(false);

  $effect(() => {
    if (errorMsg) {
      notifications.error(errorMsg);
      errorMsg = '';
    }
  });

  function isSecret(key: string): boolean {
    return /pass|secret|token|key|pwd|credential|api/i.test(key);
  }

  function parseEnvLines(out: string): EnvVar[] {
    return out
      .split('\n')
      .map((l) => l.trim())
      .filter(Boolean)
      .map((l) => {
        const i = l.indexOf('=');
        return { key: i === -1 ? l : l.slice(0, i), value: i === -1 ? '' : l.slice(i + 1), revealed: false };
      });
  }

  async function loadLocalEnv() {
    localLoading = true;
    errorMsg = '';
    try {
      const out = await invoke<string>('exec_custom_command', {
        cmd: 'env | sort',
        useSudo: false,
      });
      localVars = parseEnvLines(out);
      localLoaded = true;
    } catch (err) {
      errorMsg = `Load failed: ${formatInvokeError(err)}`;
    } finally {
      localLoading = false;
    }
  }

  async function addVar() {
    if (!newKey.trim()) return;
    adding = true;
    try {
      const escapedValue = newValue.replace(/'/g, "'\\''");
      const line = `export ${newKey.trim()}='${escapedValue}'`;
      const target = persistTarget === '/etc/environment'
        ? `/etc/environment`
        : persistTarget;
      const useSudo = persistTarget === '/etc/environment';
      const cmd = `echo ${shQuote(line)} >> ${shQuote(target)}`;
      await invoke('exec_custom_command', { cmd, useSudo });
      notifications.success(`Variable added to ${persistTarget}`);
      showAddForm = false;
      newKey = '';
      newValue = '';
      await loadLocalEnv();
    } catch (err) {
      errorMsg = `Failed to add variable: ${formatInvokeError(err)}`;
    } finally {
      adding = false;
    }
  }

  function cancelAdd() {
    showAddForm = false;
    newKey = '';
    newValue = '';
  }

  async function loadContainers() {
    containers = await listContainers(false);
    if (containers.length && !selectedContainer) selectedContainer = containers[0];
  }

  async function loadDockerEnv() {
    if (!selectedContainer) return;
    dockerLoading = true;
    errorMsg = '';
    try {
      const out = await invoke<string>('exec_custom_command', {
        cmd: `docker exec ${shQuote(selectedContainer)} env 2>/dev/null | sort`,
        useSudo: false,
      });
      dockerVars = parseEnvLines(out);
    } catch (err) {
      errorMsg = formatInvokeError(err);
    } finally {
      dockerLoading = false;
    }
  }

  async function copyValue(v: string) {
    try {
      await navigator.clipboard.writeText(v);
      notifications.success("Copied to clipboard");
    } catch {}
  }

  onMount(() => {
    loadContainers();
  });
</script>

<div class="env-manager manager-shell fade-in">
  <header class="manager-header">
    <h1 class="page-title">Environment variables</h1>
  </header>

  <div class="sub-tabs">
    <button class="sub-tab" class:active={tab === 'local'} onclick={() => (tab = 'local')}><KeyRound size={14} /> Local</button>
    <button class="sub-tab" class:active={tab === 'docker'} onclick={() => { tab = 'docker'; loadContainers(); }}><Box size={14} /> Docker container</button>
  </div>

  {#if tab === 'local'}
    <div class="control-bar glass">
      <button class="secondary btn-compact" disabled={localLoading} onclick={loadLocalEnv}><RefreshCw size={14} /> {localLoading ? "Loading…" : "Load"}</button>
      {#if localLoaded}
        <button class="secondary btn-compact" onclick={() => (showAddForm = !showAddForm)}><Plus size={14} /> Add variable</button>
      {/if}
      <span class="var-count">{localLoaded ? `${localVars.length} vars` : ''}</span>
    </div>

    {#if showAddForm}
      <div class="add-form glass">
        <input class="k-input" bind:value={newKey} placeholder="KEY" onkeydown={(e) => e.key === 'Enter' && addVar()} />
        <span class="eq">=</span>
        <input class="v-input" bind:value={newValue} placeholder="value" onkeydown={(e) => e.key === 'Enter' && addVar()} />
        <span class="persist-label">Persist to</span>
        <select class="target-select" bind:value={persistTarget}>
          {#each PERSIST_TARGETS as t}<option value={t}>{t}</option>{/each}
        </select>
        <button class="icon-mini success" disabled={adding || !newKey.trim()} onclick={addVar} title="Save"><Check size={13} /></button>
        <button class="icon-mini" onclick={cancelAdd} title="Cancel"><X size={13} /></button>
      </div>
    {/if}

    {#if !localLoaded}
      <div class="empty glass"><KeyRound size={22} /> Load to view the server's current environment variables.</div>
    {:else}
      <div class="var-list">
        {#each localVars as v}
          <div class="var-row glass ro">
            <span class="k-static">{v.key}</span>
            <span class="eq">=</span>
            <span class="v-static">{v.revealed || !isSecret(v.key) ? v.value : '••••••••'}</span>
            {#if isSecret(v.key)}
              <button class="icon-mini" onclick={() => (v.revealed = !v.revealed)} title={v.revealed ? "Hide" : "Reveal"}>
                {#if v.revealed}<EyeOff size={13} />{:else}<Eye size={13} />{/if}
              </button>
            {/if}
            <button class="icon-mini" onclick={() => copyValue(v.value)} title="Download"><Copy size={13} /></button>
          </div>
        {/each}
      </div>
    {/if}
  {:else}
    <div class="control-bar glass">
      <select class="container-select" bind:value={selectedContainer}>
        {#if containers.length === 0}<option value="">No running containers</option>{/if}
        {#each containers as c}<option value={c}>{c}</option>{/each}
      </select>
      <button class="secondary btn-compact" onclick={loadContainers} title="Refresh containers"><RefreshCw size={14} /></button>
      <button class="primary btn-compact" disabled={!selectedContainer || dockerLoading} onclick={loadDockerEnv}>{dockerLoading ? "Loading…" : "Load"}</button>
      <span class="readonly-tag">read-only</span>
    </div>

    <div class="var-list">
      {#each dockerVars as v}
        <div class="var-row glass ro">
          <span class="k-static">{v.key}</span>
          <span class="eq">=</span>
          <span class="v-static">{v.revealed || !isSecret(v.key) ? v.value : '••••••••'}</span>
          {#if isSecret(v.key)}
            <button class="icon-mini" onclick={() => (v.revealed = !v.revealed)} title={v.revealed ? "Hide" : "Reveal"}>
              {#if v.revealed}<EyeOff size={13} />{:else}<Eye size={13} />{/if}
            </button>
          {/if}
          <button class="icon-mini" onclick={() => copyValue(v.value)} title="Download"><Copy size={13} /></button>
        </div>
      {/each}
      {#if dockerVars.length === 0}
        <div class="empty glass"><Box size={22} /> Pick a container and load to view its environment variables.</div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .sub-tabs { display: flex; gap: 6px; flex-shrink: 0; }
  .sub-tab { display: flex; align-items: center; gap: 6px; background: transparent; border: 1px solid var(--border-color); color: var(--text-secondary); padding: 6px 12px; font-size: 0.8rem; border-radius: var(--radius-sm); cursor: pointer; }
  .sub-tab.active { background: var(--bg-active); color: var(--accent-amber); border-color: rgba(245,158,11,0.25); }
  .control-bar { display: flex; gap: 8px; align-items: center; padding: 10px; border-radius: var(--radius-md); flex-shrink: 0; flex-wrap: wrap; }
  .var-count { font-size: 0.72rem; color: var(--text-muted); margin-left: auto; }
  .add-form { display: flex; align-items: center; gap: 8px; padding: 10px; border-radius: var(--radius-md); flex-shrink: 0; flex-wrap: wrap; }
  .persist-label { font-size: 0.75rem; color: var(--text-muted); white-space: nowrap; }
  .target-select { background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--radius-sm); padding: 6px 8px; color: var(--text-primary); font-family: var(--font-mono); font-size: 0.78rem; }
  .container-select { flex: 1; max-width: 360px; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--radius-sm); padding: 7px 10px; color: var(--text-primary); font-size: 0.82rem; }
  .readonly-tag { font-size: 0.7rem; color: var(--text-muted); text-transform: uppercase; letter-spacing: 0.05em; margin-left: auto; }
  .empty { display: flex; flex-direction: column; align-items: center; gap: 10px; padding: 40px; color: var(--text-muted); border-radius: var(--radius-md); }
  .var-list { flex: 1; overflow: auto; display: flex; flex-direction: column; gap: 6px; }
  .var-row { display: flex; align-items: center; gap: 8px; padding: 6px 10px; border-radius: var(--radius-sm); }
  .var-row.ro { background: rgba(0,0,0,0.15); }
  .k-input { width: 220px; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--radius-sm); padding: 6px 8px; color: var(--accent-amber); font-family: var(--font-mono); font-size: 0.78rem; }
  .v-input { flex: 1; min-width: 160px; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: var(--radius-sm); padding: 6px 8px; color: var(--text-primary); font-family: var(--font-mono); font-size: 0.78rem; }
  .k-static { width: 260px; color: var(--accent-amber); font-family: var(--font-mono); font-size: 0.78rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .v-static { flex: 1; color: var(--text-secondary); font-family: var(--font-mono); font-size: 0.78rem; word-break: break-all; }
  .eq { color: var(--text-muted); }
  .icon-mini { background: transparent; border: 1px solid var(--border-color); color: var(--text-muted); border-radius: var(--radius-sm); padding: 5px 6px; cursor: pointer; display: flex; }
  .icon-mini:hover { color: var(--text-primary); background: var(--bg-hover); }
  .icon-mini.success:hover { color: var(--accent-green); border-color: rgba(34,197,94,0.3); }
  .icon-mini:disabled { opacity: 0.4; cursor: default; }
</style>
