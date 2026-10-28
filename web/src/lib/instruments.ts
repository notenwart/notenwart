// SPDX-License-Identifier: AGPL-3.0-or-later
import type { Instrument } from './api/index.js';
import { t } from './i18n/index.svelte.js';
import type { Option } from './search.js';

export interface Guess {
	instrument: Instrument;
	voice: number | null;
}

function normalize(text: string): string {
	return ` ${text
		.toLowerCase()
		.replace(/\.pdf$/, '')
		.replace(/[^\p{L}\p{N}]+/gu, ' ')
		.trim()} `;
}

/**
 * Recognises an instrument and part number in a file name such as "Trompete_2.pdf" or "03 Alto Sax 1.pdf".
 * Names and aliases must match as whole words; the longest match wins ("Bass clarinet" over "Clarinet").
 */
export function guessFromFileName(fileName: string, instruments: Instrument[]): Guess | null {
	const name = normalize(fileName);
	let best: { instrument: Instrument; alias: string; index: number } | null = null;
	for (const instrument of instruments) {
		for (const alias of [instrument.name, ...instrument.aliases]) {
			const needle = normalize(alias);
			const index = name.indexOf(needle);
			if (index >= 0 && (!best || needle.length > best.alias.length)) {
				best = { instrument, alias: needle, index };
			}
		}
	}
	if (!best) return null;
	const after = name.slice(best.index + best.alias.length - 1);
	const voice = after.match(/^ (\d{1,2}) /);
	return { instrument: best.instrument, voice: voice ? Number(voice[1]) : null };
}

/** A part's default label, e.g. "Trompete 2" – instruments are named in the organisation's own catalogue. */
export function partLabel(instrument: Instrument | undefined, voice: number | null): string {
	if (!instrument) return '';
	return voice ? `${instrument.name} ${voice}` : instrument.name;
}

/** Instruments as select options: grouped by family (in the server's order), transposition as a hint, aliases
 * searchable. */
export function instrumentOptions(instruments: Instrument[]): Option[] {
	return instruments.map((i) => ({
		value: i.id,
		label: i.name,
		group: i.family,
		hint: i.transposition ? t('in {key}', { key: i.transposition }) : undefined,
		keywords: i.aliases.join(' ')
	}));
}
