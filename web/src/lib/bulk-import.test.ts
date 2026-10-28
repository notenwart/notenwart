// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import type { Instrument, Recognition } from './api/index.js';
import { applyRecognition, chosenFiles, fixName, planImport, type Entry } from './bulk-import.js';

const instrument = (id: string, name: string, aliases: string[]): Instrument => ({
	id,
	name,
	familyId: 'f',
	family: 'F',
	transposition: null,
	aliases,
	sort: 0,
	usage: 0
});
const catalogue = [
	instrument('tpt', 'Trompete', ['Trumpet', 'Trp']),
	instrument('cl', 'Klarinette', ['Clarinet', 'Klar'])
];
const entry = (path: string, size = 1000): Entry => ({ path, size, file: new Blob() });

describe('planImport', () => {
	it('makes a work of each folder and tags of the folders above it', () => {
		const plan = planImport(
			[
				entry('Noten/Weihnachten/Stille_Nacht/Trompete 1.pdf'),
				entry('Noten/Weihnachten/Stille_Nacht/Trompete 2.pdf'),
				entry('Noten/Weihnachten/Stille_Nacht/Partitur.pdf'),
				entry('Noten/Weihnachten/Stille_Nacht/Alle Stimmen.pdf'),
				entry('Noten/Alte Kameraden/Klar_2.pdf')
			],
			catalogue,
			[],
			1e6
		);
		expect(plan.works.map((w) => [w.title, w.tags])).toEqual([
			['Alte Kameraden', []],
			['Stille Nacht', ['Weihnachten']]
		]);
		const files = plan.works[1].files.map((f) => [
			f.name,
			f.mode,
			f.instrumentId,
			f.voice,
			f.label
		]);
		expect(files).toEqual([
			['Alle Stimmen.pdf', 'source', '', null, 'Alle Stimmen'],
			['Partitur.pdf', 'score', '', null, 'Partitur'],
			['Trompete 1.pdf', 'part', 'tpt', 1, 'Trompete 1'],
			['Trompete 2.pdf', 'part', 'tpt', 2, 'Trompete 2']
		]);
		expect(plan.works[0].files[0].label).toBe('Klarinette 2');
	});

	it('makes a work of each file directly in the chosen folder', () => {
		const plan = planImport([entry('Radetzky-Marsch.pdf'), entry('Polka.pdf')], catalogue, [], 1e6);
		expect(plan.works.map((w) => w.title)).toEqual(['Polka', 'Radetzky-Marsch']);
	});

	it('finds existing works by title and skips what it cannot import', () => {
		const plan = planImport(
			[
				entry('Noten/Polka/Trp.pdf'),
				entry('Noten/Polka/notes.txt'),
				entry('Noten/Polka/Huge.pdf', 5e6),
				entry('Noten/.DS_Store'),
				entry('__MACOSX/Noten/Polka/._Trp.pdf')
			],
			catalogue,
			[{ id: 'w1', title: 'polka' }],
			1e6
		);
		expect(plan.works).toHaveLength(1);
		expect(plan.works[0].existingId).toBe('w1');
		// Nothing goes into the existing work before someone decides.
		expect(chosenFiles(plan.works[0])).toEqual([]);
		plan.works[0].decision = 'add';
		expect(chosenFiles(plan.works[0]).map((f) => f.name)).toEqual(['Trp.pdf']);
		expect(plan.skipped).toEqual([
			{ path: 'Noten/Polka/Huge.pdf', reason: 'too large' },
			{ path: 'Noten/Polka/notes.txt', reason: 'not a PDF' }
		]);
	});
});

it('reads UTF-8 file names stored without the UTF-8 flag', () => {
	const misread = String.fromCharCode(...new TextEncoder().encode('Flöte 1.pdf'));
	expect(fixName(misread)).toBe('Flöte 1.pdf');
	expect(fixName('Flöte 1.pdf')).toBe('Flöte 1.pdf');
	expect(fixName('Flöte.pdf')).toBe('Flöte.pdf');
});

describe('applyRecognition', () => {
	const found = (fields: Partial<Recognition>): Recognition => ({
		method: 'text',
		ai: false,
		...fields
	});

	it('lets the page name the part when the file name does not', () => {
		const plan = planImport([entry('Noten/Polka/scan_004.pdf')], catalogue, [], 1e6);
		const [work] = plan.works;
		const [file] = work.files;
		applyRecognition(
			work,
			file,
			found({ instrumentId: 'cl', voice: 2, composer: 'Ernst Mosch', title: 'Fuchsgraben-Polka' }),
			catalogue,
			[]
		);
		expect([file.mode, file.instrumentId, file.voice, file.label]).toEqual([
			'part',
			'cl',
			2,
			'Klarinette 2'
		]);
		expect(work.composer).toBe('Ernst Mosch');
		// The folder's name stays; the page's title is offered.
		expect([work.title, work.suggestedTitle]).toEqual(['Polka', 'Fuchsgraben-Polka']);
	});

	it('keeps the part from the file name and takes the title of a single file', () => {
		const plan = planImport(
			[entry('Trompete 1.pdf')],
			catalogue,
			[{ id: 'w9', title: 'Stille Nacht' }],
			1e6
		);
		const [work] = plan.works;
		const [file] = work.files;
		applyRecognition(work, file, found({ instrumentId: 'cl', title: 'Stille Nacht' }), catalogue, [
			{ id: 'w9', title: 'Stille Nacht' }
		]);
		expect([file.instrumentId, file.voice]).toEqual(['tpt', 1]);
		// Now a namesake of an existing work: nothing happens before someone decides.
		expect([work.title, work.existingId, work.decision]).toEqual(['Stille Nacht', 'w9', 'open']);
	});

	it('recognises scores', () => {
		const plan = planImport([entry('Noten/Marsch/Marsch.pdf')], catalogue, [], 1e6);
		const [work] = plan.works;
		applyRecognition(work, work.files[0], found({ part: 'Direktion' }), catalogue, []);
		expect([work.files[0].mode, work.files[0].label]).toEqual(['score', 'Direktion']);
	});
});
