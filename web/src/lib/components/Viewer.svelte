<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import { untrack } from 'svelte';
	import type { PDFDocumentProxy } from 'pdfjs-dist';
	import { renderFitted } from '#lib/pdf.js';
	import {
		clampStep,
		defaultMode,
		describe,
		keyAction,
		layout,
		neighbours,
		readingPage,
		stepCount,
		stepForPage,
		tapAction,
		type Mode
	} from '#lib/viewer.js';
	import PageCanvas from './PageCanvas.svelte';
	import { t } from '#lib/i18n/index.svelte.js';
	import type { Stroke } from '#lib/api/index.js';
	import { pens, type Pages, type Pen, type SharedLayer } from '#lib/annotations.js';

	interface Neighbour {
		label: string;
		go: () => void;
	}

	interface Props {
		document: PDFDocumentProxy;
		title: string;
		subtitle: string;
		/** Start at the last page (when coming back from the next piece). */
		startAtEnd?: boolean;
		previous?: Neighbour | null;
		next?: Neighbour | null;
		/** The user's annotations on this part, by page. */
		annotations?: Pages;
		/** Called with all annotations after every change. */
		onannotate?: (pages: Pages) => void;
		/** Markings shared by leaders, one layer per ensemble; shown beneath the user's own. */
		layers?: SharedLayer[];
		/** Called with a shared layer's pages after a leader changed them. */
		onshare?: (id: string, pages: Pages) => void;
		/** Opens the part with annotations drawn in, for printing; `shared` says whether the leaders' are shown. */
		onprint?: (shared: boolean) => void;
		/** Shown above the page navigation, e.g. the recordings' player. */
		below?: Snippet;
		onclose: () => void;
	}

	let {
		document,
		title,
		subtitle,
		startAtEnd = false,
		previous = null,
		next = null,
		annotations = {},
		onannotate,
		layers = [],
		onshare,
		onprint,
		below,
		onclose
	}: Props = $props();

	// Annotating: a pen, the highlighter or the eraser; null turns pages. Undo keeps earlier versions.
	let tool = $state<Pen | 'eraser' | null>(null);
	// Which layer is written on: the user's own ('own') or a shared layer the user leads.
	let target = $state('own');
	let showShared = $state(true);
	let history = $state<{ target: string; pages: Pages }[]>([]);

	const editable = $derived(layers.filter((l) => l.editable));
	const sharedLayer = $derived(layers.find((l) => l.id === target));
	/** The pages being written on. */
	const active = $derived(sharedLayer ? sharedLayer.pages : annotations);
	const hasShared = $derived(layers.some((l) => Object.keys(l.pages).length));

	/** What lies beneath on a page: the other shared layers, and one's own while writing on a shared one. */
	function under(page: number): Stroke[] {
		const shared = showShared
			? layers.filter((l) => l.id !== target).flatMap((l) => l.pages[page] ?? [])
			: [];
		return sharedLayer ? [...shared, ...(annotations[page] ?? [])] : shared;
	}

	function write(to: string, pages: Pages) {
		if (to === 'own') onannotate?.(pages);
		else onshare?.(to, pages);
	}
	function change(page: number, strokes: Stroke[]) {
		history = [...history.slice(-49), { target, pages: active }];
		write(target, { ...active, [page]: strokes });
	}
	function addStroke(page: number, stroke: Stroke) {
		change(page, [...(active[page] ?? []), stroke]);
	}
	function erase(page: number, index: number) {
		change(
			page,
			(active[page] ?? []).filter((_, i) => i !== index)
		);
	}
	function undo() {
		const previous = history.at(-1);
		if (!previous) return;
		history = history.slice(0, -1);
		write(previous.target, previous.pages);
	}
	function annotate(on: boolean) {
		tool = on ? pens[0] : null;
		showControls(true);
	}

	type Image = { bitmap: ImageBitmap; cssWidth: number; cssHeight: number };
	const pages = $derived(document.numPages);
	const MODE_KEY = 'notenwart.viewer.mode';

	function storedMode(): Mode | null {
		try {
			const value = localStorage.getItem(MODE_KEY);
			return value === 'single' || value === 'half' || value === 'double' ? value : null;
		} catch {
			return null;
		}
	}

	let mode = $state<Mode>(storedMode() ?? defaultMode(window.innerWidth, window.innerHeight));
	let step = $state(0);
	let width = $state(0);
	let height = $state(0);
	let controls = $state(true);
	let images = $state<Record<string, Image>>({});
	let stage: HTMLDivElement;

	const view = $derived(layout(mode, step, pages));
	// No writing on half pages: the strokes would land on the wrong page.
	const drawable = $derived(tool && view.kind !== 'split' ? tool : null);
	const box = $derived(
		mode === 'double'
			? { w: Math.max(1, width / 2 - 12), h: Math.max(1, height - 16) }
			: { w: Math.max(1, width - 16), h: Math.max(1, height - 16) }
	);

	$effect(() => {
		if (startAtEnd) untrack(() => (step = stepCount(mode, pages) - 1));
	});

	// Rendered pages, by page and box size; older entries are dropped. Not reactive on purpose: `images` is.
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	const cache = new Map<string, Promise<Image>>();
	function image(page: number): Promise<Image> {
		const key = `${page}@${Math.round(box.w)}x${Math.round(box.h)}`;
		let entry = cache.get(key);
		if (!entry) {
			entry = renderFitted(document, page, box.w, box.h);
			cache.set(key, entry);
			while (cache.size > 12) {
				const oldest = cache.keys().next().value!;
				cache
					.get(oldest)
					?.then((i) => i.bitmap.close())
					.catch(() => {});
				cache.delete(oldest);
				const { [oldest]: _dropped, ...rest } = images;
				void _dropped;
				images = rest;
			}
		}
		entry.then((img) => (images = { ...images, [key]: img })).catch(() => {});
		return entry;
	}
	function shown(page: number | null): Image | null {
		if (!page) return null;
		return images[`${page}@${Math.round(box.w)}x${Math.round(box.h)}`] ?? null;
	}

	// Render what is visible, then what comes next.
	$effect(() => {
		if (!width || !height) return;
		const v = view;
		const visible =
			v.kind === 'single'
				? [v.page]
				: v.kind === 'split'
					? [v.bottom, v.top]
					: [v.left, v.right].filter((p) => p !== null);
		Promise.all(visible.map((p) => image(p!))).then(() => {
			for (const p of neighbours(mode, step, pages)) image(p);
		});
	});

	function turn(direction: 'next' | 'prev') {
		const last = stepCount(mode, pages) - 1;
		if (direction === 'next') {
			if (step < last) step++;
			else next?.go();
		} else if (step > 0) step--;
		else previous?.go();
	}

	function setMode(newMode: Mode) {
		const page = readingPage(view);
		mode = newMode;
		step = clampStep(newMode, stepForPage(newMode, page, pages), pages);
		try {
			localStorage.setItem(MODE_KEY, newMode);
		} catch {
			// Private mode: the choice just isn't remembered.
		}
	}

	// Controls hide after a while; tapping the middle brings them back.
	let hideTimer: ReturnType<typeof setTimeout> | undefined;
	function showControls(on: boolean) {
		controls = on;
		clearTimeout(hideTimer);
		// While annotating, the tools stay.
		if (on && !tool) hideTimer = setTimeout(() => (controls = false), 4000);
	}
	$effect(() => {
		untrack(() => showControls(true));
		return () => clearTimeout(hideTimer);
	});

	function onkeydown(event: KeyboardEvent) {
		if (event.target instanceof HTMLInputElement || event.target instanceof HTMLSelectElement)
			return;
		if (event.key === 'Escape' && !window.document.fullscreenElement) return onclose();
		const action = keyAction(event.key);
		if (!action) return;
		event.preventDefault();
		turn(action);
	}

	let start: { x: number; y: number } | null = null;
	function onpointerdown(event: PointerEvent) {
		start = { x: event.clientX, y: event.clientY };
	}
	function onpointerup(event: PointerEvent) {
		if (!start || tool) return;
		const dx = event.clientX - start.x;
		const dy = event.clientY - start.y;
		start = null;
		if (Math.abs(dx) > 40 && Math.abs(dx) > Math.abs(dy)) return turn(dx < 0 ? 'next' : 'prev');
		if (Math.abs(dx) > 10 || Math.abs(dy) > 10) return;
		const rect = stage.getBoundingClientRect();
		const action = tapAction(event.clientX - rect.left, rect.width);
		if (action === 'controls') showControls(!controls);
		else turn(action);
	}

	// Keep the screen on while playing.
	$effect(() => {
		let lock: WakeLockSentinel | null = null;
		const request = () =>
			navigator.wakeLock
				?.request('screen')
				.then((l) => (lock = l))
				.catch(() => {});
		request();
		const again = () => window.document.visibilityState === 'visible' && request();
		window.document.addEventListener('visibilitychange', again);
		return () => {
			window.document.removeEventListener('visibilitychange', again);
			lock?.release().catch(() => {});
		};
	});

	let fullscreen = $state(false);
	const canFullscreen = typeof window.document.documentElement.requestFullscreen === 'function';
	$effect(() => {
		const update = () => (fullscreen = !!window.document.fullscreenElement);
		window.document.addEventListener('fullscreenchange', update);
		return () => window.document.removeEventListener('fullscreenchange', update);
	});
	function toggleFullscreen() {
		if (window.document.fullscreenElement) window.document.exitFullscreen();
		else window.document.documentElement.requestFullscreen().catch(() => {});
	}
</script>

<svelte:window {onkeydown} />

<div class="viewer">
	<div
		class="stage"
		class:double={view.kind === 'double'}
		bind:this={stage}
		bind:clientWidth={width}
		bind:clientHeight={height}
		{onpointerdown}
		{onpointerup}
		role="application"
		aria-label={t('Sheet music: tap or press the arrow keys to turn pages')}
	>
		{#if view.kind === 'single'}
			<PageCanvas
				image={shown(view.page)}
				strokes={active[view.page]}
				under={under(view.page)}
				tool={drawable}
				onstroke={(s) => addStroke(view.page, s)}
				onerase={(i) => erase(view.page, i)}
			/>
		{:else if view.kind === 'split'}
			<div class="split">
				<PageCanvas
					image={shown(view.bottom)}
					strokes={active[view.bottom]}
					under={under(view.bottom)}
				/>
				<PageCanvas
					image={shown(view.top)}
					strokes={active[view.top]}
					under={under(view.top)}
					topHalf
				/>
				<div class="divider" aria-hidden="true"></div>
			</div>
		{:else}
			{@const left = view.left}
			{@const right = view.right}
			<PageCanvas
				image={shown(left)}
				strokes={active[left]}
				under={under(left)}
				tool={drawable}
				onstroke={(s) => addStroke(left, s)}
				onerase={(i) => erase(left, i)}
			/>
			{#if right}
				<PageCanvas
					image={shown(right)}
					strokes={active[right]}
					under={under(right)}
					tool={drawable}
					onstroke={(s) => addStroke(right, s)}
					onerase={(i) => erase(right, i)}
				/>
			{/if}
		{/if}
	</div>

	<div class="bar top" class:hidden={!controls}>
		<button onclick={onclose} aria-label={t('Close')}>←</button>
		<div class="title">
			<strong>{title}</strong>
			<span>{subtitle}</span>
		</div>
		<div class="modes" role="group" aria-label={t('Page layout')}>
			<button aria-pressed={mode === 'single'} onclick={() => setMode('single')}
				>{t('1 page')}</button
			>
			<button aria-pressed={mode === 'half'} onclick={() => setMode('half')}
				>{t('Half turns')}</button
			>
			<button aria-pressed={mode === 'double'} onclick={() => setMode('double')}
				>{t('2 pages')}</button
			>
		</div>
		{#if hasShared}
			<button
				aria-pressed={showShared}
				title={t('Markings shared by the leaders')}
				onclick={() => (showShared = !showShared)}>{t('Leaders’ markings')}</button
			>
		{/if}
		{#if onannotate}
			<button aria-pressed={!!tool} onclick={() => annotate(!tool)}>✎ {t('Annotate')}</button>
		{/if}
		{#if onprint}
			<button
				onclick={() => onprint(showShared)}
				title={t('The part with the annotations shown, as a PDF to print or keep')}
				>{t('Print / PDF')}</button
			>
		{/if}
		{#if canFullscreen}
			<button onclick={toggleFullscreen}
				>{fullscreen ? t('Exit full screen') : t('Full screen')}</button
			>
		{/if}
	</div>

	{#if tool}
		<div class="bar tools" role="toolbar" aria-label={t('Annotation tools')}>
			{#each pens as pen (pen.tool + pen.color)}
				<button
					class="pen"
					class:highlighter={pen.tool === 'highlighter'}
					aria-pressed={tool !== 'eraser' && tool.color === pen.color}
					aria-label={pen.tool === 'highlighter' ? t('Highlighter') : t('Pen')}
					onclick={() => (tool = pen)}><span style:background={pen.color}></span></button
				>
			{/each}
			<button aria-pressed={tool === 'eraser'} onclick={() => (tool = 'eraser')}
				>{t('Eraser')}</button
			>
			<button onclick={undo} disabled={!history.length}>{t('Undo')}</button>
			{#if editable.length}
				<span class="layers" role="group" aria-label={t('Write for')}>
					<button aria-pressed={target === 'own'} onclick={() => (target = 'own')}
						>{t('For me')}</button
					>
					{#each editable as layer (layer.id)}
						<button aria-pressed={target === layer.id} onclick={() => (target = layer.id)}
							>{t('For everyone in {ensemble}', { ensemble: layer.label })}</button
						>
					{/each}
				</span>
			{/if}
			{#if view.kind === 'split'}<span class="hint">{t('Turn to a whole page to write.')}</span
				>{/if}
			<button class="done" onclick={() => annotate(false)}>{t('Done')}</button>
		</div>
	{/if}

	{#if below && !tool}
		<div class="below" class:lifted={controls}>{@render below()}</div>
	{/if}

	<div class="bar bottom" class:hidden={!controls || !!tool}>
		<button onclick={() => previous?.go()} disabled={!previous}>
			{previous ? `← ${previous.label}` : ''}
		</button>
		<span class="position">{describe(view, pages)}</span>
		<button onclick={() => next?.go()} disabled={!next}>{next ? `${next.label} →` : ''}</button>
	</div>
</div>

<style>
	.viewer {
		position: fixed;
		inset: 0;
		z-index: 10;
		background: #2a2f38;
		user-select: none;
		-webkit-user-select: none;
		touch-action: pan-y pinch-zoom;
	}
	.stage {
		position: absolute;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 8px;
		padding: 8px;
		box-sizing: border-box;
		overflow: hidden;
	}
	.split {
		position: relative;
	}
	/* Where the next page's top half ends and the current page's bottom half begins. */
	.divider {
		position: absolute;
		left: 0;
		right: 0;
		top: calc(50% - 1px);
		border-top: 2px dashed rgb(184 134 46 / 0.85);
		pointer-events: none;
	}
	.bar {
		position: absolute;
		left: 0;
		right: 0;
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 0.5rem 0.75rem;
		background: rgb(20 24 28 / 0.85);
		color: #fff;
		transition:
			opacity 0.2s,
			transform 0.2s;
	}
	.bar.top {
		top: 0;
		flex-wrap: wrap;
	}
	.bar.bottom {
		bottom: 0;
		justify-content: space-between;
	}
	.below {
		position: absolute;
		left: 0.5rem;
		right: 0.5rem;
		bottom: 0.5rem;
		transition: bottom 0.2s;
	}
	.below.lifted {
		bottom: 3.4rem;
	}
	.bar.hidden {
		opacity: 0;
		pointer-events: none;
	}
	.bar.top.hidden {
		transform: translateY(-100%);
	}
	.bar.bottom.hidden {
		transform: translateY(100%);
	}
	.bar button {
		background: rgb(255 255 255 / 0.12);
		border-color: rgb(255 255 255 / 0.2);
		color: #fff;
	}
	.bar button[aria-pressed='true'] {
		background: #d9a441;
		border-color: #d9a441;
		color: #121a2b;
	}
	.bar button:disabled {
		visibility: hidden;
	}
	.title {
		display: flex;
		flex-direction: column;
		flex: 1 1 10rem;
		min-width: 0;
	}
	.title span {
		font-size: 0.85em;
		opacity: 0.8;
	}
	.title strong,
	.title span {
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.modes {
		display: flex;
		gap: 0.25rem;
	}
	.bar.tools {
		bottom: 0;
		flex-wrap: wrap;
	}
	.layers {
		display: inline-flex;
		gap: 0.25rem;
		margin-left: 0.5rem;
	}
	.bar.tools .done {
		margin-left: auto;
	}
	.bar.tools .hint {
		opacity: 0.8;
		font-size: 0.9em;
	}
	.pen span {
		display: block;
		width: 1.1rem;
		height: 1.1rem;
		border-radius: 50%;
		border: 2px solid rgb(255 255 255 / 0.7);
	}
	.pen.highlighter span {
		border-radius: 3px;
		width: 1.4rem;
		opacity: 0.8;
	}
	.bar button:disabled.done,
	.bar.tools button:disabled {
		visibility: visible;
		opacity: 0.4;
	}
	.position {
		font-variant-numeric: tabular-nums;
	}
</style>
