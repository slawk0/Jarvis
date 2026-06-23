export interface AlertThresholds {
  enabled: boolean;
  disk_pct: number;
  ram_pct: number;
  cpu_pct: number;
}

export interface Runbook {
  id: string;
  name: string;
  command: string;
  use_sudo: boolean;
}

export type BackupDestination = 'download' | 's3' | 'sftp';

export interface BackupTemplate {
  id: string;
  name: string;
  backup_type: 'files' | 'mysql' | 'postgres';
  source_path: string;
  docker_container?: string | null;
  db_name?: string | null;
  db_user?: string | null;
  db_password?: string | null;
  destination?: BackupDestination | null;
  dest_endpoint?: string | null;
  dest_region?: string | null;
  dest_bucket?: string | null;
  dest_path?: string | null;
  dest_host?: string | null;
  dest_port?: string | null;
  dest_user?: string | null;
  dest_access_key?: string | null;
  dest_secret_key?: string | null;
}

export interface ProfileExtras {
  runbooks: Runbook[];
  backup_templates: BackupTemplate[];
  restic_repos: ResticRepo[];
  alert_thresholds: AlertThresholds;
}

export interface ResticRepo {
  id: string;
  name: string;
  repo_type: 'local' | 's3' | 'sftp' | 'b2' | 'rest' | 'rclone';
  path_or_url: string;
  s3_endpoint?: string | null;
  s3_region?: string | null;
  s3_bucket?: string | null;
  env_vars?: Record<string, string> | null;
  use_sudo: boolean;
  password?: string | null;
  access_key?: string | null;
  secret_key?: string | null;
}

export interface DiskMount {
  mount: string;
  used_mb: number;
  total_mb: number;
  use_pct: number;
  inode_use_pct: number;
}

export interface ProcessInfo {
  pid: string;
  user: string;
  cpu: number;
  mem: number;
  command: string;
}

export interface ExtendedServerStats {
  load_1: number;
  load_5: number;
  load_15: number;
  swap_used_mb: number;
  swap_total_mb: number;
  disk_mounts: DiskMount[];
  top_processes: ProcessInfo[];
}

export interface ServerProfile {
  id: string;
  label: string;
  host: string;
  port: number;
  username: string;
  auth_type: string;
  key_path?: string | null;
}

export const DEFAULT_ALERT_THRESHOLDS: AlertThresholds = {
  enabled: true,
  disk_pct: 85,
  ram_pct: 90,
  cpu_pct: 95,
};

export const DEFAULT_PROFILE_EXTRAS: ProfileExtras = {
  runbooks: [],
  backup_templates: [],
  restic_repos: [],
  alert_thresholds: { ...DEFAULT_ALERT_THRESHOLDS },
};
