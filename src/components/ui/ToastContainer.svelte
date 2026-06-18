<script lang="ts">
  import { notifications } from '$lib/notifications.svelte';
  import { AlertCircle, Check, Info, AlertTriangle, X } from 'lucide-svelte';
  import { fly } from 'svelte/transition';
</script>

<div class="toast-container">
  {#each notifications.list as toast (toast.id)}
    <div
      class="toast {toast.type}"
      in:fly={{ x: 300, duration: 300, opacity: 0 }}
      out:fly={{ y: -20, duration: 200, opacity: 0 }}
      role="alert"
    >
      <div class="toast-icon">
        {#if toast.type === 'error'}
          <AlertCircle size={18} />
        {:else}
          {#if toast.type === 'success'}
            <Check size={18} />
          {:else}
            {#if toast.type === 'warning'}
              <AlertTriangle size={18} />
            {:else}
              <Info size={18} />
            {/if}
          {/if}
        {/if}
      </div>

      <div class="toast-content">
        <p class="toast-message">{toast.message}</p>
      </div>

      <button
        class="toast-close"
        onclick={() => notifications.dismiss(toast.id)}
        aria-label="Dismiss"
      >
        <X size={14} />
      </button>
    </div>
  {/each}
</div>

<style>
  .toast-container {
    position: fixed;
    top: 16px;
    right: 16px;
    z-index: 9999;
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-width: 380px;
    width: calc(100vw - 32px);
    pointer-events: none;
  }

  .toast {
    pointer-events: auto;
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 12px 16px;
    border-radius: var(--radius-md);
    border: 1px solid var(--border-subtle);
    background: rgba(22, 27, 34, 0.9);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
    color: var(--text-primary);
    font-family: var(--font-sans);
    font-size: 0.85rem;
    line-height: 1.4;
    position: relative;
    overflow: hidden;
  }

  .toast-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    padding-top: 2px;
    flex-shrink: 0;
  }

  .toast-content {
    flex: 1;
    min-width: 0; /* Prevents overflow */
  }

  .toast-message {
    white-space: pre-wrap;
    word-break: break-word;
    text-wrap: pretty;
  }

  .toast-close {
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 4px;
    border-radius: 50%;
    margin-left: 8px;
    flex-shrink: 0;
    transition: background-color 0.15s ease, color 0.15s ease, transform 0.1s ease;
  }

  .toast-close:hover {
    background: rgba(255, 255, 255, 0.08);
    color: var(--text-primary);
  }

  .toast-close:active {
    transform: scale(0.96);
  }

  /* Type modifiers */
  .toast.error {
    border-left: 3px solid var(--accent-red);
  }
  .toast.error .toast-icon {
    color: var(--accent-red);
  }

  .toast.success {
    border-left: 3px solid var(--accent-green);
  }
  .toast.success .toast-icon {
    color: var(--accent-green);
  }

  .toast.warning {
    border-left: 3px solid #f59e0b;
  }
  .toast.warning .toast-icon {
    color: #f59e0b;
  }

  .toast.info {
    border-left: 3px solid var(--accent-primary);
  }
  .toast.info .toast-icon {
    color: #3b82f6;
  }
</style>
