<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { RefreshCw, Plus, Trash2, Play, BookOpen, Terminal, Loader2 } from 'lucide-svelte';
  import SudoModal from './SudoModal.svelte';
  import type { ProfileExtras, Runbook } from '$lib/admin/types';
  import { DEFAULT_PROFILE_EXTRAS } from '$lib/admin/types';
  import { get } from 'svelte/store';
  import { LL } from '$lib/i18n/i18n-svelte';
  import {
    formatInvokeError,
    isSudoPasswordRequired,
  } from '$lib/i18n/backendErrors';

  let { profileId = '' } = $props();

  let extras = $state<ProfileExtras>({ ...DEFAULT_PROFILE_EXTRAS });
  let isLoading = $state(false);
  let isRunning = $state(false);
  let errorMsg = $state('');

  let showAddModal = $state(false);
  let editId = $state<string | null>(null);
  let formName = $state('');
  let formCommand = $state('');
  let formUseSudo = $state(false);

  let showOutputModal = $state(false);
  let outputTitle = $state('');
  let outputContent = $state('');

  let showSudoModal = $state(false);
  let pendingAction: (() => Promise<void>) | null = null;

  const PRESETS = $derived.by(() => {
    const ll = get(LL);
    return [
      { name: ll.runbook.presetDockerPrune(), command: 'docker system prune -f', use_sudo: false },
      { name: ll.runbook.presetDockerComposePs(), command: 'docker compose ps', use_sudo: false },
      { name: ll.runbook.presetNginxTest(), command: 'nginx -t', use_sudo: true },
      { name: ll.runbook.presetNginxReload(), command: 'systemctl reload nginx', use_sudo: true },
      { name: ll.runbook.presetNginxRestart(), command: 'systemctl restart nginx', use_sudo: true },
    ];
  });

  async function exec(cmd: string, useSudo: boolean): Promise<string> {
    return invoke<string>('exec_custom_command', { cmd, useSudo });
  }

  async function withSudo(action: () => Promise<void>) {
    try {
      await action();
    } catch (err: unknown) {
      if (isSudoPasswordRequired(err)) {
        pendingAction = () => withSudo(action);
        showSudoModal = true;
      } else {
        errorMsg = formatInvokeError(err);
      }
    }
  }

  async function loadExtras() {
    if (!profileId) return;
    isLoading = true;
    try {
      extras = await invoke<ProfileExtras>('get_profile_extras', { profileId });
    } catch (err: unknown) {
      errorMsg = formatInvokeError(err);
    } finally {
      isLoading = false;
    }
  }

  async function saveExtras() {
    await invoke('save_profile_extras', { profileId, extras });
  }

  function openAdd() {
    editId = null;
    formName = '';
    formCommand = '';
    formUseSudo = false;
    showAddModal = true;
  }

  function openEdit(rb: Runbook) {
    editId = rb.id;
    formName = rb.name;
    formCommand = rb.command;
    formUseSudo = rb.use_sudo;
    showAddModal = true;
  }

  function addPreset(p: { name: string; command: string; use_sudo: boolean }) {
    formName = p.name;
    formCommand = p.command;
    formUseSudo = p.use_sudo;
    editId = null;
    showAddModal = true;
  }

  async function saveRunbook() {
    if (!formName.trim() || !formCommand.trim()) {
      alert(get(LL).runbook.alertNameAndCommand());
      return;
    }
    const rb: Runbook = {
      id: editId || Date.now().toString(),
      name: formName.trim(),
      command: formCommand.trim(),
      use_sudo: formUseSudo,
    };
    if (editId) {
      extras.runbooks = extras.runbooks.map((r) => (r.id === editId ? rb : r));
    } else {
      extras.runbooks = [...extras.runbooks, rb];
    }
    await saveExtras();
    showAddModal = false;
  }

  async function deleteRunbook(id: string) {
    if (!confirm(get(LL).runbook.confirmDelete())) return;
    extras.runbooks = extras.runbooks.filter((r) => r.id !== id);
    await saveExtras();
  }

  async function runRunbook(rb: Runbook) {
    isRunning = true;
    outputTitle = rb.name;
    outputContent = get(LL).runbook.running();
    showOutputModal = true;
    errorMsg = '';

    const run = async () => {
      try {
        outputContent = await exec(rb.command, rb.use_sudo);
      } catch (err: unknown) {
        outputContent = get(LL).runbook.errorPrefix({ error: formatInvokeError(err) });
      } finally {
        isRunning = false;
      }
    };

    if (rb.use_sudo) {
      await withSudo(run);
    } else {
      await run();
    }
    isRunning = false;
  }

  onMount(loadExtras);

  $effect(() => {
    if (profileId) loadExtras();
  });
</script>

<div class="runbooks manager-shell scrollable fade-in">
  <header class="manager-header">
    <h1 class="page-title">{$LL.runbook.title()}</h1>
    <div class="header-actions">
      <button class="secondary btn-compact" disabled={isLoading} onclick={loadExtras}>
        <RefreshCw size={14} /> {$LL.common.refresh()}
      </button>
      <button class="primary btn-compact" onclick={openAdd}>
        <Plus size={14} /> {$LL.common.new()}
      </button>
    </div>
  </header>

  {#if errorMsg}
    <div class="error-banner">{errorMsg}</div>
  {/if}

  <section class="presets glass">
    <h3>{$LL.runbook.quickPresets()}</h3>
    <div class="preset-chips">
      {#each PRESETS as p}
        <button class="chip" onclick={() => addPreset(p)}>{p.name}</button>
      {/each}
    </div>
  </section>

  {#if extras.runbooks.length === 0}
    <div class="empty glass">
      <BookOpen size={36} class="muted" />
      <p>{$LL.runbook.empty()}</p>
    </div>
  {:else}
    <div class="runbook-list">
      {#each extras.runbooks as rb}
        <div class="runbook-card glass">
          <div class="rb-info">
            <Terminal size={16} class="accent-amber-text" />
            <div>
              <div class="rb-name">{rb.name}</div>
              <div class="rb-cmd mono-val">{rb.command}</div>
            </div>
            {#if rb.use_sudo}
              <span class="badge warning">{$LL.runbook.requiresSudo()}</span>
            {/if}
          </div>
          <div class="rb-actions">
            <button class="primary btn-compact" disabled={isRunning} onclick={() => runRunbook(rb)}>
              {#if isRunning}<Loader2 size={14} class="spin" />{:else}<Play size={14} />{/if}
              {$LL.common.run()}
            </button>
            <button class="secondary btn-compact" onclick={() => openEdit(rb)}>{$LL.common.edit()}</button>
            <button class="secondary btn-compact" onclick={() => deleteRunbook(rb.id)}>
              <Trash2 size={14} />
            </button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

{#if showAddModal}
  <div class="modal-overlay" role="presentation" onclick={() => (showAddModal = false)}>
    <div class="modal glass" role="dialog" onclick={(e) => e.stopPropagation()}>
      <h3>{editId ? $LL.runbook.editRunbook() : $LL.runbook.newRunbook()}</h3>
      <label>{$LL.runbook.name()}<input bind:value={formName} placeholder={$LL.runbook.namePlaceholder()} /></label>
      <label>{$LL.runbook.command()}<textarea bind:value={formCommand} rows="3" placeholder={$LL.runbook.commandPlaceholder()}></textarea></label>
      <label class="checkbox-row">
        <input type="checkbox" bind:checked={formUseSudo} />
        {$LL.runbook.requiresSudo()}
      </label>
      <div class="modal-actions">
        <button class="secondary" onclick={() => (showAddModal = false)}>{$LL.common.cancel()}</button>
        <button class="primary" onclick={saveRunbook}>{$LL.common.save()}</button>
      </div>
    </div>
  </div>
{/if}

{#if showOutputModal}
  <div class="modal-overlay" role="presentation" onclick={() => (showOutputModal = false)}>
    <div class="modal output-modal glass" role="dialog" onclick={(e) => e.stopPropagation()}>
      <div class="output-header">
        <h3>{outputTitle}</h3>
        <button class="secondary btn-compact" onclick={() => (showOutputModal = false)}>{$LL.common.close()}</button>
      </div>
      <pre class="output">{outputContent}</pre>
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
  .header-actions { display: flex; gap: 8px; }
  .presets { padding: 12px; border-radius: var(--radius-md); }
  .presets h3 { font-size: 0.85rem; color: white; margin-bottom: 8px; }
  .preset-chips { display: flex; flex-wrap: wrap; gap: 6px; }
  .chip {
    background: var(--bg-hover);
    border: 1px solid var(--border-color);
    color: var(--text-secondary);
    padding: 4px 10px;
    font-size: 0.75rem;
    border-radius: var(--radius-sm);
  }
  .chip:hover { border-color: rgba(245, 158, 11, 0.3); color: var(--accent-amber); }
  .runbook-list { display: flex; flex-direction: column; gap: 8px; }
  .runbook-card { padding: 12px; border-radius: var(--radius-md); display: flex; justify-content: space-between; align-items: center; gap: 12px; flex-wrap: wrap; }
  .rb-info { display: flex; align-items: flex-start; gap: 10px; flex: 1; min-width: 200px; }
  .rb-name { font-weight: 600; color: white; font-size: 0.9rem; }
  .rb-cmd { font-size: 0.75rem; color: var(--text-muted); margin-top: 4px; word-break: break-all; }
  .rb-actions { display: flex; gap: 6px; }
  .empty { padding: 40px; text-align: center; display: flex; flex-direction: column; align-items: center; gap: 12px; border-radius: var(--radius-md); }
  .empty .muted { color: var(--text-muted); }
  .error-banner { padding: 10px; border-radius: var(--radius-sm); background: var(--accent-red-glow); color: #ff8585; font-size: 0.85rem; }
  .modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.6); display: flex; align-items: center; justify-content: center; z-index: 1000; }
  .modal { width: 480px; padding: 20px; border-radius: var(--radius-md); display: flex; flex-direction: column; gap: 10px; }
  .output-modal { width: 640px; max-height: 80vh; }
  .modal h3 { color: white; }
  .modal label { display: flex; flex-direction: column; gap: 4px; font-size: 0.8rem; color: var(--text-secondary); }
  .checkbox-row { flex-direction: row !important; align-items: center; gap: 8px !important; }
  .modal-actions { display: flex; justify-content: flex-end; gap: 8px; }
  .output-header { display: flex; justify-content: space-between; align-items: center; }
  .output { flex: 1; overflow: auto; max-height: 400px; font-family: var(--font-mono); font-size: 0.75rem; color: var(--text-secondary); white-space: pre-wrap; background: rgba(0,0,0,0.3); padding: 12px; border-radius: var(--radius-sm); }
  .spin { animation: spin 1s linear infinite; }
  @keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
</style>
