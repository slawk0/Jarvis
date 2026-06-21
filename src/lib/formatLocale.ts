export function formatDate(value: Date | number | string, options?: Intl.DateTimeFormatOptions): string {
	const date = value instanceof Date ? value : new Date(value);
	return date.toLocaleString('en-US', options);
}

export function formatCountry(code: string, unknownLabel: string): string {
	if (!code || code === 'XX') return unknownLabel;
	try {
		return new Intl.DisplayNames(['en-US'], { type: 'region' }).of(code) ?? code;
	} catch {
		return code;
	}
}

export function getIntlLocale(): string {
	return 'en-US';
}

