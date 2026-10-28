// SPDX-License-Identifier: AGPL-3.0-or-later
import { expect, it } from 'vitest';
import type { Recording } from './api/index.js';
import { forPart, titleFromFile } from './recordings.js';

const r = (id: string, kind: 'full' | 'practice', partId: string | null, title: string) =>
	({ id, kind, partId, title }) as Recording;

it('puts a part’s own practice recordings first', () => {
	const list = [
		r('a', 'practice', 'alt', 'Alt'),
		r('b', 'full', null, 'Konzert'),
		r('c', 'practice', 'sop', 'Sopran')
	];
	expect(forPart(list, 'sop').map((x) => x.id)).toEqual(['c', 'b', 'a']);
	expect(titleFromFile('Sopran_langsam.m4a')).toBe('Sopran langsam');
});
