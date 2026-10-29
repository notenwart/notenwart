// SPDX-License-Identifier: AGPL-3.0-or-later
// Builds src/service-worker.ts as dist/service-worker.js (a classic script with a fixed name, so the browser finds
// it again) after the app, with the list of files to cache and a version that changes with any of them.
import { createHash } from 'node:crypto';
import { readFile, readdir } from 'node:fs/promises';
import { join, relative, sep } from 'node:path';
import { build, type Plugin, type ResolvedConfig } from 'vite';

export const WORKER = 'service-worker.js';

/** The URLs the worker caches: the shell at `/`, every built file and every public file. */
export function shellFiles(built: string[], public_: string[]): string[] {
	const files = [...built, ...public_].filter(
		(file) => file !== 'index.html' && file !== WORKER && !file.endsWith('.map')
	);
	return ['/', ...[...new Set(files)].sort().map((file) => `/${file}`)];
}

/** A short version from the files' names and contents. */
export function shellVersion(files: [string, string | Uint8Array][]): string {
	const hash = createHash('sha256');
	for (const [name, content] of [...files].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))) {
		hash.update(name).update('\0').update(content).update('\0');
	}
	return hash.digest('hex').slice(0, 12);
}

export function serviceWorker(entry: string): Plugin {
	let config: ResolvedConfig;
	return {
		name: 'notenwart:service-worker',
		apply: 'build',
		configResolved(resolved) {
			config = resolved;
		},
		async writeBundle(_, bundle) {
			const contents: [string, string | Uint8Array][] = Object.values(bundle).map((file) => [
				file.fileName,
				file.type === 'chunk' ? file.code : file.source
			]);
			const publicFiles: string[] = [];
			if (config.publicDir) {
				for (const file of await readdir(config.publicDir, {
					recursive: true,
					withFileTypes: true
				})) {
					if (!file.isFile()) continue;
					const path = join(file.parentPath, file.name);
					const name = relative(config.publicDir, path).split(sep).join('/');
					publicFiles.push(name);
					contents.push([name, await readFile(path)]);
				}
			}
			const shell = shellFiles(Object.keys(bundle), publicFiles);
			await build({
				configFile: false,
				root: config.root,
				logLevel: 'warn',
				publicDir: false,
				define: {
					__SHELL__: JSON.stringify(shell),
					__VERSION__: JSON.stringify(shellVersion(contents))
				},
				build: {
					outDir: config.build.outDir,
					emptyOutDir: false,
					copyPublicDir: false,
					minify: config.build.minify,
					lib: {
						entry,
						formats: ['iife'],
						name: 'serviceWorker',
						fileName: () => WORKER
					}
				}
			});
		}
	};
}
