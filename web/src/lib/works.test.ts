// SPDX-License-Identifier: AGPL-3.0-or-later
import { expect, it } from 'vitest';
import type { Work } from './api/index.js';
import { workInput } from './works.js';

it('keeps every field of a work when saving a change', () => {
	const work = {
		id: 'w',
		title: 'Stille Nacht',
		composer: 'Franz Xaver Gruber',
		lyricist: 'Joseph Mohr',
		societyNumber: '1234567-001',
		tags: ['Weihnachten'],
		createdAt: '',
		updatedAt: ''
	} as Work;
	const input = workInput(work, { durationSeconds: 210 });
	expect(input).toMatchObject({
		title: 'Stille Nacht',
		lyricist: 'Joseph Mohr',
		societyNumber: '1234567-001',
		durationSeconds: 210,
		tags: ['Weihnachten']
	});
	expect(Object.keys(input)).not.toContain('id');
});
