<script lang="ts">
  import { onMount } from 'svelte';
  import "../global.css";
  import { initLocale, isLocaleInitialized } from '$lib/i18n/localeStore.svelte';
  import { LL } from '$lib/i18n/i18n-svelte';
  import ToastContainer from '../components/ui/ToastContainer.svelte';

  let { children } = $props();
  let localeReady = $state(isLocaleInitialized());

  onMount(async () => {
    await initLocale();
    localeReady = true;
  });
</script>

{#if localeReady}
  {@render children()}
  <ToastContainer />
{:else}
  <div class="locale-loading" aria-busy="true" aria-live="polite">
    {$LL.common.loading()}
  </div>
{/if}

<style>
  .locale-loading {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100vw;
    height: 100vh;
    background: var(--bg-primary);
    color: var(--text-secondary);
    font-size: 0.9rem;
  }
</style>
