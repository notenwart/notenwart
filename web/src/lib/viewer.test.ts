// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe as group, expect, it } from 'vitest';
import {
	defaultMode,
	describe,
	keyAction,
	layout,
	neighbours,
	readingPage,
	stepCount,
	stepForPage,
	tapAction
} from './viewer.js';

group('single', () => {
	it('shows one page per step', () => {
		expect(stepCount('single', 3)).toBe(3);
		expect(layout('single', 0, 3)).toEqual({ kind: 'single', page: 1 });
		expect(layout('single', 2, 3)).toEqual({ kind: 'single', page: 3 });
		expect(layout('single', 9, 3)).toEqual({ kind: 'single', page: 3 });
	});
});

group('half', () => {
	it('alternates full pages and half turns', () => {
		expect(stepCount('half', 3)).toBe(5);
		expect([0, 1, 2, 3, 4].map((s) => layout('half', s, 3))).toEqual([
			{ kind: 'single', page: 1 },
			{ kind: 'split', top: 2, bottom: 1 },
			{ kind: 'single', page: 2 },
			{ kind: 'split', top: 3, bottom: 2 },
			{ kind: 'single', page: 3 }
		]);
	});

	it('has no half turn for one page', () => {
		expect(stepCount('half', 1)).toBe(1);
	});
});

group('double', () => {
	it('turns by two and leaves the last right page empty for odd counts', () => {
		expect(stepCount('double', 5)).toBe(3);
		expect(layout('double', 0, 5)).toEqual({ kind: 'double', left: 1, right: 2 });
		expect(layout('double', 2, 5)).toEqual({ kind: 'double', left: 5, right: null });
	});
});

it('keeps the reading page when the mode changes', () => {
	const pages = 7;
	for (const page of [1, 4, 7]) {
		for (const mode of ['single', 'half'] as const) {
			expect(readingPage(layout(mode, stepForPage(mode, page, pages), pages))).toBe(page);
		}
	}
	expect(readingPage(layout('double', stepForPage('double', 4, pages), pages))).toBe(3);
	expect(readingPage(layout('half', 3, pages))).toBe(2);
});

it('describes positions', () => {
	expect(describe(layout('single', 2, 8), 8)).toBe('3 / 8');
	expect(describe(layout('half', 5, 8), 8)).toBe('3 → 4 / 8');
	expect(describe(layout('double', 1, 8), 8)).toBe('3–4 / 8');
	expect(describe(layout('double', 1, 3), 3)).toBe('3 / 3');
});

it('renders neighbours ahead', () => {
	expect(neighbours('single', 0, 5).sort()).toEqual([2, 3]);
	expect(neighbours('double', 1, 8).sort()).toEqual([1, 2, 5, 6, 7, 8]);
});

it('maps pedal keys and taps', () => {
	expect(keyAction('PageDown')).toBe('next');
	expect(keyAction('ArrowLeft')).toBe('prev');
	expect(keyAction('a')).toBeNull();
	expect(tapAction(10, 300)).toBe('prev');
	expect(tapAction(250, 300)).toBe('next');
	expect(tapAction(150, 300)).toBe('controls');
});

it('picks two pages only on wide landscape screens', () => {
	expect(defaultMode(1366, 1024)).toBe('double');
	expect(defaultMode(1024, 1366)).toBe('single');
	expect(defaultMode(390, 844)).toBe('single');
});
