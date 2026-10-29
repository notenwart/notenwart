// SPDX-License-Identifier: AGPL-3.0-or-later
import { expect, it } from 'vitest';
import { find, match, resolve } from './routing.js';

const routes = [
	{ pattern: '/' },
	{ pattern: '/works' },
	{ pattern: '/works/import' },
	{ pattern: '/works/[id]' },
	{ pattern: '/ensembles/[id]/programs/[programId]/setlist' }
];

it('matches static segments and parameters', () => {
	expect(match('/', '/')).toEqual({});
	expect(match('/works', '/works/')).toEqual({});
	expect(match('/works/[id]', '/works/a%20b')).toEqual({ id: 'a b' });
	expect(match('/works/[id]', '/works')).toBeNull();
	expect(match('/works/[id]', '/works/1/2')).toBeNull();
	expect(match('/works/[id]', '/works/%E0')).toBeNull();
});

it('finds the first fitting route, static ones first', () => {
	expect(find(routes, '/works/import')?.route.pattern).toBe('/works/import');
	expect(find(routes, '/works/42')).toEqual({ route: routes[3], params: { id: '42' } });
	expect(find(routes, '/ensembles/e/programs/p/setlist')?.params).toEqual({
		id: 'e',
		programId: 'p'
	});
	expect(find(routes, '/api/health')).toBeNull();
});

it('fills in route parameters', () => {
	expect(resolve('/')).toBe('/');
	expect(resolve('/login?next=%2Fworks')).toBe('/login?next=%2Fworks');
	expect(resolve('/ensembles/[id]/programs/[programId]', { id: 'e', programId: 'p/q' })).toBe(
		'/ensembles/e/programs/p%2Fq'
	);
	expect(() => resolve('/works/[id]')).toThrow();
});
