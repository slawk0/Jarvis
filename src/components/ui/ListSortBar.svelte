<script lang="ts">
  import type { SortDir } from '$lib/sort/sortUtils';

  interface Column {
    id: string;
    label: string;
  }

  interface Props {
    columns: Column[];
    activeColumn: string;
    direction: SortDir;
    onsort: (column: string) => void;
  }

  let { columns, activeColumn, direction, onsort }: Props = $props();
</script>

<div class="list-sort-bar">
  <span class="sort-label">Sortuj:</span>
  {#each columns as col}
    <button
      type="button"
      class="sort-btn"
      class:active={activeColumn === col.id}
      onclick={() => onsort(col.id)}
    >
      {col.label}
      {#if activeColumn === col.id}
        <span class="arrow">{direction === 'asc' ? '↑' : '↓'}</span>
      {/if}
    </button>
  {/each}
</div>

<style>
  .list-sort-bar {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    padding: 6px 0 8px;
    flex-shrink: 0;
  }

  .sort-label {
    font-size: 0.75rem;
    color: var(--text-muted);
    margin-right: 2px;
  }

  .sort-btn {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 4px 10px;
    font-size: 0.75rem;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-color);
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    transition: var(--transition-fast);
  }

  .sort-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .sort-btn.active {
    border-color: rgba(245, 158, 11, 0.35);
    color: var(--accent-amber);
    background: rgba(245, 158, 11, 0.08);
  }

  .arrow {
    font-size: 0.65rem;
    font-variant-numeric: tabular-nums;
  }
</style>
