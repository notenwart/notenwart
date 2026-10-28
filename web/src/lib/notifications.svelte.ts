// SPDX-License-Identifier: AGPL-3.0-or-later
// The bell in the header (unread count, refreshed every minute while the page is visible) and the texts of
// notifications in the reader's language.

import { api, type Notification, type NotificationKind } from './api/index.js';
import { formatDate } from './dates.js';
import { t } from './i18n/index.svelte.js';

export const notifications = $state({ unread: 0 });

export async function refreshUnread() {
	const { data } = await api.GET('/notifications', { params: { query: { count: true } } });
	if (data) notifications.unread = data.unread;
}

/** Keeps the count current; returns a function that stops it. */
export function watchUnread(): () => void {
	refreshUnread();
	const timer = setInterval(() => {
		if (document.visibilityState === 'visible') refreshUnread();
	}, 60_000);
	const onVisible = () => document.visibilityState === 'visible' && refreshUnread();
	document.addEventListener('visibilitychange', onVisible);
	return () => {
		clearInterval(timer);
		document.removeEventListener('visibilitychange', onVisible);
	};
}

/** Names of the kinds, for the settings. */
export const kindNames: Record<NotificationKind, string> = {
	partsNew: 'New parts for me',
	programChanged: 'Programme changes',
	loanDue: 'Loans due',
	annotationsShared: 'Markings shared by leaders',
	guestLinkExpiring: 'Guest links running out'
};

/** The text of a notification. */
export function describe(n: Pick<Notification, 'kind' | 'params'>): string {
	const p = n.params as Record<string, string | boolean | undefined>;
	const s = (key: string) => String(p[key] ?? '');
	switch (n.kind) {
		case 'partsNew':
			return p.part
				? t('New part: {part} – {work}', { part: s('part'), work: s('work') })
				: t('New parts: {work}', { work: s('work') });
		case 'programChanged':
			return t('Programme “{program}” ({ensemble}) was updated', {
				program: s('program'),
				ensemble: s('ensemble')
			});
		case 'loanDue':
			return p.overdue
				? t('Overdue: please return {item} of “{work}” (was due {date})', {
						item: s('item'),
						work: s('work'),
						date: formatDate(s('date'))
					})
				: t('Please return {item} of “{work}” by {date}', {
						item: s('item'),
						work: s('work'),
						date: formatDate(s('date'))
					});
		case 'annotationsShared':
			return t('New markings by {by} in {part} – {work}', {
				by: s('by'),
				part: s('part'),
				work: s('work')
			});
		case 'guestLinkExpiring':
			return t('The guest link for {guest} expires on {date}', {
				guest: s('guest'),
				date: formatDate(s('date'))
			});
	}
}
