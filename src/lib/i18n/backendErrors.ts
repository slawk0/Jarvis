import { get } from 'svelte/store';
import LL from './i18n-svelte';
import type { TranslationFunctions } from './i18n-types';

export type AppErrorPayload = {
	code: string;
	details?: string;
};

const SUDO_CODES = new Set(['SUDO_PASSWORD_REQUIRED', 'SUDO_PASSWORD_INCORRECT']);
const TRANSFER_CANCELLED = 'TRANSFER_CANCELLED';

export function isSudoPasswordRequired(err: unknown): boolean {
	return extractErrorCode(err) === 'SUDO_PASSWORD_REQUIRED';
}

export function isSudoPasswordIncorrect(err: unknown): boolean {
	return extractErrorCode(err) === 'SUDO_PASSWORD_INCORRECT';
}

export function isTransferCancelled(err: unknown): boolean {
	const code = extractErrorCode(err);
	return code === TRANSFER_CANCELLED || String(err) === 'Anulowano';
}

export function parseAppError(err: unknown): AppErrorPayload | null {
	if (typeof err === 'object' && err !== null && 'code' in err) {
		const payload = err as AppErrorPayload;
		if (typeof payload.code === 'string') return payload;
	}
	const text = String(err);
	try {
		const parsed = JSON.parse(text) as AppErrorPayload;
		if (parsed?.code) return parsed;
	} catch {
		// legacy plain string
	}
	if (SUDO_CODES.has(text)) return { code: text };
	if (text === 'Anulowano') return { code: TRANSFER_CANCELLED };
	return null;
}

function extractErrorCode(err: unknown): string | null {
	return parseAppError(err)?.code ?? null;
}

function translateCode(LL: TranslationFunctions, code: string, details?: string): string | null {
	const errors = LL.errors as Record<string, (arg?: { details?: string } | string) => string>;
	const fn = errors[code];
	if (!fn) return null;
	if (details !== undefined) return fn({ details });
	return fn();
}

export function formatInvokeError(err: unknown, ll?: TranslationFunctions): string {
	const translations = ll ?? get(LL);
	const parsed = parseAppError(err);
	if (parsed?.code) {
		const translated = translateCode(translations, parsed.code, parsed.details);
		if (translated) return translated;
	}
	return String(err);
}

export function formatBackendError(code: string, details?: string, ll?: TranslationFunctions): string {
	const translations = ll ?? get(LL);
	return translateCode(translations, code, details) ?? details ?? code;
}
