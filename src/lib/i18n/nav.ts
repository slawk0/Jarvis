import type { Locales } from './i18n-types';
import type { TranslationFunctions } from './i18n-types';

export const TAB_IDS = [
	'dashboard',
	'maintenance',
	'backups',
	'network',
	'runbooks',
	'files',
	'services',
	'docker',
	'cron',
	'users',
	'firewall',
	'crowdsec',
	'pangolin',
	'logs',
	'terminal',
] as const;

export type TabId = (typeof TAB_IDS)[number];

type NavKey = keyof TranslationFunctions['nav'];

export function getNavLabel(LL: TranslationFunctions, tabId: string): string {
	if (TAB_IDS.includes(tabId as TabId)) {
		return LL.nav[tabId as NavKey]();
	}
	return tabId;
}

export function getNavLabels(LL: TranslationFunctions): Record<string, string> {
	return Object.fromEntries(TAB_IDS.map((id) => [id, LL.nav[id]()]));
}
