import { invoke } from '@tauri-apps/api/core';

/**
 * Execution target abstraction.
 *
 * Several managers (SSL/certbot, web servers, databases, env vars) can operate
 * either directly on the host ("bare metal") or inside a Docker container.
 * This module wraps a shell command so it runs in the chosen context, and
 * exposes a helper to enumerate running containers for the target picker.
 */

export type ExecTarget =
  | { kind: 'host' }
  | { kind: 'docker'; container: string };

export const HOST_TARGET: ExecTarget = { kind: 'host' };

/** Quote a string for POSIX sh (mirrors the Rust shell_single_quote helper). */
export function shQuote(s: string): string {
  return `'${s.replace(/'/g, `'\\''`)}'`;
}

/**
 * Wrap a command so it executes against the given target.
 * For the host target the command is returned untouched.
 * For a docker target the command is run inside the container via `docker exec`.
 */
export function wrapCmd(target: ExecTarget, cmd: string): string {
  if (target.kind === 'host') return cmd;
  return `docker exec ${shQuote(target.container)} sh -c ${shQuote(cmd)}`;
}

/** Wrap a command that needs a TTY/interactive container exec. */
export function wrapCmdInteractive(target: ExecTarget, cmd: string): string {
  if (target.kind === 'host') return cmd;
  return `docker exec -i ${shQuote(target.container)} sh -c ${shQuote(cmd)}`;
}

/** True when the target is a docker container. */
export function isDocker(target: ExecTarget): boolean {
  return target.kind === 'docker';
}

/** Human label for the current target. */
export function targetLabel(target: ExecTarget): string {
  return target.kind === 'host' ? 'host' : target.container;
}

/**
 * List running docker container names. Tries without sudo first; if docker
 * needs root the caller can retry with useSudo. Returns [] when docker is
 * unavailable rather than throwing, so the picker degrades gracefully.
 */
export async function listContainers(useSudo = false): Promise<string[]> {
  try {
    const out = await invoke<string>('exec_custom_command', {
      cmd: `docker ps --format '{{.Names}}' 2>/dev/null`,
      useSudo,
    });
    return out
      .split('\n')
      .map((l) => l.trim())
      .filter(Boolean);
  } catch {
    return [];
  }
}
