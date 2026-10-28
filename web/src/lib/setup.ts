// SPDX-License-Identifier: AGPL-3.0-or-later
import type { System } from './api/index.js';

type Step = System['checklist'][number];

/** The required steps still open. */
export function open(checklist: Step[]): Step[] {
	return checklist.filter((s) => !s.done && !s.optional);
}

/** "3 MB", "1.2 GB" (in steps of 1024, like the upload limit in MiB). */
export function bytes(n: number, locale: string): string {
	const units = ['B', 'KB', 'MB', 'GB', 'TB'];
	let value = n;
	let unit = 0;
	while (value >= 1024 && unit < units.length - 1) {
		value /= 1024;
		unit++;
	}
	return `${value.toLocaleString(locale, { maximumFractionDigits: value < 10 && unit ? 1 : 0 })} ${units[unit]}`;
}
