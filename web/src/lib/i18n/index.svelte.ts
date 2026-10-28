// SPDX-License-Identifier: AGPL-3.0-or-later
// UI translations. Source strings are English and double as keys: t('Sign in'). Placeholders: t('Hello {name}', {name}).
// Missing translations fall back to English. The locale follows the browser until the user picks one.
import { de } from './de.js';

export type Locale = 'en' | 'de';
export const locales: Record<Locale, string> = { de: 'Deutsch', en: 'English' };

const KEY = 'notenwart.locale';
const dictionaries: Record<Locale, Record<string, string>> = { en: {}, de };

function initial(): Locale {
	try {
		const stored = localStorage.getItem(KEY);
		if (stored === 'de' || stored === 'en') return stored;
	} catch {
		// Private mode: follow the browser.
	}
	if (typeof navigator === 'undefined') return 'en';
	return navigator.languages?.some((l) => l.toLowerCase().startsWith('de')) ? 'de' : 'en';
}

export const i18n = $state<{ locale: Locale }>({ locale: initial() });

export function setLocale(locale: Locale) {
	i18n.locale = locale;
	try {
		localStorage.setItem(KEY, locale);
	} catch {
		// Not remembered, still applied.
	}
	if (typeof document !== 'undefined') document.documentElement.lang = locale;
}

export function t(text: string, params?: Record<string, string | number>): string {
	const template = dictionaries[i18n.locale][text] ?? text;
	if (!params) return template;
	return template.replace(/\{(\w+)\}/g, (match, key) =>
		key in params ? String(params[key]) : match
	);
}

/** `one` for exactly 1, `other` otherwise; both may use {n}. */
export function plural(
	n: number,
	one: string,
	other: string,
	params?: Record<string, string | number>
) {
	return t(n === 1 ? one : other, { n, ...params });
}

/** Instrument names come from the server's built-in catalogue in English. */
export function instrumentName(name: string): string {
	return t(name);
}
