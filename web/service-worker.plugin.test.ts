// SPDX-License-Identifier: AGPL-3.0-or-later
import { expect, it } from 'vitest';
import { shellFiles, shellVersion } from './service-worker.plugin.js';

it('caches the shell, the built files and the public files', () => {
	expect(
		shellFiles(
			['index.html', 'assets/index-abc.js', 'assets/index-abc.css', 'assets/index-abc.js.map'],
			['favicon.svg', 'theme.js', 'service-worker.js']
		)
	).toEqual(['/', '/assets/index-abc.css', '/assets/index-abc.js', '/favicon.svg', '/theme.js']);
});

it('changes the version with any file, not with their order', () => {
	const a = shellVersion([
		['a.js', 'one'],
		['b.css', 'two']
	]);
	expect(
		shellVersion([
			['b.css', 'two'],
			['a.js', 'one']
		])
	).toBe(a);
	expect(
		shellVersion([
			['a.js', 'one'],
			['b.css', 'three']
		])
	).not.toBe(a);
});
