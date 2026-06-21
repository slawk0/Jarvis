<script lang="ts">
  import { onMount } from 'svelte';
  import { Server, Box, RefreshCw } from 'lucide-svelte';
    import { listContainers, type ExecTarget } from '$lib/exec/target';

  let {
    target = $bindable({ kind: 'host' } as ExecTarget),
    useSudo = false,
  } = $props();

  let containers = $state<string[]>([]);
  let loading = $state(false);

  async function refresh() {
    loading = true;
    containers = await listContainers(useSudo);
    loading = false;
  }

  function selectHost() {
    target = { kind: 'host' };
  }

  function selectContainer(name: string) {
    target = { kind: 'docker', container: name };
  }

  onMount(refresh);
</script>

<div class="target-picker">
  <span class="tp-label">Run on</span>
  <button class="tp-chip" class:active={target.kind === 'host'} onclick={selectHost}>
    <Server size={13} /> Host
  </button>
  {#each containers as c}
    <button
      class="tp-chip"
      class:active={target.kind === 'docker' && target.container === c}
      onclick={() => selectContainer(c)}
      title={c}
    >
      <Box size={13} /> {c}
    </button>
  {/each}
  <button class="tp-refresh" onclick={refresh} title="Refresh containers" aria-label="Refresh containers">
    <RefreshCw size={13} class={loading ? 'spin' : ''} />
  </button>
</div>

<style>
  .target-picker {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    flex-shrink: 0;
  }
  .tp-label {
    font-family: var(--font-mono);
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }
  .tp-chip {
    display: flex;
    align-items: center;
    gap: 5px;
    background: transparent;
    border: 1px solid var(--border-color);
    color: var(--text-secondary);
    padding: 4px 10px;
    font-size: 0.75rem;
    border-radius: var(--radius-sm);
    cursor: pointer;
    max-width: 180px;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .tp-chip:hover { background: var(--bg-hover); color: var(--text-primary); }
  .tp-chip.active {
    background: var(--bg-active);
    color: var(--accent-amber);
    border-color: rgba(245, 158, 11, 0.25);
  }
  .tp-refresh {
    background: transparent;
    border: 1px solid var(--border-color);
    color: var(--text-muted);
    padding: 4px 6px;
    border-radius: var(--radius-sm);
    cursor: pointer;
    display: flex;
    align-items: center;
  }
  .tp-refresh:hover { color: var(--text-primary); background: var(--bg-hover); }
</style>
