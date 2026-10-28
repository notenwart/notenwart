// SPDX-License-Identifier: AGPL-3.0-or-later
// Offline shell: caches the built app so it starts without a network.
// Sheet music for offline use gets its own store with milestone 1 (spec §4); the API is never cached here.
import { self } from '$app/service-worker';
import { immutable, assets } from '$app/manifest';
import { version } from '$app/environment';

const CACHE = `shell-${version}`;
const SHELL = ['/', ...immutable.map((f) => f.path), ...assets.map((f) => f.path)];

self.addEventListener('install', (event) => {
	event.waitUntil(caches.open(CACHE).then((cache) => cache.addAll(SHELL)));
});

self.addEventListener('activate', (event) => {
	event.waitUntil(
		caches
			.keys()
			.then((keys) =>
				Promise.all(keys.filter((key) => key !== CACHE).map((key) => caches.delete(key)))
			)
			// Take over open tabs right away, so the first visit already works offline.
			.then(() => self.clients.claim())
	);
});

self.addEventListener('fetch', (event) => {
	const url = new URL(event.request.url);
	if (event.request.method !== 'GET' || url.origin !== self.location.origin) return;
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
