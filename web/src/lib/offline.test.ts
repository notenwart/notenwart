// SPDX-License-Identifier: AGPL-3.0-or-later
import { expect, it } from 'vitest';
import type { MyEnsemble } from './api/index.js';
import { plan, wantedParts } from './offline.js';

const v1 = '2026-10-22T10:00:00Z';
const v2 = '2026-10-23T10:00:00Z';

it('downloads new and changed parts and removes withdrawn ones', () => {
	const stored = [
		{ id: 'a', updatedAt: v1, ensembleIds: ['e'] },
		{ id: 'b', updatedAt: v1, ensembleIds: ['e'] },
		{ id: 'gone', updatedAt: v1, ensembleIds: ['e'] }
	];
	const wanted = [
		{ id: 'a', updatedAt: v1, ensembleIds: ['e'] },
		{ id: 'b', updatedAt: v2, ensembleIds: ['e'] },
		{ id: 'new', updatedAt: v1, ensembleIds: ['e'] }
	];
	expect(plan(stored, wanted)).toEqual({ remove: ['gone'], download: ['b', 'new'], relabel: [] });
});

it('only relabels a part that moved between offline ensembles', () => {
	expect(
		plan(
			[{ id: 'a', updatedAt: v1, ensembleIds: ['e'] }],
			[{ id: 'a', updatedAt: v1, ensembleIds: ['e', 'f'] }]
		)
	).toEqual({ remove: [], download: [], relabel: ['a'] });
});

it('collects parts of offline ensembles once', () => {
	const part = { id: 'p', label: 'Trumpet 2', pageCount: 2, updatedAt: v1 };
	const work = { id: 'w', title: 'March', composer: null, parts: [part] };
	const music: MyEnsemble[] = [
		{ id: 'e', name: 'Band', leader: false, works: [work], programs: [] },
		{ id: 'f', name: 'Brass', leader: false, works: [work], programs: [] },
		{
			id: 'g',
			name: 'Choir',
			leader: false,
			works: [{ ...work, id: 'w2', parts: [{ ...part, id: 'q' }] }],
			programs: []
		}
	];
	const wanted = wantedParts(music, ['e', 'f']);
	expect([...wanted.keys()]).toEqual(['p']);
	expect(wanted.get('p')?.ensembleIds).toEqual(['e', 'f']);
	expect(wanted.get('p')?.workTitle).toBe('March');
});
