// SPDX-License-Identifier: AGPL-3.0-or-later
import { expect, it } from 'vitest';
import { describe } from './notifications.svelte.js';

it('words every kind of notification', () => {
	expect(describe({ kind: 'partsNew', params: { part: 'Trompete 2', work: 'Polka' } })).toBe(
		'New part: Trompete 2 – Polka'
	);
	expect(describe({ kind: 'partsNew', params: { work: 'Polka' } })).toBe('New parts: Polka');
	expect(
		describe({
			kind: 'loanDue',
			params: { item: 'Partitur', work: 'Polka', date: '2027-01-10', overdue: true }
		})
	).toContain('Overdue');
});
