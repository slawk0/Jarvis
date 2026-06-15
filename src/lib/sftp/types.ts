export interface FileInfo {
  name: string;
  is_dir: boolean;
  size: number;
  permissions: number | null;
  modified: number;
  /** Pełna ścieżka zdalna (np. wyniki wyszukiwania rekurencyjnego) */
  path?: string;
}

export type TransferKind = 'upload' | 'download' | 'move' | 'delete';
export type TransferStatus = 'queued' | 'running' | 'completed' | 'failed' | 'cancelled';

export interface BatchSummary {
  completed: number;
  failed: number;
  total: number;
}

export interface SftpTransferEvent {
  job_id: string;
  file_name: string;
  kind: TransferKind;
  status: TransferStatus;
  bytes_done: number;
  total_bytes: number;
  speed_bps: number;
  error?: string | null;
  remote_path: string;
  local_path?: string | null;
  dest_path?: string | null;
  is_dir: boolean;
  batch: BatchSummary;
}

export interface TransferJob {
  id: string;
  fileName: string;
  kind: TransferKind;
  status: TransferStatus;
  bytesDone: number;
  totalBytes: number;
  speedBps: number;
  error?: string;
  remotePath: string;
  localPath?: string;
  destPath?: string;
  isDir: boolean;
}

export interface MoveItem {
  src: string;
  dest: string;
}

export interface DeleteItem {
  path: string;
  is_dir: boolean;
}
