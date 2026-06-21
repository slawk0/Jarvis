import { get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  BatchSummary,
  DeleteItem,
  MoveItem,
  SftpTransferEvent,
  TransferJob,
  TransferKind,
  TransferStatus,
} from './types';
import { encodeUploadPairs } from './pathUtils';

export const transferStore = $state({
  jobs: [] as TransferJob[],
  batchSummary: { completed: 0, failed: 0, total: 0 } as BatchSummary,
  panelOpen: false,
  panelCollapsed: false,
  isTransferring: false,
  currentSpeedBps: 0,
});

let unlisten: UnlistenFn | null = null;
let onBatchComplete: (() => void) | null = null;

export function setOnBatchComplete(cb: (() => void) | null) {
  onBatchComplete = cb;
}

export async function initTransferStore() {
  if (unlisten) return;
  unlisten = await listen<SftpTransferEvent>('sftp-transfer-event', (event) => {
    handleTransferEvent(event.payload);
  });
}

export function destroyTransferStore() {
  unlisten?.();
  unlisten = null;
}

function handleTransferEvent(ev: SftpTransferEvent) {
  transferStore.batchSummary = {
    completed: ev.batch.completed,
    failed: ev.batch.failed,
    total: ev.batch.total,
  };

  if (ev.job_id === 'batch-complete') {
    transferStore.isTransferring = false;
    transferStore.currentSpeedBps = 0;
    onBatchComplete?.();
    return;
  }

  transferStore.isTransferring = true;
  transferStore.panelOpen = true;
  if (ev.status === 'running' && ev.speed_bps > 0) {
    transferStore.currentSpeedBps = ev.speed_bps;
  }

  const existing = transferStore.jobs.findIndex((j) => j.id === ev.job_id);
  const job: TransferJob = {
    id: ev.job_id,
    fileName: ev.file_name,
    kind: ev.kind,
    status: ev.status,
    bytesDone: ev.bytes_done,
    totalBytes: ev.total_bytes,
    speedBps: ev.speed_bps,
    error: ev.error ?? undefined,
    remotePath: ev.remote_path,
    localPath: ev.local_path ?? undefined,
    destPath: ev.dest_path ?? undefined,
    isDir: ev.is_dir,
  };

  if (existing >= 0) {
    transferStore.jobs[existing] = job;
  } else {
    transferStore.jobs.push(job);
  }
}

export function clearCompletedJobs() {
  transferStore.jobs = transferStore.jobs.filter(
    (j) => j.status !== 'completed' && j.status !== 'cancelled',
  );
}

export function getFailedJobs(): TransferJob[] {
  return transferStore.jobs.filter((j) => j.status === 'failed');
}

export async function retryFailedJobs() {
  const failed = getFailedJobs();
  if (failed.length === 0) return;

  const uploads = failed.filter((j) => j.kind === 'upload' && j.localPath);
  const downloads = failed.filter((j) => j.kind === 'download' && j.remotePath);
  const moves = failed.filter((j) => j.kind === 'move' && j.destPath);
  const deletes = failed.filter((j) => j.kind === 'delete');

  if (uploads.length > 0) {
    await startUploadBatch(
      '',
      [],
      uploads.map((j) => ({
        localPath: j.localPath!,
        remotePath: j.remotePath,
      })),
    );
  }
  if (downloads.length > 0) {
    await startDownloadBatch(downloads.map((j) => j.remotePath));
  }
  if (moves.length > 0) {
    await startMoveBatch(moves.map((j) => ({ src: j.remotePath, dest: j.destPath! })));
  }
  if (deletes.length > 0) {
    await startDeleteBatch(deletes.map((j) => ({ path: j.remotePath, is_dir: j.isDir })));
  }
}

export async function startUploadBatch(
  remoteDir: string,
  localPaths: string[],
  pairs?: { localPath: string; remotePath: string }[],
) {
  transferStore.panelOpen = true;
  transferStore.isTransferring = true;
  const paths = pairs ? encodeUploadPairs(pairs) : localPaths;
  const count = await invoke<number>('sftp_start_upload_batch', {
    remoteDir,
    localPaths: paths,
  });
  transferStore.batchSummary.total = count;
}

export async function startDownloadBatch(remotePaths: string[]) {
  transferStore.panelOpen = true;
  transferStore.isTransferring = true;
  const count = await invoke<number>('sftp_start_download_batch', {
    remotePaths,
    localDir: null,
  });
  transferStore.batchSummary.total = count;
}

export async function startMoveBatch(moves: MoveItem[]) {
  transferStore.panelOpen = true;
  transferStore.isTransferring = true;
  const count = await invoke<number>('sftp_start_move_batch', { moves });
  transferStore.batchSummary.total = count;
}

export async function startDeleteBatch(paths: DeleteItem[]) {
  transferStore.panelOpen = true;
  transferStore.isTransferring = true;
  const count = await invoke<number>('sftp_start_delete_batch', { paths });
  transferStore.batchSummary.total = count;
}

export async function cancelTransfer() {
  await invoke('sftp_cancel_transfer');
}

export function kindLabel(kind: TransferKind): string {
  switch (kind) {
    case 'upload':
      return "Uploading";
    case 'download':
      return "Downloading";
    case 'move':
      return "Moving";
    case 'delete':
      return "Deleting";
  }
}

export function statusLabel(status: TransferStatus): string {
  switch (status) {
    case 'queued':
      return "Queued";
    case 'running':
      return "In progress";
    case 'completed':
      return "Completed";
    case 'failed':
      return "Failed";
    case 'cancelled':
      return "Cancelled";
  }
}
