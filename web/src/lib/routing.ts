// SPDX-License-Identifier: AGPL-3.0-or-later
// Route patterns like `/works/[id]`: matching a path and filling one in – pure, unit-tested; the router uses them.

export type Params = Record<string, string>;

/** The parameters if `path` fits `pattern`, otherwise null. Static segments must match exactly. */
export function match(pattern: string, path: string): Params | null {
	const want = segments(pattern);
	const have = segments(path);
	if (want.length !== have.length) return null;
	const params: Params = {};
	for (const [i, segment] of want.entries()) {
		const value = have[i] ?? '';
		const name = /^\[(\w+)\]$/.exec(segment)?.[1];
		if (name) {
			if (!value) return null;
			try {
				params[name] = decodeURIComponent(value);
			} catch {
				return null;
			}
		} else if (segment !== value) return null;
	}
	return params;
}

/** The first pattern that fits `path`, with its parameters. List static routes before ones with parameters. */
export function find<T extends { pattern: string }>(
	routes: readonly T[],
	path: string
): { route: T; params: Params } | null {
	for (const route of routes) {
		const params = match(route.pattern, path);
		if (params) return { route, params };
	}
	return null;
}

/**
 * A link to a route: `resolve('/works/[id]', { id })`. Paths without parameters (also with a query) come back as
 * they are.
 */
export function resolve(pattern: string, params: Params = {}): string {
	return pattern.replace(/\[(\w+)\]/g, (_, name: string) => {
		const value = params[name];
		if (value === undefined) throw new Error(`missing route parameter ${name} for ${pattern}`);
		return encodeURIComponent(value);
	});
}

function segments(path: string) {
	return path.split('/').filter(Boolean);
}
