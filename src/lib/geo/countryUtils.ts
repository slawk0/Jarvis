import { get } from 'svelte/store';
import LL from '$lib/i18n/i18n-svelte';
import { getIntlLocale } from '$lib/i18n/formatLocale';

export interface CountryTraffic {
  code: string;
  count: number;
  allowedCount?: number;
  blockedCount?: number;
  name?: string;
}

export function formatCompact(n: number): string {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1).replace(/\.0$/, '') + 'M';
  if (n >= 1_000) return (n / 1_000).toFixed(1).replace(/\.0$/, '') + 'k';
  return String(Math.round(n));
}

export function countryCodeToName(code: string): string {
  if (!code) return get(LL).common.unknown();
  try {
    const dn = new Intl.DisplayNames([getIntlLocale()], { type: 'region' });
    return dn.of(code.toUpperCase()) || code;
  } catch {
    return code;
  }
}

export function getCountryName(geo: CountryTraffic): string {
  return geo.name || countryCodeToName(geo.code);
}

export function trafficIntensity(count: number, max: number): number {
  if (max <= 0 || count <= 0) return 0;
  return Math.min(count / max, 1);
}

export function trafficFill(intensity: number): string {
  if (intensity <= 0) return 'rgba(255, 255, 255, 0.04)';
  const r = Math.round(180 + intensity * 75);
  const g = Math.round(70 + intensity * 40);
  const b = Math.round(20 + intensity * 10);
  const a = 0.35 + intensity * 0.65;
  return `rgba(${r}, ${g}, ${b}, ${a})`;
}

export function trafficBarGradient(intensity: number): string {
  if (intensity <= 0) return 'rgba(255,255,255,0.08)';
  const pct = Math.round(intensity * 100);
  return `linear-gradient(90deg, rgba(234, 88, 12, ${0.25 + intensity * 0.55}) ${pct}%, rgba(255,255,255,0.04) ${pct}%)`;
}
