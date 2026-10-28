// SPDX-License-Identifier: AGPL-3.0-or-later
import { expect, it } from 'vitest';
import { overdue, today } from './loans.js';

it('knows which loans are overdue', () => {
	expect(today(new Date(2027, 0, 5))).toBe('2027-01-05');
	expect(overdue({ dueOn: '2027-01-04', returnedOn: null }, '2027-01-05')).toBe(true);
	expect(overdue({ dueOn: '2027-01-05', returnedOn: null }, '2027-01-05')).toBe(false);
	expect(overdue({ dueOn: '2027-01-04', returnedOn: '2027-01-04' }, '2027-01-05')).toBe(false);
	expect(overdue({ dueOn: null, returnedOn: null }, '2027-01-05')).toBe(false);
});
