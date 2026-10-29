// SPDX-License-Identifier: AGPL-3.0-or-later
// Offline shell: caches the built app so it starts without a network.
// Sheet music for offline use gets its own store (lib/offline.ts); the API is never cached here.
// Built on its own as /service-worker.js by service-worker.plugin.ts, which fills in __SHELL__ and __VERSION__.
/// <reference lib="webworker" />

declare const __SHELL__: string[];
declare const __VERSION__: string;

const worker = globalThis as unknown as ServiceWorkerGlobalScope;
const CACHE = `shell-${__VERSION__}`;
const SHELL = __SHELL__;

worker.addEventListener('install', (event) => {
	event.waitUntil(caches.open(CACHE).then((cache) => cache.addAll(SHELL)));
});

worker.addEventListener('activate', (event) => {
	event.waitUntil(
		caches
			.keys()
			.then((keys) =>
				Promise.all(keys.filter((key) => key !== CACHE).map((key) => caches.delete(key)))
			)
			// Take over open tabs right away, so the first visit already works offline.
			.then(() => worker.clients.claim())
	);
});

worker.addEventListener('fetch', (event) => {
	const url = new URL(event.request.url);
	if (event.request.method !== 'GET' || url.origin !== worker.location.origin) return;
	if (url.pathname.startsWith('/api/')) return;

	event.respondWith(
		(async () => {
			const cache = await caches.open(CACHE);
			const cached = await cache.match(event.request);
			if (cached) return cached;
			try {
				return await fetch(event.request);
			} catch (error) {
				// Offline navigation to a client-side route: serve the app shell.
				if (event.request.mode === 'navigate') {
					const shell = await cache.match('/');
					if (shell) return shell;
				}
				throw error;
			}
		})()
	);
});
