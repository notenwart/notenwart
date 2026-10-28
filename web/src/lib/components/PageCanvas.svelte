<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import type { Stroke } from '#lib/api/index.js';
	import { hit, svgPath, thin, toPage, widths, type Pen, type Point } from '#lib/annotations.js';

	interface Props {
		image: { bitmap: ImageBitmap; cssWidth: number; cssHeight: number } | null;
		/** Show only the top half (the next page during a half-page turn). */
		topHalf?: boolean;
		/** This page's annotations. */
		strokes?: Stroke[];
		/** Strokes shown beneath, not to be erased here (shared markings, or one's own while editing shared ones). */
		under?: Stroke[];
		/** The active tool while annotating; null turns pages instead. */
		tool?: Pen | 'eraser' | null;
		onstroke?: (stroke: Stroke) => void;
		onerase?: (index: number) => void;
	}

	let {
		image,
		topHalf = false,
		strokes = [],
		under = [],
		tool = null,
		onstroke,
		onerase
	}: Props = $props();
	let canvas: HTMLCanvasElement;
	let current = $state<Point[] | null>(null);

	const width = $derived(image?.cssWidth ?? 0);
	const height = $derived(image?.cssHeight ?? 0);

	$effect(() => {
		if (!image || !canvas) return;
		canvas.width = image.bitmap.width;
		canvas.height = image.bitmap.height;
		canvas.getContext('2d')?.drawImage(image.bitmap, 0, 0);
	});

	let surface = $state<HTMLDivElement>();

	/** In page coordinates; coalesced events have no target of their own, so measure the drawing surface. */
	function point(event: PointerEvent): Point {
		const rect = surface!.getBoundingClientRect();
		return toPage(event.clientX - rect.left, event.clientY - rect.top, rect.width, rect.height);
	}

	function down(event: PointerEvent) {
		event.stopPropagation();
		if (!tool) return;
		if (tool === 'eraser') {
			const index = hit(strokes, point(event), height / width);
			if (index >= 0) onerase?.(index);
			return;
		}
		surface?.setPointerCapture(event.pointerId);
		current = [point(event)];
	}

	function move(event: PointerEvent) {
		if (!current) return;
		// Coalesced events keep fast pen strokes smooth; some browsers return none at all.
		const coalesced = event.getCoalescedEvents?.() ?? [];
		const events = coalesced.length ? coalesced : [event];
		current = [...current, ...events.map((e) => point(e))];
	}

	function up(event: PointerEvent) {
		event.stopPropagation();
		if (!current || !tool || tool === 'eraser') return;
		const points = thin(current);
		current = null;
		onstroke?.({ tool: tool.tool, color: tool.color, width: widths[tool.tool], points });
	}
</script>

<div
	class="page"
	class:top-half={topHalf}
	style:width={image ? `${width}px` : '0'}
	style:height={image ? `${height}px` : '0'}
>
	<canvas bind:this={canvas} aria-hidden="true"></canvas>
	{#if image && (strokes.length || under.length || current)}
		<svg viewBox="0 0 {width} {height}" aria-hidden="true">
			{#each under as stroke, i (i)}
				<path
					class="under"
					class:highlighter={stroke.tool === 'highlighter'}
					d={svgPath(stroke.points as Point[], width, height)}
					stroke={stroke.color}
					stroke-width={stroke.width * width}
				/>
			{/each}
			{#each strokes as stroke, i (i)}
				<path
					class:highlighter={stroke.tool === 'highlighter'}
					d={svgPath(stroke.points as Point[], width, height)}
					stroke={stroke.color}
					stroke-width={stroke.width * width}
				/>
			{/each}
			{#if current && tool && tool !== 'eraser'}
				<path
					class:highlighter={tool.tool === 'highlighter'}
					d={svgPath(current, width, height)}
					stroke={tool.color}
					stroke-width={widths[tool.tool] * width}
				/>
			{/if}
		</svg>
	{/if}
	{#if tool && !topHalf}
		<div
			class="draw"
			class:erasing={tool === 'eraser'}
			bind:this={surface}
			role="presentation"
			onpointerdown={down}
			onpointermove={move}
			onpointerup={up}
			onpointercancel={() => (current = null)}
		></div>
	{/if}
</div>

<style>
	.page {
		position: relative;
		background: #fff;
		box-shadow: 0 1px 6px rgb(0 0 0 / 0.25);
	}
	canvas,
	svg,
	.draw {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
	}
	svg {
		pointer-events: none;
	}
	path {
		fill: none;
		stroke-linecap: round;
		stroke-linejoin: round;
	}
	path.highlighter {
		opacity: 0.35;
		mix-blend-mode: multiply;
	}
	.draw {
		cursor: crosshair;
		touch-action: none;
	}
	.draw.erasing {
		cursor: cell;
	}
	.top-half {
		position: absolute;
		top: 0;
		left: 0;
		clip-path: inset(0 0 50% 0);
		box-shadow: none;
	}
</style>
