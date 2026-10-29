// SPDX-License-Identifier: AGPL-3.0-or-later
// The web app is built into dist/, which the notenwart binary embeds, together with dist/service-worker.js. In
// development, `pnpm dev` serves it and passes /api on to a `notenwart serve` – on 127.0.0.1:8080 as by default, or
// where NOTENWART_LISTEN says.
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';
import { pdfjsResources } from './pdfjs.plugin.js';
import { serviceWorker } from './service-worker.plugin.js';

const api = `http://${process.env.NOTENWART_LISTEN ?? '127.0.0.1:8080'}`;

// The policy the server sends with the app; `pnpm preview` sends it too, so violations show up before a release.
// (Not in `pnpm dev`: the dev server injects styles inline.)
const csp = [
	"default-src 'self'",
	"script-src 'self'",
	"style-src 'self'",
	"img-src 'self' data: blob:",
	"media-src 'self' blob:",
	"worker-src 'self' blob:",
	"font-src 'self'",
	"connect-src 'self'",
	"frame-ancestors 'none'",
	"base-uri 'none'",
	"form-action 'self'"
].join('; ');

export default defineConfig({
	plugins: [svelte(), pdfjsResources(), serviceWorker('src/service-worker.ts')],
	build: { outDir: 'dist', emptyOutDir: true, assetsInlineLimit: 0 },
	// The browser's Host header is passed on as it is: the server refuses changes whose Origin is another host.
	server: { proxy: { '/api': { target: api } } },
	preview: { headers: { 'Content-Security-Policy': csp } }
});
