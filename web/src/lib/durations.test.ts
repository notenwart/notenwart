// SPDX-License-Identifier: AGPL-3.0-or-later
import { expect, it } from 'vitest';
import { clockAfter, formatDuration, parseDuration } from './durations.js';

it('reads durations as people type them', () => {
	expect(parseDuration('4:30')).toBe(270);
	expect(parseDuration(' 4 ')).toBe(240);
	expect(parseDuration('1:02:03')).toBe(3723);
	expect(parseDuration('')).toBeNull();
	expect(parseDuration('4:75')).toBeNaN();
	expect(parseDuration('vier')).toBeNaN();
});

it('writes durations and times of day', () => {
	expect(formatDuration(270)).toBe('4:30');
	expect(formatDuration(65)).toBe('1:05');
	expect(formatDuration(3723)).toBe('1:02:03');
	expect(formatDuration(null)).toBe('');
	expect(clockAfter('19:30', 900)).toBe('19:45');
	expect(clockAfter('23:50', 1200)).toBe('00:10');
});
