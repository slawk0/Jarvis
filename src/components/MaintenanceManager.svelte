<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import {
    RefreshCw, Download, RotateCw, AlertTriangle, Package, Loader2, Power,
  } from 'lucide-svelte';
  import SudoModal from './SudoModal.svelte';
  import { get } from 'svelte/store';
  import { LL } from '$lib/i18n/i18n-svelte';
  import {
    formatInvokeError,
    isSudoPasswordRequired,
    parseAppError,
  } from '$lib/i18n/backendErrors';

  let {
    onDisconnect = () => {},
  } = $props();

  let isLoading = $state(false);
  let isRunningAction = $state(false);
  let errorMsg = $state('');
  let actionOutput = $state('');
  let showOutput = $state(false);

  let upgradableCount = $state(0);
  let upgradableList = $state<string[]>([]);
  let rebootRequired = $state(false);
  let rebootReason = $state('');
  let unattendedEnabled = $state<boolean | null>(null);

  let showConfirmUpgradeModal = $state(false);
  let isRebooting = $state(false);
  let showSudoModal = $state(false);
  let pendingAction: (() => Promise<void>) | null = null;

  async function exec(cmd: string, useSudo = false): Promise<string> {
    return invoke<string>('exec_custom_command', { cmd, useSudo });
  }

  async function execStream(cmd: string, useSudo = false, onChunk: (text: string) => void): Promise<void> {
    const eventId = Math.random().toString(36).substring(7);
    const unlistenStdout = await listen<string>(`exec-stdout-${eventId}`, (event) => {
      onChunk(event.payload);
    });
    const unlistenStderr = await listen<string>(`exec-stderr-${eventId}`, (event) => {
      onChunk(event.payload);
    });
    try {
      await invoke('exec_custom_command_stream', { cmd, useSudo, eventId });
    } finally {
      unlistenStdout();
      unlistenStderr();
    }
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

  async function loadStatus() {
    isLoading = true;
    errorMsg = '';
    try {
      const listOut = await exec(
        'apt list --upgradable 2>/dev/null | grep -v "^Listing" | grep -v "^$" || true',
      );
      const lines = listOut.trim().split('\n').filter(Boolean);
      upgradableList = lines;
      upgradableCount = lines.length;

      try {
        const rebootOut = await exec('cat /var/run/reboot-required 2>/dev/null || echo ""');
        rebootRequired = rebootOut.trim().length > 0;
        rebootReason = rebootOut.trim();
      } catch {
        rebootRequired = false;
        rebootReason = '';
      }

      try {
        const uaOut = await exec(
          'dpkg -l unattended-upgrades 2>/dev/null | grep "^ii" || echo "not-installed"',
        );
        unattendedEnabled = uaOut.includes('unattended-upgrades') && !uaOut.includes('not-installed');
      } catch {
        unattendedEnabled = null;
      }
    } catch (err: unknown) {
      errorMsg = get(LL).maintenance.statusLoadFailed({ error: formatInvokeError(err) });
    } finally {
      isLoading = false;
    }
  }

  async function runAptUpdate() {
    isRunningAction = true;
    actionOutput = '';
    showOutput = true;
    await withSudo(async () => {
      try {
        actionOutput = '';
        await execStream(
          'env DEBIAN_FRONTEND=noninteractive apt-get update -y 2>&1',
          true,
          (chunk) => {
            actionOutput += chunk;
          }
        );
        await loadStatus();
      } catch (err: unknown) {
        if (isSudoPasswordRequired(err)) {
          throw err;
        }
        actionOutput += `\nError: ${formatInvokeError(err)}`;
      } finally {
        isRunningAction = false;
      }
    });
    isRunningAction = false;
  }

  async function runAptUpgrade() {
    showConfirmUpgradeModal = false;
    isRunningAction = true;
    actionOutput = '';
    showOutput = true;
    await withSudo(async () => {
      try {
        actionOutput = '';
        await execStream(
          'env DEBIAN_FRONTEND=noninteractive apt-get upgrade -y 2>&1',
          true,
          (chunk) => {
            actionOutput += chunk;
          }
        );
        await loadStatus();
      } catch (err: unknown) {
        if (isSudoPasswordRequired(err)) {
          throw err;
        }
        actionOutput += `\nError: ${formatInvokeError(err)}`;
      } finally {
        isRunningAction = false;
      }
    });
    isRunningAction = false;
  }

  async function runReboot() {
    if (!confirm(get(LL).maintenance.confirmReboot())) return;
    isRunningAction = true;
    let disconnectTimer: any = null;
    await withSudo(async () => {
      try {
        await exec(
          'sh -c "reboot || /sbin/reboot || /usr/sbin/reboot || shutdown -r now || /sbin/shutdown -r now || systemctl reboot"',
          true,
        );
        isRebooting = true;
        showOutput = false;
        disconnectTimer = setTimeout(() => {
          onDisconnect();
        }, 1500);
      } catch (err: unknown) {
        if (isSudoPasswordRequired(err)) {
          throw err;
        }
        
        const parsed = parseAppError(err);
        if (parsed?.code === 'REMOTE_COMMAND_FAILED') {
          actionOutput = formatInvokeError(err);
          showOutput = true;
          isRebooting = false;
          if (disconnectTimer) {
            clearTimeout(disconnectTimer);
          }
        } else {
          // Connection loss is expected when rebooting, so we treat it as success
          isRebooting = true;
          showOutput = false;
          disconnectTimer = setTimeout(() => {
            onDisconnect();
          }, 1500);
        }
      } finally {
        isRunningAction = false;
      }
    });
    isRunningAction = false;
  }

  onMount(loadStatus);
</script>

<div class="maintenance manager-shell scrollable fade-in">
  <header class="manager-header">
    <h1 class="page-title">{$LL.maintenance.title()}</h1>
    <div class="header-actions">
      <button class="secondary btn-compact" disabled={isLoading} onclick={loadStatus}>
        <RefreshCw size={14} class={isLoading ? 'spin' : ''} /> {$LL.common.refresh()}
      </button>
    </div>
  </header>

  {#if errorMsg}
    <div class="error-banner">{errorMsg}</div>
  {/if}

  <section class="status-grid">
    <div class="status-card glass">
      <div class="card-top">
        <Package size={18} class="accent-amber-text" />
        <span>{$LL.maintenance.packagesToUpdate()}</span>
      </div>
      <div class="big-val mono-val">{upgradableCount}</div>
      <p class="card-desc">{$LL.maintenance.detectedByApt()}</p>
    </div>

    <div class="status-card glass {rebootRequired ? 'warn' : ''}">
      <div class="card-top">
        <Power size={18} class={rebootRequired ? 'accent-red-text' : 'accent-green-text'} />
        <span>{$LL.maintenance.rebootRequired()}</span>
      </div>
      <div class="big-val">{rebootRequired ? $LL.common.yesUpper() : $LL.common.noUpper()}</div>
      {#if rebootReason}
        <p class="card-desc mono-val">{rebootReason.slice(0, 120)}</p>
      {:else}
        <p class="card-desc">{$LL.maintenance.noRebootFile()}</p>
      {/if}
    </div>

    <div class="status-card glass">
      <div class="card-top">
        <Download size={18} class="accent-green-text" />
        <span>{$LL.maintenance.autoUpdates()}</span>
      </div>
      <div class="big-val">
        {#if unattendedEnabled === null}
          {$LL.common.unknownQ()}
        {:else if unattendedEnabled}
          {$LL.common.enabledUpper()}
        {:else}
          {$LL.common.disabledUpper()}
        {/if}
      </div>
      <p class="card-desc">{$LL.maintenance.unattendedPackage()}</p>
    </div>
  </section>

  <section class="actions-panel glass">
    <h3>{$LL.maintenance.actions()}</h3>
    <div class="action-buttons">
      <button class="secondary btn-compact" disabled={isRunningAction} onclick={runAptUpdate}>
        {#if isRunningAction}<Loader2 size={14} class="spin" />{:else}<RefreshCw size={14} />{/if}
        {$LL.maintenance.aptUpdate()}
      </button>
      <button
        class="primary btn-compact"
        disabled={isRunningAction || upgradableCount === 0}
        onclick={() => (showConfirmUpgradeModal = true)}
      >
        {#if isRunningAction}<Loader2 size={14} class="spin" />{:else}<Download size={14} />{/if}
        {$LL.maintenance.aptUpgrade({ count: upgradableCount })}
      </button>
      <button
        class="danger btn-compact"
        disabled={isRunningAction}
        onclick={runReboot}
      >
        <RotateCw size={14} /> {$LL.maintenance.rebootServer()}
      </button>
    </div>
    {#if rebootRequired}
      <div class="warn-banner">
        <AlertTriangle size={16} />
        <span>{$LL.maintenance.rebootWarning()}</span>
      </div>
    {/if}
  </section>

  {#if upgradableList.length > 0}
    <section class="packages-panel glass">
      <h3>{$LL.maintenance.pendingPackages({ count: upgradableList.length })}</h3>
      <div class="packages-list mono-val">
        {#each upgradableList.slice(0, 50) as pkg}
          <div class="pkg-line">{pkg.split('/')[0]}</div>
        {/each}
        {#if upgradableList.length > 50}
          <div class="pkg-more">{$LL.maintenance.andMore({ count: upgradableList.length - 50 })}</div>
        {/if}
      </div>
    </section>
  {/if}

  {#if showOutput && actionOutput}
    <section class="output-panel glass">
      <div class="output-header">
        <h3>{$LL.maintenance.operationResult()}</h3>
        <button class="secondary btn-compact" onclick={() => (showOutput = false)}>{$LL.common.close()}</button>
      </div>
      <pre class="output-content">{actionOutput}</pre>
    </section>
  {/if}
</div>

{#if showConfirmUpgradeModal}
  <div class="modal-overlay" role="presentation" onclick={() => showConfirmUpgradeModal = false}>
    <div class="confirm-modal glass" role="dialog" onclick={(e) => e.stopPropagation()}>
      <div class="modal-header">
        <AlertTriangle size={20} class="accent-amber-text" />
        <h3>{$LL.docker.confirmTitle()}</h3>
      </div>
      <p class="modal-desc">
        {$LL.maintenance.confirmUpgrade({ count: upgradableCount })}
      </p>

      <div class="modal-pkg-list">
        {#each upgradableList.slice(0, 50) as pkg}
          <div class="modal-pkg-item">{pkg.split('/')[0]}</div>
        {/each}
        {#if upgradableList.length > 50}
          <div class="modal-pkg-more">{$LL.maintenance.andMore({ count: upgradableList.length - 50 })}</div>
        {/if}
      </div>

      <div class="modal-actions">
        <button class="secondary" onclick={() => { showConfirmUpgradeModal = false; }}>
          {$LL.common.cancel()}
        </button>
        <button class="primary" onclick={runAptUpgrade}>
          {$LL.common.confirm()}
        </button>
      </div>
    </div>
  </div>
{/if}

{#if isRebooting}
  <div class="modal-overlay">
    <div class="confirm-modal glass" role="dialog" style="display: flex; flex-direction: column; align-items: center; justify-content: center; text-align: center; gap: 1.5rem; padding: 2.5rem;">
      <Loader2 size={36} class="spin accent-blue-text" />
      <h3>{$LL.maintenance.rebootServer()}</h3>
      <p class="modal-desc" style="max-width: 320px; margin: 0;">
        {$LL.maintenance.rebooting()}
      </p>
    </div>
  </div>
{/if}

<SudoModal
  bind:open={showSudoModal}
  onSuccess={() => {
    if (pendingAction) {
      const action = pendingAction;
      pendingAction = null;
      isRunningAction = true;
      action();
    }
  }}
/>

<style>
  .status-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 10px;
  }

  .status-card {
    padding: 16px;
    border-radius: var(--radius-md);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .status-card.warn {
    border-color: rgba(239, 68, 68, 0.3);
  }

  .card-top {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.85rem;
    color: var(--text-secondary);
  }

  .big-val {
    font-size: 1.6rem;
    font-weight: 700;
    color: white;
    font-family: var(--font-display);
  }

  .card-desc {
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  .actions-panel, .packages-panel, .output-panel {
    padding: 16px;
    border-radius: var(--radius-md);
  }

  .actions-panel h3, .packages-panel h3, .output-panel h3 {
    font-size: 0.9rem;
    color: white;
    margin-bottom: 10px;
  }

  .action-buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .warn-banner {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 12px;
    padding: 10px;
    border-radius: var(--radius-sm);
    background: var(--accent-red-glow);
    border: 1px solid rgba(239, 68, 68, 0.25);
    color: #ff8585;
    font-size: 0.85rem;
  }

  .packages-list {
    max-height: 200px;
    overflow-y: auto;
    font-size: 0.78rem;
    color: var(--text-secondary);
  }

  .pkg-line {
    padding: 3px 0;
    border-bottom: 1px solid var(--border-color);
  }

  .pkg-more {
    padding: 8px 0;
    color: var(--text-muted);
  }

  .output-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 8px;
  }

  .output-content {
    max-height: 300px;
    overflow: auto;
    font-family: var(--font-mono);
    font-size: 0.75rem;
    color: var(--text-secondary);
    white-space: pre-wrap;
    background: rgba(0, 0, 0, 0.3);
    padding: 12px;
    border-radius: var(--radius-sm);
  }

  .error-banner {
    padding: 10px;
    border-radius: var(--radius-sm);
    background: var(--accent-red-glow);
    color: #ff8585;
    font-size: 0.85rem;
  }

  .header-actions {
    display: flex;
    gap: 8px;
  }

  .spin {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  @media (max-width: 900px) {
    .status-grid { grid-template-columns: 1fr; }
  }

  .modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    backdrop-filter: blur(4px);
  }

  .confirm-modal {
    width: 450px;
    max-width: 90%;
    padding: 24px;
    border-radius: var(--radius-lg);
    display: flex;
    flex-direction: column;
    gap: 16px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.5), 0 10px 10px -5px rgba(0, 0, 0, 0.5);
  }

  .modal-header {
    display: flex;
    align-items: center;
    gap: 10px;
    color: white;
  }

  .modal-header h3 {
    font-size: 1.1rem;
    font-weight: 600;
    margin: 0;
  }

  .modal-desc {
    font-size: 0.9rem;
    color: var(--text-secondary);
    line-height: 1.4;
    margin: 0;
  }

  .modal-pkg-list {
    max-height: 180px;
    overflow-y: auto;
    background: rgba(0, 0, 0, 0.25);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    padding: 8px 12px;
    font-size: 0.8rem;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .modal-pkg-item {
    font-family: var(--font-mono);
    color: var(--text-secondary);
    padding: 2px 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }

  .modal-pkg-item:last-child {
    border-bottom: none;
  }

  .modal-pkg-more {
    color: var(--text-muted);
    font-size: 0.75rem;
    padding-top: 4px;
    text-align: center;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 8px;
  }
</style>
