// SPDX-License-Identifier: AGPL-3.0-or-later
// Importing a whole folder of sheet music (a Google Drive download, a ZIP, a folder on the computer): which works and
// parts the files become. The page (routes/works/import) shows the plan for corrections and then carries it out.

import { unzip } from 'fflate';
import type { Instrument, Recognition } from './api/index.js';
import { guessFromFileName, partLabel } from './instruments.js';
import { fold } from './search.js';

/** A file from the chosen folder or ZIP; `path` is relative and uses `/`. */
export interface Entry {
	path: string;
	size: number;
	file: Blob;
}

/**
 * What happens with a file: `part` – one part with all its pages for an instrument; `score` – one part without an
 * instrument (full score, choir); `source` – only uploaded, to be split on the work's page; `skip` – left out.
 */
export type Mode = 'part' | 'score' | 'source' | 'skip';

export interface PlannedFile {
	id: string;
	path: string;
	name: string;
	size: number;
	file: Blob;
	mode: Mode;
	instrumentId: string;
	voice: number | null;
	label: string;
	/** What the server read on the first page, once asked. */
	found: Recognition | null;
}

export interface PlannedWork {
	id: string;
	title: string;
	tags: string[];
	/** A work in the library with the same title. */
	existingId: string | null;
	/**
	 * What to do when there is such a work – nothing happens before someone decides, so that files do not end up in a
	 * namesake by accident: `add` to the existing work, a `new` separate one, or still `open` (left out).
	 */
	decision: 'open' | 'add' | 'new';
	/** The title is the file name of a PDF directly in the chosen folder (the page knows better). */
	titleFromFile: boolean;
	/** A title read on a page that differs from the folder's name, offered to take over. */
	suggestedTitle: string | null;
	composer: string;
	arranger: string;
	lyricist: string;
	publisher: string;
	files: PlannedFile[];
}

export interface Plan {
	works: PlannedWork[];
	skipped: { path: string; reason: 'not a PDF' | 'too large' }[];
}

const SCORE = /\b(score|full score|partitur|direktion|dirigent|conductor)\b/i;

/** Files and folders the import never looks at: hidden ones and macOS extras in ZIPs. */
function hidden(segment: string) {
	return segment.startsWith('.') || segment === '__MACOSX' || segment === 'Thumbs.db';
}

/** "Alte_Kameraden" → "Alte Kameraden". */
function title(name: string) {
	return name
		.replace(/\.pdf$/i, '')
		.replace(/[_]+/g, ' ')
		.replace(/\s+/g, ' ')
		.trim();
}

/** Files that will be imported: not left out, and their work is decided where it has a namesake. */
export function chosenFiles(work: PlannedWork): PlannedFile[] {
	if (work.existingId && work.decision === 'open') return [];
	return work.files.filter((f) => f.mode !== 'skip');
}

/**
 * Groups the files into works: the folder a PDF is in names its work, the folders above it become tags ("Weihnachten
 * / Stille Nacht / Trompete 1.pdf"). A PDF directly in the chosen folder is a work of its own. One folder around
 * everything (the downloaded folder itself) is left out.
 */
export function planImport(
	entries: Entry[],
	instruments: Instrument[],
	existing: { id: string; title: string }[],
	maxBytes: number
): Plan {
	const files = entries
		.map((e) => ({ ...e, segments: e.path.split('/').filter(Boolean) }))
		.filter((e) => e.segments.length && !e.segments.some(hidden));
	const first = files[0]?.segments[0];
	if (first && files.every((f) => f.segments.length > 1 && f.segments[0] === first))
		for (const f of files) f.segments = f.segments.slice(1);

	const byTitle = new Map(existing.map((w) => [fold(w.title), w.id]));
	const works = new Map<string, PlannedWork>();
	const skipped: Plan['skipped'] = [];
	let n = 0;
	for (const f of [...files].sort((a, b) =>
		a.path.localeCompare(b.path, undefined, { numeric: true })
	)) {
		const name = f.segments[f.segments.length - 1];
		if (!/\.pdf$/i.test(name)) {
			skipped.push({ path: f.path, reason: 'not a PDF' });
			continue;
		}
		if (f.size > maxBytes) {
			skipped.push({ path: f.path, reason: 'too large' });
			continue;
		}
		const folders = f.segments.slice(0, -1);
		const key = folders.length ? folders.join('/') : `/${name}`;
		let work = works.get(key);
		if (!work) {
			const workTitle = title(folders.length ? folders[folders.length - 1] : name);
			work = {
				id: `w${works.size}`,
				title: workTitle,
				tags: folders.slice(0, -1).map(title).filter(Boolean),
				existingId: byTitle.get(fold(workTitle)) ?? null,
				decision: 'open',
				titleFromFile: !folders.length,
				suggestedTitle: null,
				composer: '',
				arranger: '',
				lyricist: '',
				publisher: '',
				files: []
			};
			works.set(key, work);
		}
		const guess = guessFromFileName(name, instruments);
		const mode: Mode = guess ? 'part' : SCORE.test(title(name)) ? 'score' : 'source';
		work.files.push({
			id: `f${n++}`,
			path: f.path,
			name,
			size: f.size,
			file: f.file,
			mode,
			instrumentId: guess?.instrument.id ?? '',
			voice: guess?.voice ?? null,
			label: guess ? partLabel(guess.instrument, guess.voice) : title(name),
			found: null
		});
	}
	return { works: [...works.values()], skipped };
}

/**
 * Takes over what was read on a file's first page. The file name stays in charge of the part when it names one;
 * otherwise the page decides. The work gets composer, arranger and publisher where they are still empty, and a
 * title when its own came from a file name – a folder's name is only challenged by a suggestion.
 */
export function applyRecognition(
	work: PlannedWork,
	file: PlannedFile,
	found: Recognition,
	instruments: Instrument[],
	existing: { id: string; title: string }[]
) {
	file.found = found;
	const instrument = instruments.find((i) => i.id === found.instrumentId);
	if (instrument && file.mode !== 'skip') {
		if (!file.instrumentId || file.mode !== 'part') {
			file.mode = 'part';
			file.instrumentId = instrument.id;
			file.voice = found.voice ?? null;
			file.label = partLabel(instrument, file.voice);
		} else if (file.instrumentId === instrument.id && !file.voice && found.voice) {
			file.voice = found.voice;
			file.label = partLabel(instrument, file.voice);
		}
	} else if (!instrument && file.mode === 'source' && found.part && SCORE.test(found.part)) {
		file.mode = 'score';
		file.label = found.part;
	}
	work.composer ||= found.composer ?? '';
	work.arranger ||= found.arranger ?? '';
	work.lyricist ||= found.lyricist ?? '';
	work.publisher ||= found.publisher ?? '';
	const read = found.title?.trim();
	if (read && fold(read) !== fold(work.title)) {
		if (work.titleFromFile && !work.existingId) {
			work.title = read;
			work.titleFromFile = false;
			work.existingId = existing.find((w) => fold(w.title) === fold(read))?.id ?? null;
		} else if (!work.existingId) work.suggestedTitle = read;
	}
}

/**
 * ZIPs from macOS and Windows often store UTF-8 names without saying so; fflate then reads them as Latin-1
 * ("TrompeteÌ\u0088"). Such names are read again as UTF-8.
 */
export function fixName(name: string): string {
	// Only names made of single bytes can be misread UTF-8.
	const singleBytes = [...name].every((c) => c.charCodeAt(0) <= 0xff);
	if (!singleBytes || !/[\u0080-ÿ]/.test(name)) return name.normalize('NFC');
	try {
		const bytes = Uint8Array.from(name, (c) => c.charCodeAt(0));
		return new TextDecoder('utf-8', { fatal: true }).decode(bytes).normalize('NFC');
	} catch {
		return name.normalize('NFC');
	}
}

/** The files in a ZIP. */
export async function readZip(zip: Blob): Promise<Entry[]> {
	const data = new Uint8Array(await zip.arrayBuffer());
	// Only PDFs are unpacked; other files are listed (and skipped later) without reading them.
	const others: Entry[] = [];
	const files = await new Promise<Record<string, Uint8Array>>((resolve, reject) =>
		unzip(
			data,
			{
				filter: (file) => {
					if (file.name.endsWith('/')) return false;
					if (/\.pdf$/i.test(file.name)) return true;
					others.push({ path: fixName(file.name), size: file.originalSize, file: new Blob() });
					return false;
				}
			},
			(error, result) => (error ? reject(error) : resolve(result))
		)
	);
	const pdfs = Object.entries(files).map(([path, bytes]) => ({
		path: fixName(path),
		size: bytes.length,
		file: new Blob([bytes as BlobPart], { type: 'application/pdf' })
	}));
	return [...pdfs, ...others];
}

/** Files and folders dropped on the page, read recursively (Chrome, Edge, Firefox and Safari). */
export async function readDropped(items: DataTransferItemList): Promise<Entry[]> {
	const entries: Entry[] = [];
	async function walk(entry: FileSystemEntry, path: string): Promise<void> {
		if (entry.isFile) {
			const file = await new Promise<File>((resolve, reject) =>
				(entry as FileSystemFileEntry).file(resolve, reject)
			);
			if (/\.zip$/i.test(file.name)) entries.push(...(await readZip(file)));
			else entries.push({ path: path + file.name, size: file.size, file });
		} else if (entry.isDirectory) {
			const reader = (entry as FileSystemDirectoryEntry).createReader();
			// readEntries returns at most 100 entries per call.
			for (;;) {
				const batch = await new Promise<FileSystemEntry[]>((resolve, reject) =>
					reader.readEntries(resolve, reject)
				);
				if (!batch.length) break;
				for (const child of batch) await walk(child, `${path}${entry.name}/`);
			}
		}
	}
	const roots = [...items].map((item) => item.webkitGetAsEntry()).filter((e) => e !== null);
	for (const root of roots) await walk(root, '');
	return entries.map((e) => ({ ...e, path: e.path.normalize('NFC') }));
}

/** Files from `<input type="file" webkitdirectory>` or a plain multiple file input (ZIPs are unpacked). */
export async function readFiles(files: FileList): Promise<Entry[]> {
	const entries: Entry[] = [];
	for (const file of files) {
		if (/\.zip$/i.test(file.name)) entries.push(...(await readZip(file)));
		else
			entries.push({
				path: (file.webkitRelativePath || file.name).normalize('NFC'),
				size: file.size,
				file
			});
	}
	return entries;
}
