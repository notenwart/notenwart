// SPDX-License-Identifier: AGPL-3.0-or-later
// The files pdf.js loads while it reads a PDF – image decoders for JPEG 2000 and JBIG2 (WebAssembly and its plain
// JavaScript fallback; scans often use them), the standard fonts and the character maps – served under /pdfjs/ from
// pdfjs-dist. In a build they become part of the bundle, so the service worker keeps them for offline parts too.
import { readFile, readdir } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import type { Plugin } from 'vite';

export const BASE = '/pdfjs/';
const FOLDERS = ['wasm', 'cmaps', 'standard_fonts', 'iccs'];
// quickjs-eval runs scripts inside PDFs; notenwart does not, and it would need eval.
const SKIP = /^quickjs/;

const root = dirname(createRequire(import.meta.url).resolve('pdfjs-dist/package.json'));

async function files(): Promise<string[]> {
	const names: string[] = [];
	for (const folder of FOLDERS) {
		for (const name of await readdir(join(root, folder))) {
			if (!SKIP.test(name)) names.push(`${folder}/${name}`);
		}
	}
	return names;
}

export function pdfjsResources(): Plugin {
	return {
		name: 'notenwart:pdfjs-resources',
		configureServer(server) {
			server.middlewares.use(BASE, async (request, response, next) => {
				const name = decodeURIComponent((request.url ?? '').split('?')[0].replace(/^\//, ''));
				if (!(await files()).includes(name)) return next();
				if (name.endsWith('.wasm')) response.setHeader('Content-Type', 'application/wasm');
				response.end(await readFile(join(root, name)));
			});
		},
		async generateBundle() {
			for (const name of await files()) {
				this.emitFile({
					type: 'asset',
					fileName: `${BASE.slice(1)}${name}`,
					source: await readFile(join(root, name))
				});
			}
		}
	};
}
