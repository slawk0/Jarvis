import { invoke } from '@tauri-apps/api/core';
import { shQuote } from '$lib/exec/target';

export type DetectedDb = {
  engine: 'mysql' | 'postgres';
  user: string;
  password: string;
  dbName: string | null;
};

function parseContainerEnv(envList: string[] | undefined): Record<string, string> {
  const env: Record<string, string> = {};
  for (const e of envList || []) {
    const i = e.indexOf('=');
    if (i > 0) env[e.slice(0, i)] = e.slice(i + 1);
  }
  return env;
}

function detectFromConfig(cfg: { Env?: string[]; Image?: string }): DetectedDb | null {
  const env = parseContainerEnv(cfg.Env);
  const image = String(cfg.Image || '').toLowerCase();

  const isPg =
    image.includes('postgres') ||
    env.POSTGRES_USER !== undefined ||
    env.POSTGRES_PASSWORD !== undefined ||
    env.POSTGRES_DB !== undefined;
  const isMy =
    image.includes('mysql') ||
    image.includes('mariadb') ||
    env.MYSQL_ROOT_PASSWORD !== undefined ||
    env.MARIADB_ROOT_PASSWORD !== undefined ||
    env.MYSQL_USER !== undefined ||
    env.MARIADB_USER !== undefined;

  if (!isPg && !isMy) return null;

  if (isPg) {
    return {
      engine: 'postgres',
      user: env.POSTGRES_USER || 'postgres',
      password: env.POSTGRES_PASSWORD || '',
      dbName: env.POSTGRES_DB || null,
    };
  }

  const rootPw = env.MYSQL_ROOT_PASSWORD || env.MARIADB_ROOT_PASSWORD;
  if (rootPw !== undefined) {
    return {
      engine: 'mysql',
      user: 'root',
      password: rootPw,
      dbName: env.MYSQL_DATABASE || env.MARIADB_DATABASE || null,
    };
  }

  return {
    engine: 'mysql',
    user: env.MYSQL_USER || env.MARIADB_USER || 'root',
    password: env.MYSQL_PASSWORD || env.MARIADB_PASSWORD || '',
    dbName: env.MYSQL_DATABASE || env.MARIADB_DATABASE || null,
  };
}

/**
 * Inspect a Docker container and infer database engine + credentials from its
 * image name and environment variables (standard Postgres/MySQL/MariaDB images).
 */
export async function detectDbFromContainer(container: string): Promise<DetectedDb | null> {
  const out = await invoke<string>('exec_custom_command', {
    cmd: `docker inspect ${shQuote(container)} --format '{{json .Config}}' 2>/dev/null`,
    useSudo: false,
  });
  const cfg = JSON.parse(out);
  return detectFromConfig(cfg);
}
