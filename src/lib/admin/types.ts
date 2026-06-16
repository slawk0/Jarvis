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

export interface BackupTemplate {
  id: string;
  name: string;
  backup_type: 'files' | 'mysql' | 'postgres';
  source_path: string;
  docker_container?: string | null;
  db_name?: string | null;
  db_user?: string | null;
  db_password?: string | null;
}

export interface ProfileExtras {
  runbooks: Runbook[];
  backup_templates: BackupTemplate[];
  alert_thresholds: AlertThresholds;
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
  alert_thresholds: { ...DEFAULT_ALERT_THRESHOLDS },
};
