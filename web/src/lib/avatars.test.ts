// SPDX-License-Identifier: AGPL-3.0-or-later
import { expect, it } from 'vitest';
import { colorFor, initials } from './avatars.js';

it('makes initials and keeps colours', () => {
	expect(initials('Musik- und Kunstschule Seestadt')).toBe('MS');
	expect(initials('Kammerchor am Tor 3')).toBe('KT');
	expect(initials('Anna')).toBe('A');
	expect(initials('Jürgen Größe')).toBe('JG');
	expect(colorFor('Anna')).toBe(colorFor('Anna'));
});
