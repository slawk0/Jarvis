<script lang="ts">
  import { KeyRound } from 'lucide-svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { LL } from '$lib/i18n/i18n-svelte';
  import { formatInvokeError } from '$lib/i18n/backendErrors';

  let {
    open = $bindable(false),
    onSuccess = () => {},
    title = undefined as string | undefined,
    description = undefined as string | undefined,
  } = $props();

  let sudoPassword = $state('');
  let sudoError = $state('');
  let isSubmitting = $state(false);

  async function submit() {
    sudoError = '';
    isSubmitting = true;
    try {
      await invoke('set_sudo_password', { password: sudoPassword });
      open = false;
      sudoPassword = '';
      onSuccess();
    } catch (err: unknown) {
      sudoError = formatInvokeError(err);
    } finally {
      isSubmitting = false;
    }
  }

  function close() {
    open = false;
    sudoPassword = '';
    sudoError = '';
  }
</script>

{#if open}
  <div class="modal-overlay" role="presentation" onclick={close}>
    <div class="modal glass" role="dialog" onclick={(e) => e.stopPropagation()}>
      <div class="modal-header">
        <KeyRound size={18} />
        <h3>{title ?? $LL.sudo.titleRequired()}</h3>
      </div>
      <p class="modal-desc">{description ?? $LL.sudo.descRequired()}</p>
      <input
        type="password"
        placeholder={$LL.sudo.passwordPlaceholder()}
        bind:value={sudoPassword}
        onkeydown={(e) => e.key === 'Enter' && submit()}
      />
      {#if sudoError}
        <div class="error-msg">{sudoError}</div>
      {/if}
      <div class="modal-actions">
        <button class="secondary" onclick={close}>{$LL.common.cancel()}</button>
        <button class="primary" disabled={isSubmitting || !sudoPassword} onclick={submit}>
          {isSubmitting ? $LL.sudo.verifying() : $LL.common.confirm()}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .modal {
    width: 400px;
    padding: 20px;
    border-radius: var(--radius-md);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .modal-header {
    display: flex;
    align-items: center;
    gap: 10px;
    color: white;
  }

  .modal-header h3 {
    font-size: 1rem;
  }

  .modal-desc {
    font-size: 0.85rem;
    color: var(--text-secondary);
  }

  .error-msg {
    color: var(--accent-red);
    font-size: 0.8rem;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }
</style>
