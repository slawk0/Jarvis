import { loadLocaleAsync } from './i18n-util.async';
import { setLocale as setI18nLocale } from './i18n-svelte';
import type { Locales } from './i18n-types';

const STORAGE_KEY = 'jarvis-locale';

function readStoredLocale(): Locales {
	if (typeof localStorage === 'undefined') return 'en';
	const stored = localStorage.getItem(STORAGE_KEY);
	return stored === 'pl' ? 'pl' : 'en';
}

let currentLocale = $state<Locales>(readStoredLocale());
let initialized = $state(false);

export function getCurrentLocale(): Locales {
	return currentLocale;
}

export async function initLocale(): Promise<void> {
	const locale = readStoredLocale();
	await loadLocaleAsync(locale);
	setI18nLocale(locale);
	currentLocale = locale;
	if (typeof document !== 'undefined') {
		document.documentElement.lang = locale;
	}
	initialized = true;
}

export async function setAppLocale(locale: Locales): Promise<void> {
	await loadLocaleAsync(locale);
	setI18nLocale(locale);
	currentLocale = locale;
	if (typeof localStorage !== 'undefined') {
		localStorage.setItem(STORAGE_KEY, locale);
	}
	if (typeof document !== 'undefined') {
		document.documentElement.lang = locale;
	}
}

export function isLocaleInitialized(): boolean {
	return initialized;
}
