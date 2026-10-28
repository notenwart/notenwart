// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import type { Instrument } from './api/index.js';
import { guessFromFileName, partLabel } from './instruments.js';

const instrument = (
	id: string,
	name: string,
	family: string,
	transposition: string | null,
	aliases: string[]
): Instrument => ({
	id,
	name,
	familyId: family,
	family,
	transposition,
	aliases,
	sort: 0,
	usage: 0
});

const catalogue: Instrument[] = [
	instrument('cl', 'Clarinet', 'Woodwinds', 'Bb', ['Klarinette', 'Cl']),
	instrument('bcl', 'Bass clarinet', 'Woodwinds', 'Bb', ['Bassklarinette']),
	instrument('tpt', 'Trumpet', 'Brass', 'Bb', ['Trompete']),
	instrument('asx', 'Alto saxophone', 'Woodwinds', 'Eb', ['Altsax', 'Alto Sax']),
	instrument('fl', 'Flute', 'Woodwinds', null, ['Flöte', 'Querflöte'])
];

describe('guessFromFileName', () => {
	it.each([
		['Trompete_2.pdf', 'tpt', 2],
		['03 Alto Sax 1.pdf', 'asx', 1],
		['Bass Clarinet.pdf', 'bcl', null],
		['Bassklarinette in B.pdf', 'bcl', null],
		['Querflöte-1.PDF', 'fl', 1],
		['clarinet 3.pdf', 'cl', 3]
	])('%s', (file, id, voice) => {
		const guess = guessFromFileName(file, catalogue);
		expect(guess?.instrument.id).toBe(id);
		expect(guess?.voice).toBe(voice);
	});

	it('needs whole words', () => {
		expect(guessFromFileName('Clockwork.pdf', catalogue)).toBeNull();
	});
});

it('builds labels', () => {
	expect(partLabel(catalogue[2], 2)).toBe('Trumpet 2');
	expect(partLabel(catalogue[2], null)).toBe('Trumpet');
});
