// SPDX-License-Identifier: AGPL-3.0-or-later
// The client-side router: pages by path (lib/routing.ts patterns, list in src/routes.ts), the History API, links
// within the app handled in the page, scroll positions kept for back and forward. Paths no route knows (the API,
// files) are left to the browser.
// The URLs and the map of loaded pages are replaced, never changed in place, so they need no reactive versions.
/* eslint-disable svelte/prefer-svelte-reactivity */
import { tick, type Component } from 'svelte';
import { find, type Params } from './routing.js';

export { resolve } from './routing.js';

export interface Route {
	pattern: string;
	/** The page, loaded on first use so the start stays small. */
	load: () => Promise<{ default: Component }>;
}

/** The current location, its route and parameters – what pages read. */
export const page = $state<{ url: URL; params: Params; route: string | null }>({
	url: new URL(typeof location === 'undefined' ? 'http://localhost/' : location.href),
	params: {},
	route: null
});

/** The page component to show; null while the first one loads or when no route fits (`missing`). */
export const view = $state<{ component: Component | null; missing: boolean }>({
	component: null,
	missing: false
});

let routes: readonly Route[] = [];
const components = new Map<Route, Component>();
// Each history entry gets an index; scroll positions are kept per index (also across a reload).
const SCROLL_KEY = 'notenwart.scroll';
let index = 0;
let scrolls: Record<number, [number, number]> = {};
// Only the newest navigation may finish, when pages load slowly.
let pending = 0;

export interface GotoOptions {
	replaceState?: boolean;
	noScroll?: boolean;
	keepFocus?: boolean;
}

/** Navigates to `href` within the app; a path no route knows is loaded by the browser. */
export async function goto(href: string, options: GotoOptions = {}) {
	const url = new URL(href, location.href);
	if (url.origin !== location.origin || !find(routes, url.pathname)) {
		if (options.replaceState) location.replace(url);
		else location.assign(url);
		return;
	}
	await navigate(url, options.replaceState ? 'replace' : 'push', options);
}

/** Starts the router on the current location; called once by the app shell. */
export function start(list: readonly Route[]) {
	routes = list;
	history.scrollRestoration = 'manual';
	try {
		scrolls = JSON.parse(sessionStorage.getItem(SCROLL_KEY) ?? '{}');
	} catch {
		scrolls = {};
	}
	index = typeof history.state?.index === 'number' ? history.state.index : 0;
	history.replaceState({ ...history.state, index }, '');

	addEventListener('click', onClick);
	addEventListener('popstate', onPopState);
	addEventListener('pagehide', saveScroll);
	void navigate(new URL(location.href), 'start', {});
}

async function navigate(
	url: URL,
	kind: 'start' | 'push' | 'replace' | 'pop',
	options: GotoOptions,
	popped = index
) {
	const id = ++pending;
	const found = find(routes, url.pathname);
	let component: Component | null = null;
	if (found) {
		component = components.get(found.route) ?? null;
		if (!component) {
			try {
				component = (await found.route.load()).default;
			} catch {
				// A newer build replaced the page's file (or the network is gone): let the browser load it.
				if (id === pending) location.assign(url);
				return;
			}
			components.set(found.route, component);
		}
	}
	if (id !== pending) return;

	if (kind === 'push' || kind === 'replace') {
		saveScroll();
		if (kind === 'push') index += 1;
		const state = { index };
		if (kind === 'push') history.pushState(state, '', url);
		else history.replaceState(state, '', url);
	} else if (kind === 'pop') {
		index = popped;
	}
	page.url = url;
	page.params = found?.params ?? {};
	page.route = found?.route.pattern ?? null;
	view.component = component;
	view.missing = !found;
	await tick();

	if (kind === 'pop' || kind === 'start') {
		const [x, y] = scrolls[index] ?? [0, 0];
		if (kind === 'pop' || x || y) restoreScroll(x, y);
		else scrollToHash(url);
	} else if (!options.noScroll) {
		if (!scrollToHash(url)) scrollTo(0, 0);
	}
	if (kind !== 'start' && !options.keepFocus) resetFocus();
}

/** Back to an earlier position – also once the page has loaded its data and grown tall enough (within 2 s). */
function restoreScroll(x: number, y: number) {
	scrollTo(x, y);
	if (scrollY >= y - 1) return;
	const observer = new ResizeObserver(() => {
		scrollTo(x, y);
		if (scrollY >= y - 1) observer.disconnect();
	});
	observer.observe(document.body);
	setTimeout(() => observer.disconnect(), 2000);
	// Someone scrolling in the meantime decides.
	addEventListener('wheel', () => observer.disconnect(), { once: true, passive: true });
	addEventListener('touchstart', () => observer.disconnect(), { once: true, passive: true });
}

function scrollToHash(url: URL) {
	if (!url.hash) return false;
	const target = document.getElementById(decodeURIComponent(url.hash.slice(1)));
	target?.scrollIntoView();
	return !!target;
}

/** Like a page load: the next Tab starts at the top, screen readers start reading the new page. */
function resetFocus() {
	const body = document.body;
	const tabindex = body.getAttribute('tabindex');
	body.tabIndex = -1;
	body.focus({ preventScroll: true });
	if (tabindex === null) body.removeAttribute('tabindex');
	else body.setAttribute('tabindex', tabindex);
}

function saveScroll() {
	scrolls[index] = [scrollX, scrollY];
	try {
		sessionStorage.setItem(SCROLL_KEY, JSON.stringify(scrolls));
	} catch {
		// Private mode: positions are kept for this page load only.
	}
}

function onPopState(event: PopStateEvent) {
	const url = new URL(location.href);
	const target = typeof event.state?.index === 'number' ? event.state.index : index + 1;
	// Only the #fragment changed: the same page stays.
	if (url.pathname === page.url.pathname && url.search === page.url.search) {
		page.url = url;
		index = target;
		history.replaceState({ ...history.state, index }, '');
		return;
	}
	saveScroll();
	void navigate(url, 'pop', {}, target);
}

function onClick(event: MouseEvent) {
	if (event.defaultPrevented || event.button !== 0) return;
	if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
	const link = (event.target as Element | null)?.closest?.('a');
	if (!(link instanceof HTMLAnchorElement) || !link.href) return;
	if ((link.target && link.target !== '_self') || link.hasAttribute('download')) return;
	if (link.relList.contains('external')) return;
	const url = new URL(link.href);
	if (url.origin !== location.origin || !find(routes, url.pathname)) return;
	// A jump within the same page: the browser does that.
	if (url.pathname === page.url.pathname && url.search === page.url.search && url.hash) return;
	event.preventDefault();
	void navigate(url, 'push', {});
}
