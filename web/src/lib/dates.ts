// SPDX-License-Identifier: AGPL-3.0-or-later
import { i18n } from './i18n/index.svelte.js';

/** "25 April 2027" / "25. April 2027" for an ISO date (YYYY-MM-DD), read as a calendar day without time zone. */
export function formatDate(iso: string | null | undefined): string {
	if (!iso) return '';
	const [year, month, day] = iso.split('-').map(Number);
	const date = new Date(Date.UTC(year, month - 1, day));
	return new Intl.DateTimeFormat(i18n.locale === 'de' ? 'de-DE' : 'en-GB', {
		day: 'numeric',
		month: 'long',
		year: 'numeric',
		timeZone: 'UTC'
	}).format(date);
}
