<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import {
    RefreshCw, Download, RotateCw, AlertTriangle, Package, Loader2, Power,
  } from 'lucide-svelte';
  import SudoModal from './SudoModal.svelte';

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

  let showSudoModal = $state(false);
  let pendingAction: (() => Promise<void>) | null = null;

  async function exec(cmd: string, useSudo = false): Promise<string> {
    return invoke<string>('exec_custom_command', { cmd, useSudo });
  }

  async function withSudo(action: () => Promise<void>) {
    try {
      await action();
    } catch (err: any) {
      if (err.toString() === 'SUDO_PASSWORD_REQUIRED') {
        pendingAction = () => withSudo(action);
        showSudoModal = true;
      } else {
        errorMsg = err.toString();
      }
    }
  }

  async function loadStatus() {
    isLoading = true;
    errorMsg = '';
    try {
      const listOut = await exec(
        'apt list --upgradable 2>/dev/null | grep -v "^Listing" | grep -v "^$"',
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
    } catch (err: any) {
      errorMsg = 'Nie udało się pobrać statusu: ' + err.toString();
    } finally {
      isLoading = false;
    }
  }

  async function runAptUpdate() {
    isRunningAction = true;
    actionOutput = '';
    await withSudo(async () => {
      try {
        actionOutput = await exec('DEBIAN_FRONTEND=noninteractive apt-get update -y 2>&1', true);
        showOutput = true;
        await loadStatus();
      } catch (err: any) {
        actionOutput = err.toString();
        showOutput = true;
      } finally {
        isRunningAction = false;
      }
    });
    isRunningAction = false;
  }

  async function runAptUpgrade() {
    if (!confirm(`Zaktualizować ${upgradableCount} pakiet(ów)? Operacja może potrwać kilka minut.`)) {
      return;
    }
    isRunningAction = true;
    actionOutput = '';
    await withSudo(async () => {
      try {
        actionOutput = await exec(
          'DEBIAN_FRONTEND=noninteractive apt-get upgrade -y 2>&1',
          true,
        );
        showOutput = true;
        await loadStatus();
      } catch (err: any) {
        actionOutput = err.toString();
        showOutput = true;
      } finally {
        isRunningAction = false;
      }
    });
    isRunningAction = false;
  }

  async function runReboot() {
    if (!confirm('Czy na pewno zrestartować serwer? Połączenie zostanie przerwane.')) return;
    isRunningAction = true;
    await withSudo(async () => {
      try {
        await exec('nohup bash -c "sleep 2 && reboot" >/dev/null 2>&1 &', true);
        actionOutput = 'Serwer restartuje się... Połączenie zostanie utracone.';
        showOutput = true;
      } catch (err: any) {
        actionOutput = err.toString();
        showOutput = true;
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
    <h1 class="page-title">Konserwacja systemu</h1>
    <div class="header-actions">
      <button class="secondary btn-compact" disabled={isLoading} onclick={loadStatus}>
        <RefreshCw size={14} class={isLoading ? 'spin' : ''} /> Odśwież
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
        <span>Pakiety do aktualizacji</span>
      </div>
      <div class="big-val mono-val">{upgradableCount}</div>
      <p class="card-desc">Wykryte przez apt list --upgradable</p>
    </div>

    <div class="status-card glass {rebootRequired ? 'warn' : ''}">
      <div class="card-top">
        <Power size={18} class={rebootRequired ? 'accent-red-text' : 'accent-green-text'} />
        <span>Wymagany restart</span>
      </div>
      <div class="big-val">{rebootRequired ? 'TAK' : 'NIE'}</div>
      {#if rebootReason}
        <p class="card-desc mono-val">{rebootReason.slice(0, 120)}</p>
      {:else}
        <p class="card-desc">Brak pliku /var/run/reboot-required</p>
      {/if}
    </div>

    <div class="status-card glass">
      <div class="card-top">
        <Download size={18} class="accent-green-text" />
        <span>Automatyczne aktualizacje</span>
      </div>
      <div class="big-val">
        {#if unattendedEnabled === null}
          ?
        {:else if unattendedEnabled}
          WŁĄCZONE
        {:else}
          WYŁĄCZONE
        {/if}
      </div>
      <p class="card-desc">Pakiet unattended-upgrades</p>
    </div>
  </section>

  <section class="actions-panel glass">
    <h3>Akcje</h3>
    <div class="action-buttons">
      <button class="secondary btn-compact" disabled={isRunningAction} onclick={runAptUpdate}>
        {#if isRunningAction}<Loader2 size={14} class="spin" />{:else}<RefreshCw size={14} />{/if}
        apt update
      </button>
      <button
        class="primary btn-compact"
        disabled={isRunningAction || upgradableCount === 0}
        onclick={runAptUpgrade}
      >
        {#if isRunningAction}<Loader2 size={14} class="spin" />{:else}<Download size={14} />{/if}
        apt upgrade ({upgradableCount})
      </button>
      <button
        class="danger btn-compact"
        disabled={isRunningAction}
        onclick={runReboot}
      >
        <RotateCw size={14} /> Restart serwera
      </button>
    </div>
    {#if rebootRequired}
      <div class="warn-banner">
        <AlertTriangle size={16} />
        <span>Kernel lub kluczowe pakiety wymagają restartu serwera.</span>
      </div>
    {/if}
  </section>

  {#if upgradableList.length > 0}
    <section class="packages-panel glass">
      <h3>Pakiety oczekujące ({upgradableList.length})</h3>
      <div class="packages-list mono-val">
        {#each upgradableList.slice(0, 50) as pkg}
          <div class="pkg-line">{pkg.split('/')[0]}</div>
        {/each}
        {#if upgradableList.length > 50}
          <div class="pkg-more">... i {upgradableList.length - 50} więcej</div>
        {/if}
      </div>
    </section>
  {/if}

  {#if showOutput && actionOutput}
    <section class="output-panel glass">
      <div class="output-header">
        <h3>Wynik operacji</h3>
        <button class="secondary btn-compact" onclick={() => (showOutput = false)}>Zamknij</button>
      </div>
      <pre class="output-content">{actionOutput}</pre>
    </section>
  {/if}
</div>

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
</style>
