<script lang="ts">
  import type { SortDir } from '$lib/sort/sortUtils';

  interface Props {
    label: string;
    column: string;
    activeColumn: string;
    direction: SortDir;
    onsort: (column: string) => void;
    width?: string;
    align?: 'left' | 'right';
  }

  let {
    label,
    column,
    activeColumn,
    direction,
    onsort,
    width,
    align = 'left',
  }: Props = $props();

  const isActive = $derived(activeColumn === column);
</script>

<th style={width ? `width: ${width}` : undefined} class:align-right={align === 'right'}>
  <button type="button" class="sort-th" class:active={isActive} onclick={() => onsort(column)}>
    <span>{label}</span>
    <span class="sort-icon" aria-hidden="true">
      {#if isActive}
        {direction === 'asc' ? '↑' : '↓'}
      {:else}
        ↕
      {/if}
    </span>
  </button>
</th>

<style>
  th {
    padding: 0;
  }

  th.align-right .sort-th {
    justify-content: flex-end;
  }

  .sort-th {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    width: 100%;
    padding: 14px 16px;
    margin: 0;
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 0.8rem;
    text-transform: uppercase;
    font-weight: 600;
    letter-spacing: 0.05em;
    cursor: pointer;
    user-select: none;
    transition: color 0.15s ease;
  }

  .sort-th:hover {
    color: var(--text-primary);
  }

  .sort-th.active {
    color: var(--accent-amber);
  }

  .sort-icon {
    font-size: 0.7rem;
    opacity: 0.45;
    font-variant-numeric: tabular-nums;
  }

  .sort-th.active .sort-icon {
    opacity: 1;
  }
</style>
