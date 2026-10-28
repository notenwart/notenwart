// SPDX-License-Identifier: AGPL-3.0-or-later
import { expect, it } from 'vitest';
import { bytes, open } from './setup.js';

it('counts the open required steps and sizes', () => {
	const step = (key: string, done: boolean, optional = false) =>
		({ key, done, optional, manual: false }) as never;
	expect(open([step('logo', false, true), step('people', false), step('mail', true)]).length).toBe(
		1
	);
	expect(bytes(1_234_567, 'de')).toBe('1,2 MB');
	expect(bytes(512, 'en')).toBe('512 B');
});
