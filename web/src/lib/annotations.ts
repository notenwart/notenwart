// SPDX-License-Identifier: AGPL-3.0-or-later
// Geometry of annotations: strokes live in page coordinates (0–1 across and down the page), so they fit any
// screen and zoom. Line widths are fractions of the page width.
import type { Stroke } from './api/index.js';

export type Pages = Record<string, Stroke[]>;
export type Point = [number, number];

/** Markings a leader shares with an ensemble: shown beneath one's own, written on only by its leaders. */
export interface SharedLayer {
	id: string;
	label: string;
	pages: Pages;
	editable: boolean;
}

export interface Pen {
	tool: Stroke['tool'];
	color: string;
}

/** The tools on offer: ink in three colours, and a highlighter. */
export const pens: Pen[] = [
	{ tool: 'pen', color: '#1d2b45' },
	{ tool: 'pen', color: '#c62828' },
	{ tool: 'pen', color: '#1565c0' },
	{ tool: 'highlighter', color: '#f2c200' }
];

export const widths: Record<Stroke['tool'], number> = { pen: 0.0035, highlighter: 0.02 };

const clamp = (v: number) => Math.min(1, Math.max(0, v));
const round = (v: number) => Math.round(v * 10000) / 10000;

/** A point on screen (relative to the page element's box) in page coordinates. */
export function toPage(x: number, y: number, width: number, height: number): Point {
	return [round(clamp(x / width)), round(clamp(y / height))];
}

/** Drops points closer than `minimum` to the previous one (keeps the last), so strokes stay small. */
export function thin(points: Point[], minimum = 0.002): Point[] {
	if (points.length <= 2) return points;
	const kept: Point[] = [points[0]];
	for (const p of points.slice(1, -1)) {
		const last = kept[kept.length - 1];
		if (Math.hypot(p[0] - last[0], p[1] - last[1]) >= minimum) kept.push(p);
	}
	kept.push(points[points.length - 1]);
	return kept;
}

/** An SVG path through the points, scaled to a box of `width` × `height`. */
export function svgPath(points: Point[], width: number, height: number): string {
	if (points.length === 0) return '';
	const [first, ...rest] = points.map(
		([x, y]) => `${(x * width).toFixed(1)} ${(y * height).toFixed(1)}`
	);
	// A single point becomes a dot: a zero-length line with round caps.
	return rest.length ? `M${first} L${rest.join(' L')}` : `M${first} L${first}`;
}

/** Distance from point p to the segment a–b, in page units scaled by the page's aspect ratio. */
function distance(p: Point, a: Point, b: Point, aspect: number): number {
	const [px, py, ax, ay, bx, by] = [p[0], p[1] * aspect, a[0], a[1] * aspect, b[0], b[1] * aspect];
	const dx = bx - ax;
	const dy = by - ay;
	const length = dx * dx + dy * dy;
	const t = length ? Math.max(0, Math.min(1, ((px - ax) * dx + (py - ay) * dy) / length)) : 0;
	return Math.hypot(px - (ax + t * dx), py - (ay + t * dy));
}

/**
 * The index of the topmost stroke within `tolerance` (fraction of the page width) of the point, or -1.
 * `aspect` is the page's height / width, so distances are measured evenly in both directions.
 */
export function hit(strokes: Stroke[], point: Point, aspect: number, tolerance = 0.015): number {
	for (let i = strokes.length - 1; i >= 0; i--) {
		const pts = strokes[i].points as Point[];
		const reach = tolerance + strokes[i].width / 2;
		if (pts.length === 1 && distance(point, pts[0], pts[0], aspect) <= reach) return i;
		for (let j = 1; j < pts.length; j++) {
			if (distance(point, pts[j - 1], pts[j], aspect) <= reach) return i;
		}
	}
	return -1;
}
