<script lang="ts">
  import { Download, Trash2, FolderInput, X } from 'lucide-svelte';

  interface Props {
    selectedCount: number;
    moveMode?: boolean;
    inline?: boolean;
    onDownload: () => void;
    onDelete: () => void;
    onMove: () => void;
    onCancelMove?: () => void;
    onClearSelection: () => void;
  }

  let {
    selectedCount,
    moveMode = false,
    inline = false,
    onDownload,
    onDelete,
    onMove,
    onCancelMove,
    onClearSelection,
  }: Props = $props();

  const hasSelection = $derived(selectedCount > 0);
</script>

{#if inline}
  <div class="selection-actions" class:move-active={moveMode} role="toolbar" aria-label="Akcje na zaznaczonych">
    {#if moveMode}
      <span class="move-hint">
        <FolderInput size={14} />
        <span class="move-hint-text">Kliknij folder docelowy</span>
        <span class="bulk-count tabular-nums">{selectedCount}</span>
      </span>
      <button class="secondary btn-compact bulk-btn" type="button" onclick={onCancelMove}>
        <X size={14} /> Anuluj
      </button>
    {:else}
      <button
        class="secondary btn-compact bulk-btn"
        type="button"
        disabled={!hasSelection}
        onclick={onDownload}
        title={hasSelection ? 'Pobierz zaznaczone' : 'Zaznacz pliki, aby pobrać'}
      >
        <Download size={14} /> Pobierz
      </button>
      <button
        class="secondary btn-compact bulk-btn"
        type="button"
        disabled={!hasSelection}
        onclick={onMove}
        title={hasSelection ? 'Przenieś zaznaczone' : 'Zaznacz pliki, aby przenieść'}
      >
        <FolderInput size={14} /> Przenieś
      </button>
      <button
        class="secondary btn-compact bulk-btn danger"
        type="button"
        disabled={!hasSelection}
        onclick={onDelete}
        title={hasSelection ? 'Usuń zaznaczone' : 'Zaznacz pliki, aby usunąć'}
      >
        <Trash2 size={14} /> Usuń
      </button>
      {#if hasSelection}
        <button
          class="secondary btn-compact bulk-btn btn-icon-only"
          type="button"
          onclick={onClearSelection}
          title="Odznacz wszystko ({selectedCount})"
        >
          <X size={14} />
        </button>
      {/if}
    {/if}
  </div>
{:else}
  <div class="bulk-toolbar glass" role="toolbar" aria-label="Akcje grupowe">
    {#if moveMode}
      <span class="bulk-hint">
        <FolderInput size={16} />
        Kliknij folder docelowy, aby przenieść
        <span class="bulk-count tabular-nums">{selectedCount}</span>
        {selectedCount === 1 ? 'element' : 'elementów'}
      </span>
      <button class="secondary btn-compact bulk-btn" type="button" onclick={onCancelMove}>
        <X size={14} /> Anuluj
      </button>
    {:else}
      <span class="toolbar-label">Zaznaczone:</span>
      <span class="bulk-count tabular-nums" class:inactive={!hasSelection}>
        {hasSelection ? selectedCount : '—'}
      </span>
      <div class="bulk-actions">
        <button
          class="secondary btn-compact bulk-btn"
          type="button"
          disabled={!hasSelection}
          onclick={onDownload}
        >
          <Download size={14} /> Pobierz
        </button>
        <button
          class="secondary btn-compact bulk-btn"
          type="button"
          disabled={!hasSelection}
          onclick={onMove}
        >
          <FolderInput size={14} /> Przenieś
        </button>
        <button
          class="secondary btn-compact bulk-btn danger"
          type="button"
          disabled={!hasSelection}
          onclick={onDelete}
        >
          <Trash2 size={14} /> Usuń
        </button>
        {#if hasSelection}
          <button
            class="secondary btn-compact bulk-btn btn-icon-only"
            type="button"
            onclick={onClearSelection}
            title="Odznacz wszystko"
          >
            <X size={14} />
          </button>
        {/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  .selection-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
    padding: 3px 6px;
    border-radius: var(--radius-sm);
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid var(--border-color);
  }

  .selection-actions.move-active {
    border-color: rgba(245, 158, 11, 0.35);
    background: rgba(245, 158, 11, 0.06);
  }

  .move-hint {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 0.78rem;
    color: var(--accent-amber);
    white-space: nowrap;
  }

  .move-hint-text {
    max-width: 140px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .bulk-toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    padding: 8px 12px;
    border-radius: var(--radius-md);
    flex-shrink: 0;
    border: 1px solid var(--border-color);
  }

  .toolbar-label {
    font-size: 0.78rem;
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .bulk-count {
    font-size: 0.82rem;
    color: var(--accent-amber);
    font-weight: 600;
    min-width: 1.25rem;
    font-variant-numeric: tabular-nums;
  }

  .bulk-count.inactive {
    color: var(--text-muted);
    font-weight: 500;
  }

  .bulk-hint {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    font-size: 0.85rem;
    color: var(--accent-amber);
  }

  .bulk-actions {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    margin-left: auto;
  }

  .bulk-btn {
    transition:
      opacity 0.15s ease,
      transform 0.12s cubic-bezier(0.2, 0, 0, 1),
      color 0.15s ease,
      background 0.15s ease,
      border-color 0.15s ease;
  }

  .bulk-btn:not(:disabled):active {
    transform: scale(0.96);
  }

  .bulk-btn:disabled {
    opacity: 0.42;
    cursor: not-allowed;
    color: var(--text-muted);
    border-color: var(--border-color);
    background: transparent;
  }

  .bulk-btn.danger:not(:disabled):hover {
    color: var(--accent-red);
    background: var(--accent-red-glow);
  }

  .btn-icon-only {
    padding: 5px;
    min-width: 32px;
    justify-content: center;
  }
</style>
