// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import { filterOptions, fold, matches, suggest } from './search.js';

it('ignores case and accents', () => {
	expect(fold('Flöte Größe')).toBe('flote große');
	expect(matches('Querflöte', 'flote')).toBe(true);
});

it('needs every word', () => {
	expect(matches('Böhmischer Traum – Trumpet 2', 'traum 2')).toBe(true);
	expect(matches('Böhmischer Traum – Trumpet 2', 'traum 3')).toBe(false);
});

it('filters options by label, group, hint and keywords', () => {
	const options = [
		{ value: 'a', label: 'Trumpet 1', group: 'March' },
		{ value: 'b', label: 'Trumpet 2', group: 'Waltz' },
		{ value: 'c', label: 'Alto saxophone', keywords: 'Altsax' }
	];
	expect(filterOptions(options, 'waltz').map((o) => o.value)).toEqual(['b']);
	expect(filterOptions(options, 'altsax').map((o) => o.value)).toEqual(['c']);
	expect(filterOptions(options, '  ')).toHaveLength(3);
});

describe('suggest', () => {
	const composers = ['Ernst Mosch', 'Carl Teike', 'Ernő Dohnányi', 'Mosch-Ensemble'];
	it('puts values starting with the input first', () => {
		expect(suggest(composers, 'mos')).toEqual(['Mosch-Ensemble', 'Ernst Mosch']);
	});
	it('ignores accents and leaves out what was typed exactly or chosen already', () => {
		expect(suggest(composers, 'erno')).toEqual(['Ernő Dohnányi']);
		expect(suggest(composers, 'carl teike')).toEqual([]);
		expect(suggest(['Marsch', 'Polka'], '', ['marsch'])).toEqual(['Polka']);
	});
	it('stops at the limit', () => {
		expect(suggest(composers, '', [], 2)).toEqual(['Ernst Mosch', 'Carl Teike']);
	});
});
