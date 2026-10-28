// SPDX-License-Identifier: AGPL-3.0-or-later
// Matching for search fields and selects: case- and accent-insensitive, every typed word must occur.

export interface Option {
	value: string;
	label: string;
	/** Heading the option is listed under (e.g. the work of a part). */
	group?: string;
	/** Shown dimmed after the label. */
	hint?: string;
	/** More words to search by (aliases, translations). */
	keywords?: string;
}

/** Lower case without accents, so "flote" finds "Flöte". */
export function fold(text: string): string {
	return text
		.normalize('NFD')
		.replace(/\p{Diacritic}/gu, '')
		.toLowerCase();
}

/** True when every word of the query occurs in the text. */
export function matches(text: string, query: string): boolean {
	const words = fold(query).split(/\s+/).filter(Boolean);
	const haystack = fold(text);
	return words.every((w) => haystack.includes(w));
}

/** Options matching the query, in their order. */
export function filterOptions(options: Option[], query: string): Option[] {
	if (!query.trim()) return options;
	return options.filter((o) =>
		matches(`${o.label} ${o.group ?? ''} ${o.hint ?? ''} ${o.keywords ?? ''}`, query)
	);
}

/**
 * Suggestions for a free-text field: values matching what was typed, those starting with it first, without the
 * value typed exactly and without `exclude` (tags already chosen). Keeps the given order (most used first) otherwise.
 */
export function suggest(
	values: string[],
	query: string,
	exclude: string[] = [],
	limit = 8
): string[] {
	const typed = fold(query.trim());
	const skip = new Set([typed, ...exclude.map(fold)]);
	const found = values.filter((v) => !skip.has(fold(v)) && matches(v, query));
	const starts = found.filter((v) => fold(v).startsWith(typed));
	const rest = found.filter((v) => !fold(v).startsWith(typed));
	return [...starts, ...rest].slice(0, limit);
}
