// SPDX-License-Identifier: AGPL-3.0-or-later
import { expect, it } from 'vitest';
import { formatPages, range } from './pages.js';

it('formats page lists as ranges, keeping the order', () => {
	expect(formatPages([1, 2, 3, 5, 7, 8])).toBe('1–3, 5, 7–8');
	expect(formatPages([4, 2])).toBe('4, 2');
	expect(formatPages([])).toBe('');
});

it('builds ranges in both directions', () => {
	expect(range(2, 4)).toEqual([2, 3, 4]);
	expect(range(4, 2)).toEqual([4, 3, 2]);
});
