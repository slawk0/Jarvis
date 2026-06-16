import { get } from 'svelte/store';
import { locale } from './i18n-svelte';
import type { Locales } from './i18n-types';

const INTL_LOCALE: Record<Locales, string> = {
	en: 'en-US',
	pl: 'pl-PL',
};

export function getLocale(): Locales {
	return get(locale);
}

export function getIntlLocale(): string {
	return INTL_LOCALE[getLocale()];
}

export function formatDate(value: Date | number | string, options?: Intl.DateTimeFormatOptions): string {
	const date = value instanceof Date ? value : new Date(value);
	return date.toLocaleString(getIntlLocale(), options);
}

export function formatCountry(code: string, unknownLabel: string): string {
	if (!code || code === 'XX') return unknownLabel;
	try {
		return new Intl.DisplayNames([getIntlLocale()], { type: 'region' }).of(code) ?? code;
	} catch {
		return code;
	}
}
