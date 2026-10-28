// SPDX-License-Identifier: AGPL-3.0-or-later
// Page turning, independent of rendering: which pages a position shows in each mode.

/**
 * - `single`: one page at a time.
 * - `half`: half-page turns – between two pages the top half already shows the next page while the bottom half
 *   still shows the end of the current one, so players can read on while the turn happens.
 * - `double`: two pages side by side (landscape tablets), turning by two.
 */
export type Mode = 'single' | 'half' | 'double';

export type Layout =
	| { kind: 'single'; page: number }
	| { kind: 'split'; top: number; bottom: number }
	| { kind: 'double'; left: number; right: number | null };

/** Number of positions in a part with `pages` pages. */
export function stepCount(mode: Mode, pages: number): number {
	if (pages < 1) return 0;
	if (mode === 'half') return 2 * pages - 1;
	if (mode === 'double') return Math.ceil(pages / 2);
	return pages;
}

export function clampStep(mode: Mode, step: number, pages: number): number {
	return Math.max(0, Math.min(step, stepCount(mode, pages) - 1));
}

/** The pages (1-based) shown at a position. */
export function layout(mode: Mode, step: number, pages: number): Layout {
	const s = clampStep(mode, step, pages);
	if (mode === 'double') {
		const left = 2 * s + 1;
		return { kind: 'double', left, right: left + 1 <= pages ? left + 1 : null };
	}
	if (mode === 'half' && s % 2 === 1) {
		const page = (s + 1) / 2;
		return { kind: 'split', top: page + 1, bottom: page };
	}
	return { kind: 'single', page: mode === 'half' ? s / 2 + 1 : s + 1 };
}

/** The page being read at a position: the one a player continues on after a mode change. */
export function readingPage(view: Layout): number {
	if (view.kind === 'single') return view.page;
	if (view.kind === 'split') return view.bottom;
	return view.left;
}

/** The position in `mode` that shows `page`. */
export function stepForPage(mode: Mode, page: number, pages: number): number {
	const p = Math.max(1, Math.min(page, pages));
	if (mode === 'half') return 2 * (p - 1);
	if (mode === 'double') return Math.floor((p - 1) / 2);
	return p - 1;
}

/** "3 / 8", "3–4 / 8" or "3 → 4 / 8". */
export function describe(view: Layout, pages: number): string {
	if (view.kind === 'single') return `${view.page} / ${pages}`;
	if (view.kind === 'split') return `${view.bottom} → ${view.top} / ${pages}`;
	return `${view.right ? `${view.left}–${view.right}` : view.left} / ${pages}`;
}

/** Pages worth rendering ahead, so turning feels instant. */
export function neighbours(mode: Mode, step: number, pages: number): number[] {
	const wanted = new Set<number>();
	for (const s of [step - 1, step + 1, step + 2]) {
		if (s < 0 || s >= stepCount(mode, pages)) continue;
		const view = layout(mode, s, pages);
		if (view.kind === 'single') wanted.add(view.page);
		else if (view.kind === 'split') [view.top, view.bottom].forEach((p) => wanted.add(p));
		else [view.left, view.right].forEach((p) => p && wanted.add(p));
	}
	return [...wanted];
}

/**
 * Page-turning keys. Bluetooth page turners (pedals) send arrow keys or Page Up/Down depending on their mode;
 * Space and Enter turn forward as on a keyboard.
 */
export function keyAction(key: string): 'next' | 'prev' | null {
	if (['ArrowRight', 'ArrowDown', 'PageDown', ' ', 'Enter'].includes(key)) return 'next';
	if (['ArrowLeft', 'ArrowUp', 'PageUp'].includes(key)) return 'prev';
	return null;
}

/** Tapping the left third turns back, the right third forward, the middle shows or hides the controls. */
export function tapAction(x: number, width: number): 'next' | 'prev' | 'controls' {
	if (x < width / 3) return 'prev';
	if (x > (2 * width) / 3) return 'next';
	return 'controls';
}

/** A suitable default: two pages side by side on wide landscape screens. */
export function defaultMode(width: number, height: number): Mode {
	return width >= 900 && width > height * 1.2 ? 'double' : 'single';
}
