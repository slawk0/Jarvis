// Shared restic environment-variable helpers, used by both the Restic tab
// (inline command prefix) and the Backup tab's scheduled scripts (env file).
import type { ResticRepo } from '$lib/admin/types';

/** POSIX single-quote a string so it is safe to embed in a shell command. */
export function shellQuote(s: string): string {
  return "'" + s.replace(/'/g, "'\\''") + "'";
}

/**
 * Returns the raw (unquoted) env-var pairs restic/rclone need for a repo:
 * the repository password plus S3/B2 credentials and any custom vars.
 */
export function resticEnvPairs(repo: ResticRepo): [string, string][] {
  const envs: [string, string][] = [];
  envs.push(['RESTIC_PASSWORD', repo.password || '']);

  if (repo.repo_type === 's3' || repo.repo_type === 'b2') {
    if (repo.access_key) {
      envs.push([repo.repo_type === 's3' ? 'AWS_ACCESS_KEY_ID' : 'B2_ACCOUNT_ID', repo.access_key]);
    }
    if (repo.secret_key) {
      envs.push([repo.repo_type === 's3' ? 'AWS_SECRET_ACCESS_KEY' : 'B2_ACCOUNT_KEY', repo.secret_key]);
    }
    if (repo.s3_region && repo.repo_type === 's3') {
      envs.push(['AWS_DEFAULT_REGION', repo.s3_region]);
    }
  }

  if (repo.env_vars) {
    for (const [key, val] of Object.entries(repo.env_vars)) {
      envs.push([key, val]);
    }
  }
  return envs;
}

/** Inline command prefix: `K='v' K='v' ` for use directly on a command line. */
export function resticEnvPrefix(repo: ResticRepo): string {
  return resticEnvPairs(repo)
    .map(([k, v]) => `${k}=${shellQuote(v)}`)
    .join(' ') + ' ';
}

/** `export K='v'` lines for sourcing from a script's secrets env file. */
export function resticEnvExports(repo: ResticRepo): string {
  return resticEnvPairs(repo)
    .map(([k, v]) => `export ${k}=${shellQuote(v)}`)
    .join('\n');
}
