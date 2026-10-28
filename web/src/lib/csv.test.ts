// SPDX-License-Identifier: AGPL-3.0-or-later
import { expect, it } from 'vitest';
import { decode, detectDelimiter, parse, toCsv, toPeople } from './csv.js';

it('decodes UTF-8 and falls back to Windows-1252', () => {
	expect(decode(new Uint8Array([0xef, 0xbb, 0xbf, 0x46, 0xc3, 0xb6]))).toBe('Fö');
	expect(decode(new Uint8Array([0x46, 0xf6, 0x74, 0x65]))).toBe('Föte');
});

it('detects the separator', () => {
	expect(detectDelimiter('Name;E-Mail\nA;b')).toBe(';');
	expect(detectDelimiter('Name,Email')).toBe(',');
	expect(detectDelimiter('Name\tEmail')).toBe('\t');
	expect(detectDelimiter('"Nachname; Vorname",Email')).toBe(',');
});

it('parses quotes, doubled quotes, line breaks and CRLF', () => {
	const text = 'a;"b;c";"say ""hi"""\r\n"multi\nline";x;\r\n\r\n';
	expect(parse(text)).toEqual([
		['a', 'b;c', 'say "hi"'],
		['multi\nline', 'x', '']
	]);
});

it('maps German headers and joins first and last names', () => {
	const table = parse(
		'Vorname;Nachname;E-Mail-Adresse;Kurs;Instrument;Stimme\nAnna;Alt;anna@example.org;Blasorchester;Trompete;2\nBen;;ben@example.org;;;'
	);
	const people = toPeople(table);
	expect(people.problem).toBeNull();
	expect(people.rows).toEqual([
		{
			name: 'Anna Alt',
			email: 'anna@example.org',
			role: null,
			ensemble: 'Blasorchester',
			instrument: 'Trompete',
			voice: 2
		},
		{
			name: 'Ben',
			email: 'ben@example.org',
			role: null,
			ensemble: null,
			instrument: null,
			voice: null
		}
	]);
});

it('needs a name and an email column', () => {
	expect(toPeople(parse('Name;Instrument\nAnna;Flöte')).problem).toMatch(/email column/);
	expect(toPeople(parse('Name;Email')).problem).toMatch(/no rows/);
});

it('writes CSV for German Excel', () => {
	expect(
		toCsv([
			['Name', 'Link'],
			['A; B', 'say "x"']
		])
	).toBe('\uFEFFName;Link\r\n"A; B";"say ""x"""\r\n');
});
