// SPDX-License-Identifier: AGPL-3.0-or-later

/** "1–3, 5, 7–8" for [1, 2, 3, 5, 7, 8]; keeps the given order. */
export function formatPages(pages: number[]): string {
	const ranges: string[] = [];
	let start = pages[0];
	let previous = pages[0];
	for (const page of [...pages.slice(1), NaN]) {
		if (page === previous + 1) {
			previous = page;
			continue;
		}
		ranges.push(start === previous ? `${start}` : `${start}–${previous}`);
		start = previous = page;
	}
	return pages.length ? ranges.join(', ') : '';
}

/** All pages from `from` to `to`, in either direction. */
export function range(from: number, to: number): number[] {
	const step = from <= to ? 1 : -1;
	return Array.from({ length: Math.abs(to - from) + 1 }, (_, i) => from + i * step);
}
