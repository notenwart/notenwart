// SPDX-License-Identifier: AGPL-3.0-or-later
import { expect, it } from 'vitest';
import type { Stroke } from './api/index.js';
import { hit, svgPath, thin, toPage, type Point } from './annotations.js';

it('maps screen points to the page and keeps them on it', () => {
	expect(toPage(50, 150, 200, 300)).toEqual([0.25, 0.5]);
	expect(toPage(-10, 400, 200, 300)).toEqual([0, 1]);
});

it('thins points that are too close but keeps both ends', () => {
	const points: Point[] = [
		[0, 0],
		[0.0005, 0],
		[0.001, 0],
		[0.01, 0],
		[0.0101, 0]
	];
	expect(thin(points)).toEqual([
		[0, 0],
		[0.01, 0],
		[0.0101, 0]
	]);
});

it('draws paths in the box, a single point as a dot', () => {
	expect(svgPath([[0.5, 0.5]], 100, 200)).toBe('M50.0 100.0 L50.0 100.0');
	expect(
		svgPath(
			[
				[0, 0],
				[1, 1]
			],
			100,
			200
		)
	).toBe('M0.0 0.0 L100.0 200.0');
});

it('finds the topmost stroke near a point', () => {
	const stroke = (points: Point[]): Stroke => ({
		tool: 'pen',
		color: '#000000',
		width: 0.004,
		points
	});
	const strokes = [
		stroke([
			[0.1, 0.1],
			[0.5, 0.1]
		]),
		stroke([
			[0.3, 0.05],
			[0.3, 0.15]
		])
	];
	expect(hit(strokes, [0.3, 0.1], 1)).toBe(1);
	expect(hit(strokes, [0.45, 0.105], 1)).toBe(0);
	expect(hit(strokes, [0.9, 0.9], 1)).toBe(-1);
});
