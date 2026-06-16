import { getIntlLocale } from '$lib/i18n/formatLocale';

export function joinRemotePath(base: string, name: string): string {
  if (!base || base === '/') {
    return `/${name}`.replace(/\/+/g, '/');
  }
  const sep = base.endsWith('/') ? '' : '/';
  return `${base}${sep}${name}`.replace(/\/+/g, '/');
}

export function getRemotePath(currentPath: string, fileName: string): string {
  return joinRemotePath(currentPath, fileName);
}

export function fileRemotePath(file: { name: string; path?: string }, currentDir: string): string {
  return file.path ?? getRemotePath(currentDir, file.name);
}

export function parentPath(path: string): string {
  const normalized = path.replace(/\/+$/, '') || '/';
  const idx = normalized.lastIndexOf('/');
  if (idx <= 0) return '/';
  return normalized.slice(0, idx) || '/';
}

export function isHiddenEntry(name: string): boolean {
  return name.startsWith('.');
}

export function isSubPath(parent: string, child: string): boolean {
  const normParent = parent.replace(/\/+$/, '') || '/';
  const normChild = child.replace(/\/+$/, '') || '/';
  if (normParent === normChild) return true;
  return normChild.startsWith(normParent + '/');
}

export function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

export function formatSpeed(bps: number): string {
  return `${formatBytes(bps)}/s`;
}

export function formatModified(timestamp: number): string {
  if (!timestamp) return '--';
  const date = new Date(timestamp * 1000);
  return date.toLocaleString(getIntlLocale(), {
    day: '2-digit',
    month: '2-digit',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

export function formatPermissions(perm: number | null): string {
  if (perm === null) return '---';
  return (perm & 0o777).toString(8);
}

/** Flatten local file paths from Tauri drag-drop into upload pairs */
export function flattenLocalPaths(
  paths: string[],
  remoteDir: string,
): { localPath: string; remotePath: string }[] {
  const result: { localPath: string; remotePath: string }[] = [];

  for (const p of paths) {
    collectLocal(p, p, remoteDir, result);
  }
  return result;
}

function collectLocal(
  root: string,
  current: string,
  remoteDir: string,
  out: { localPath: string; remotePath: string }[],
) {
  // Browser/Tauri doesn't give us sync directory listing in frontend easily;
  // for drag-drop Tauri provides full file paths (files only in drop list).
  // Directories are handled by Rust collect_local_files when passing root paths.
  const name = current.split(/[/\\]/).pop() || current;
  const remote = joinRemotePath(remoteDir, name);
  out.push({ localPath: current, remotePath: remote });
}

export function encodeUploadPairs(pairs: { localPath: string; remotePath: string }[]): string[] {
  return pairs.map((p) => `${p.localPath}::${p.remotePath}`);
}
