// SPDX-License-Identifier: AGPL-3.0-or-later
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { beforeEach, expect, it } from 'vitest';
import { de } from './de.js';
import { i18n, plural, t } from './index.svelte.js';

/** Every literal key passed to t() or plural() in the UI source. */
function usedKeys(dir = 'src'): string[] {
	const keys = new Set<string>();
	for (const name of readdirSync(dir)) {
		const path = join(dir, name);
		if (statSync(path).isDirectory()) {
			if (name !== 'i18n') usedKeys(path).forEach((k) => keys.add(k));
			continue;
		}
		if (!/\.(svelte|ts)$/.test(name) || name.endsWith('.test.ts') || name === 'schema.d.ts')
			continue;
		const source = readFileSync(path, 'utf8');
		const literal = String.raw`(['"])((?:\\.|(?!\1).)*)\1`;
		for (const m of source.matchAll(new RegExp(String.raw`\bt\(\s*` + literal, 'g')))
			keys.add(m[2]);
		for (const m of source.matchAll(
			new RegExp(
				String.raw`\bplural\(\s*[^,]+,\s*` +
					literal +
					String.raw`\s*,\s*(['"])((?:\\.|(?!\3).)*)\3`,
				'g'
			)
		)) {
			keys.add(m[2]);
			keys.add(m[4]);
		}
	}
	return [...keys].map((k) => k.replace(/\\'/g, "'"));
}

beforeEach(() => {
	i18n.locale = 'en';
});

it('has a German translation for every UI string', () => {
	const keys = usedKeys();
	expect(keys.length).toBeGreaterThan(150);
	const missing = keys.filter((key) => !(key in de));
	expect(missing).toEqual([]);
});

it('keeps the placeholders in translations', () => {
	const names = (s: string) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
	const broken = Object.entries(de).filter(
		([en, translated]) => names(en).join() !== names(translated).join()
	);
	expect(broken).toEqual([]);
});

it('translates and fills in placeholders', () => {
	expect(t('Invitation for {name}', { name: 'Anna' })).toBe('Invitation for Anna');
	i18n.locale = 'de';
	expect(t('Invitation for {name}', { name: 'Anna' })).toBe('Einladung für Anna');
	expect(t('Not translated anywhere')).toBe('Not translated anywhere');
	expect(plural(1, '{n} part', '{n} parts')).toBe('1 Stimme');
	expect(plural(3, '{n} part', '{n} parts')).toBe('3 Stimmen');
});
