import { get } from 'svelte/store';
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification';
import type { AlertThresholds } from '$lib/admin/types';

let permissionChecked = false;

async function ensurePermission(): Promise<boolean> {
  if (permissionChecked) return true;
  let granted = await isPermissionGranted();
  if (!granted) {
    const result = await requestPermission();
    granted = result === 'granted';
  }
  permissionChecked = granted;
  return granted;
}

const lastAlertAt: Record<string, number> = {};
const ALERT_COOLDOWN_MS = 5 * 60 * 1000;

async function notifyOnce(key: string, title: string, body: string) {
  const now = Date.now();
  if (lastAlertAt[key] && now - lastAlertAt[key] < ALERT_COOLDOWN_MS) return;
  lastAlertAt[key] = now;

  if (!(await ensurePermission())) return;
  sendNotification({ title, body });
}

export async function checkResourceAlerts(
  profileLabel: string,
  stats: {
    cpu_usage: number;
    ram_used: number;
    ram_total: number;
    disk_used: number;
    disk_total: number;
  },
  thresholds: AlertThresholds,
) {
  if (!thresholds.enabled) return;

  const ramPct = stats.ram_total > 0 ? (stats.ram_used / stats.ram_total) * 100 : 0;
  const diskPct = stats.disk_total > 0 ? (stats.disk_used / stats.disk_total) * 100 : 0;

  if (stats.cpu_usage >= thresholds.cpu_pct) {
    await notifyOnce(
      `${profileLabel}-cpu`,
      `CPU Alert: ${profileLabel}`,
      `CPU usage is at ${Math.round(stats.cpu_usage)}%, exceeding threshold of ${thresholds.cpu_pct}%`,
    );
  }

  if (ramPct >= thresholds.ram_pct) {
    await notifyOnce(
      `${profileLabel}-ram`,
      `RAM Alert: ${profileLabel}`,
      `RAM usage is at ${Math.round(ramPct)}%, exceeding threshold of ${thresholds.ram_pct}%`,
    );
  }

  if (diskPct >= thresholds.disk_pct) {
    await notifyOnce(
      `${profileLabel}-disk`,
      `Disk Alert: ${profileLabel}`,
      `Disk usage is at ${Math.round(diskPct)}%, exceeding threshold of ${thresholds.disk_pct}%`,
    );
  }
}

export function resetAlertCooldowns() {
  for (const key of Object.keys(lastAlertAt)) {
    delete lastAlertAt[key];
  }
}
