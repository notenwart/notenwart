<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import type { PDFDocumentProxy } from 'pdfjs-dist';
	import { renderPage } from '#lib/pdf.js';
	import { t } from '#lib/i18n/index.svelte.js';

	interface Props {
		document: PDFDocumentProxy;
		number: number;
		selected: boolean;
		/** Position in the selection (1-based), shown on the page. */
		order: number | null;
		/** Labels of existing parts that contain this page. */
		used: string[];
		onclick: (event: MouseEvent) => void;
	}

	let { document, number, selected, order, used, onclick }: Props = $props();
	let canvas: HTMLCanvasElement;

	// Render only once the page scrolls into view; scores can have hundreds of pages.
	$effect(() => {
		let done = false;
		const observer = new IntersectionObserver(
			(entries) => {
				if (!done && entries.some((e) => e.isIntersecting)) {
					done = true;
					observer.disconnect();
					renderPage(document, number, canvas, 140).catch(() => {});
				}
			},
			{ rootMargin: '400px' }
		);
		observer.observe(canvas);
		return () => observer.disconnect();
	});
</script>

<button
	type="button"
	class="page"
	class:selected
	aria-pressed={selected}
	{onclick}
	title={t('Page {n}', { n: number })}
>
	<canvas bind:this={canvas} width="140" height="198"></canvas>
	<span class="number">{number}</span>
	{#if order}<span class="order">{order}</span>{/if}
	{#if used.length}<span class="used">{used.join(', ')}</span>{/if}
</button>

<style>
	.page {
		position: relative;
		padding: 4px;
		background: var(--bg);
		border: 2px solid var(--line);
		border-radius: var(--radius);
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 4px;
	}
	.page.selected {
		border-color: var(--accent);
		background: var(--selected);
	}
	canvas {
		width: 140px;
		height: auto;
		background: #fff;
		display: block;
	}
	.number {
		font-size: 0.8em;
		color: var(--muted);
	}
	.order {
		position: absolute;
		top: 8px;
		right: 8px;
		background: var(--accent);
		color: var(--accent-text);
		border-radius: 999px;
		min-width: 1.6em;
		padding: 0 0.3em;
		font-weight: 700;
	}
	.used {
		max-width: 140px;
		font-size: 0.75em;
		color: var(--muted);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
