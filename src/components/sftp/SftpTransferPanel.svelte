<script lang="ts">
  import {
    X,
    ChevronDown,
    ChevronUp,
    Ban,
    RotateCcw,
    Trash2,
    Upload,
    Download,
    FolderInput,
  } from 'lucide-svelte';
  import {
    transferStore,
    cancelTransfer,
    clearCompletedJobs,
    retryFailedJobs,
    kindLabel,
    statusLabel,
  } from '$lib/sftp/transferStore.svelte';
  import { formatBytes, formatSpeed } from '$lib/sftp/pathUtils';
  import type { TransferJob } from '$lib/sftp/types';
  import ListSortBar from '../ui/ListSortBar.svelte';
  import { applySort, nextSort, type SortState } from '$lib/sort/sortUtils';
  import { LL } from '$lib/i18n/i18n-svelte';

  let jobSort = $state<SortState<string>>({ column: 'name', direction: 'asc' });

  const statusOrder: Record<string, number> = {
    running: 0,
    queued: 1,
    completed: 2,
    failed: 3,
    cancelled: 4,
  };

  function getSortedJobs(): TransferJob[] {
    return applySort(transferStore.jobs, jobSort, {
      name: (j) => j.fileName || j.remotePath,
      status: (j) => statusOrder[j.status] ?? 99,
      size: (j) => j.totalBytes,
      kind: (j) => kindLabel(j.kind),
    });
  }

  function progressPercent(job: TransferJob): number {
    if (!job.totalBytes) return job.status === 'completed' ? 100 : 0;
    return Math.min(100, Math.round((job.bytesDone / job.totalBytes) * 100));
  }

  function kindIcon(kind: string) {
    switch (kind) {
      case 'upload':
        return Upload;
      case 'download':
        return Download;
      case 'move':
        return FolderInput;
      default:
        return Trash2;
    }
  }
</script>

{#if transferStore.panelOpen}
  <div class="transfer-panel glass" class:collapsed={transferStore.panelCollapsed}>
    <header class="panel-header">
      <div class="panel-title">
        <h3>{$LL.sftp.transferTitle()}</h3>
        {#if transferStore.isTransferring}
          <span class="batch-meta mono-val">
            {$LL.sftp.transferCompleted({ completed: String(transferStore.batchSummary.completed), total: String(transferStore.batchSummary.total) })}
            {#if transferStore.currentSpeedBps > 0}
              · {formatSpeed(transferStore.currentSpeedBps)}
            {/if}
          </span>
        {:else if transferStore.batchSummary.total > 0}
          <span class="batch-meta mono-val">
            {$LL.sftp.transferCompleted({ completed: String(transferStore.batchSummary.completed), total: String(transferStore.batchSummary.total) })}
            {#if transferStore.batchSummary.failed > 0}
              · {$LL.sftp.transferFailed({ count: String(transferStore.batchSummary.failed) })}
            {/if}
          </span>
        {/if}
      </div>
      <div class="panel-actions">
        {#if transferStore.isTransferring}
          <button class="btn-sm danger" onclick={cancelTransfer} title={$LL.sftp.transferCancel()}>
            <Ban size={14} /> {$LL.sftp.transferCancel()}
          </button>
        {/if}
        <button class="btn-sm secondary" onclick={retryFailedJobs} title={$LL.sftp.transferRetry()}>
          <RotateCcw size={14} />
        </button>
        <button class="btn-sm secondary" onclick={clearCompletedJobs} title={$LL.sftp.transferClear()}>
          <Trash2 size={14} />
        </button>
        <button
          class="btn-sm secondary btn-icon"
          onclick={() => (transferStore.panelCollapsed = !transferStore.panelCollapsed)}
        >
          {#if transferStore.panelCollapsed}
            <ChevronUp size={14} />
          {:else}
            <ChevronDown size={14} />
          {/if}
        </button>
        <button class="btn-sm secondary btn-icon" onclick={() => (transferStore.panelOpen = false)}>
          <X size={14} />
        </button>
      </div>
    </header>

    {#if !transferStore.panelCollapsed}
      {#if transferStore.jobs.length > 0}
        <ListSortBar
          columns={[
            { id: 'name', label: $LL.common.name() },
            { id: 'status', label: $LL.common.status() },
            { id: 'size', label: $LL.common.size() },
            { id: 'kind', label: $LL.common.type() },
          ]}
          activeColumn={jobSort.column}
          direction={jobSort.direction}
          onsort={(c) => jobSort = nextSort(jobSort, c)}
        />
      {/if}
      <div class="job-list">
        {#if transferStore.jobs.length === 0}
          <p class="empty">{$LL.sftp.transferEmpty()}</p>
        {:else}
          {#each getSortedJobs() as job (job.id)}
            {@const Icon = kindIcon(job.kind)}
            <div class="job-item" class:failed={job.status === 'failed'}>
              <div class="job-row">
                <Icon size={14} class="job-icon" />
                <span class="job-name" title={job.remotePath}>{job.fileName || job.remotePath}</span>
                <span class="job-status status-{job.status}">{statusLabel(job.status)}</span>
              </div>
              {#if job.totalBytes > 0 && (job.status === 'running' || job.status === 'completed')}
                <div class="progress-bar">
                  <div class="progress-fill" style="width: {progressPercent(job)}%"></div>
                </div>
                <div class="job-meta mono-val">
                  <span>{formatBytes(job.bytesDone)} / {formatBytes(job.totalBytes)}</span>
                  {#if job.status === 'running' && job.speedBps > 0}
                    <span>{formatSpeed(job.speedBps)}</span>
                  {/if}
                  <span>{kindLabel(job.kind)}</span>
                </div>
              {:else if job.kind === 'move' || job.kind === 'delete'}
                <div class="job-meta mono-val">
                  <span>{kindLabel(job.kind)}</span>
                </div>
              {/if}
              {#if job.error}
                <p class="job-error">{job.error}</p>
              {/if}
            </div>
          {/each}
        {/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  .transfer-panel {
    position: fixed;
    bottom: 24px;
    right: 24px;
    width: 380px;
    max-height: 420px;
    display: flex;
    flex-direction: column;
    border-radius: var(--radius-md);
    border: 1px solid var(--border-color);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.45);
    z-index: 200;
    overflow: hidden;
  }

  .transfer-panel.collapsed {
    max-height: none;
  }

  .panel-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 8px;
    padding: 12px 14px;
    border-bottom: 1px solid var(--border-color);
    flex-shrink: 0;
  }

  .panel-title h3 {
    font-size: 0.9rem;
    color: white;
    margin: 0 0 4px;
  }

  .batch-meta {
    font-size: 0.75rem;
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
  }

  .panel-actions {
    display: flex;
    gap: 4px;
    flex-shrink: 0;
  }

  .btn-sm {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 4px 8px;
    font-size: 0.75rem;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-color);
    background: var(--bg-hover);
    color: var(--text-secondary);
    cursor: pointer;
    transition: var(--transition-fast);
  }

  .btn-sm:hover {
    color: var(--text-primary);
    background: rgba(255, 255, 255, 0.06);
  }

  .btn-sm.danger:hover {
    color: var(--accent-red);
    background: var(--accent-red-glow);
  }

  .btn-sm.btn-icon {
    padding: 4px;
  }

  .job-list {
    overflow-y: auto;
    max-height: 280px;
    padding: 0 8px 8px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .empty {
    text-align: center;
    color: var(--text-muted);
    font-size: 0.85rem;
    padding: 20px;
  }

  .job-item {
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid transparent;
  }

  .job-item.failed {
    border-color: rgba(244, 63, 94, 0.25);
    background: var(--accent-red-glow);
  }

  .job-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
  }

  .job-name {
    flex: 1;
    font-size: 0.82rem;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .job-status {
    font-size: 0.7rem;
    padding: 2px 6px;
    border-radius: 4px;
    flex-shrink: 0;
  }

  .status-running {
    color: var(--accent-amber);
    background: var(--accent-amber-glow);
  }

  .status-completed {
    color: #10b981;
    background: rgba(16, 185, 129, 0.1);
  }

  .status-failed {
    color: var(--accent-red);
    background: var(--accent-red-glow);
  }

  .status-queued {
    color: var(--text-muted);
    background: var(--bg-hover);
  }

  .status-cancelled {
    color: var(--text-secondary);
    background: var(--bg-hover);
  }

  .progress-bar {
    height: 4px;
    background: var(--bg-hover);
    border-radius: 2px;
    overflow: hidden;
    margin-bottom: 4px;
  }

  .progress-fill {
    height: 100%;
    background: linear-gradient(90deg, var(--accent-amber), #f59e0b);
    border-radius: 2px;
    transition: width 0.2s ease;
  }

  .job-meta {
    display: flex;
    justify-content: space-between;
    font-size: 0.72rem;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .job-error {
    font-size: 0.72rem;
    color: #ff8595;
    margin: 4px 0 0;
  }

  :global(.transfer-panel .list-sort-bar) {
    padding: 6px 14px 4px;
    border-bottom: 1px solid var(--border-color);
    flex-shrink: 0;
  }

  :global(.job-icon) {
    color: var(--text-secondary);
    flex-shrink: 0;
  }
</style>
