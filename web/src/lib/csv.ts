// SPDX-License-Identifier: AGPL-3.0-or-later
// Reading people tables exported from spreadsheets (Excel, LibreOffice, Numbers) for the import.

/** UTF-8 if the bytes are valid UTF-8, else Windows-1252 (what German Excel writes); without a byte-order mark. */
export function decode(bytes: Uint8Array): string {
	let text: string;
	try {
		text = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
	} catch {
		text = new TextDecoder('windows-1252').decode(bytes);
	}
	return text.charCodeAt(0) === 0xfeff ? text.slice(1) : text;
}

/** The separator used in the first line: semicolon, comma or tab, whichever occurs most outside quotes. */
export function detectDelimiter(text: string): string {
	const first = text.split(/\r?\n/, 1)[0] ?? '';
	const unquoted = first.replace(/"[^"]*"/g, '');
	const counts = [';', ',', '\t'].map((d) => [d, unquoted.split(d).length - 1] as const);
	counts.sort((a, b) => b[1] - a[1]);
	return counts[0][1] > 0 ? counts[0][0] : ';';
}

/** RFC 4180 parsing: quoted fields, doubled quotes, line breaks inside quotes, CRLF. Empty lines are dropped. */
export function parse(text: string, delimiter = detectDelimiter(text)): string[][] {
	const rows: string[][] = [];
	let row: string[] = [];
	let field = '';
	let quoted = false;
	for (let i = 0; i < text.length; i++) {
		const c = text[i];
		if (quoted) {
			if (c === '"' && text[i + 1] === '"') {
				field += '"';
				i++;
			} else if (c === '"') quoted = false;
			else field += c;
		} else if (c === '"' && field === '') quoted = true;
		else if (c === delimiter) {
			row.push(field);
			field = '';
		} else if (c === '\n' || c === '\r') {
			if (c === '\r' && text[i + 1] === '\n') i++;
			row.push(field);
			rows.push(row);
			row = [];
			field = '';
		} else field += c;
	}
	if (field !== '' || row.length) {
		row.push(field);
		rows.push(row);
	}
	return rows.filter((r) => r.some((f) => f.trim() !== ''));
}

export interface PersonRow {
	name: string;
	email: string;
	role: string | null;
	ensemble: string | null;
	instrument: string | null;
	voice: number | null;
}

type Column =
	'name' | 'firstName' | 'lastName' | 'email' | 'role' | 'ensemble' | 'instrument' | 'voice';

/** Header names in English and German, compared without case, spaces and punctuation. */
const headers: Record<string, Column> = {
	name: 'name',
	fullname: 'name',
	vollername: 'name',
	firstname: 'firstName',
	vorname: 'firstName',
	lastname: 'lastName',
	surname: 'lastName',
	nachname: 'lastName',
	familienname: 'lastName',
	email: 'email',
	mail: 'email',
	emailadresse: 'email',
	role: 'role',
	rolle: 'role',
	ensemble: 'ensemble',
	group: 'ensemble',
	gruppe: 'ensemble',
	kurs: 'ensemble',
	klasse: 'ensemble',
	class: 'ensemble',
	orchester: 'ensemble',
	instrument: 'instrument',
	voice: 'voice',
	part: 'voice',
	partnumber: 'voice',
	stimme: 'voice',
	stimmnummer: 'voice'
};

export interface Table {
	rows: PersonRow[];
	/** Recognised columns, by header text. */
	columns: Record<string, Column>;
	/** What is missing for an import (English, for t()). */
	problem: string | null;
}

/** Turns a parsed table (header first) into rows for the import. */
export function toPeople(table: string[][]): Table {
	const [header = [], ...body] = table;
	const index: Partial<Record<Column, number>> = {};
	const columns: Record<string, Column> = {};
	header.forEach((title, i) => {
		const column = headers[title.toLowerCase().replace(/[^\p{L}\p{N}]/gu, '')];
		if (column && index[column] === undefined) {
			index[column] = i;
			columns[title] = column;
		}
	});
	const hasName =
		index.name !== undefined || index.firstName !== undefined || index.lastName !== undefined;
	if (!hasName || index.email === undefined) {
		return { rows: [], columns, problem: 'The table needs a name column and an email column.' };
	}
	const cell = (row: string[], column: Column) => {
		const i = index[column];
		const value = i === undefined ? '' : (row[i] ?? '').trim();
		return value === '' ? null : value;
	};
	const rows = body.map((row) => {
		const name =
			cell(row, 'name') ??
			[cell(row, 'firstName'), cell(row, 'lastName')].filter(Boolean).join(' ');
		const voice = cell(row, 'voice');
		const number = voice === null ? null : Number.parseInt(voice, 10);
		return {
			name,
			email: cell(row, 'email') ?? '',
			role: cell(row, 'role'),
			ensemble: cell(row, 'ensemble'),
			instrument: cell(row, 'instrument'),
			voice: number !== null && Number.isFinite(number) ? number : null
		};
	});
	return { rows, columns, problem: rows.length ? null : 'The table has no rows below the header.' };
}

/** A CSV with semicolons (opens directly in German Excel), quoting where needed, with a byte-order mark. */
export function toCsv(rows: string[][]): string {
	const quote = (value: string) =>
		/[;"\n\r]/.test(value) ? `"${value.replace(/"/g, '""')}"` : value;
	return '\uFEFF' + rows.map((row) => row.map(quote).join(';')).join('\r\n') + '\r\n';
}
