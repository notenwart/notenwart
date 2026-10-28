<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import type { PDFDocumentProxy } from 'pdfjs-dist';
	import { api, type MyEnsemble, type Recording } from '#lib/api/index.js';
	import Viewer from '#lib/components/Viewer.svelte';
	import AudioPlayer from '#lib/components/AudioPlayer.svelte';
	import { forPart } from '#lib/recordings.js';
	import { getAnnotations, getMeta, getPart, putAnnotations } from '#lib/offline.js';
	import type { Pages, SharedLayer } from '#lib/annotations.js';
	import { notify } from '#lib/dialog.svelte.js';
	import { openPdf } from '#lib/pdf.js';
	import { t } from '#lib/i18n/index.svelte.js';

	interface Loaded {
		document: PDFDocumentProxy;
		title: string;
		subtitle: string;
		offline: boolean;
		/** Known when loaded from the server; recordings need it. */
		workId?: string;
	}

	const partId = $derived(page.params.id ?? '');
	const ensembleId = $derived(page.url.searchParams.get('ensemble'));
	const programId = $derived(page.url.searchParams.get('program'));
	const startAtEnd = $derived(page.url.searchParams.has('end'));
	let loaded = $state<Loaded | null>(null);
	let annotations = $state<Pages>({});

	// Annotations: the device's copy first, then the server's. Edits made offline win and are sent when possible;
	// otherwise the server's version replaces the device's. The last write wins across devices.
	$effect(() => {
		const id = partId;
		annotations = {};
		loadAnnotations(id);
		return () => flush(id);
	});

	async function loadAnnotations(id: string) {
		const local = await getAnnotations(id).catch(() => undefined);
		if (partId === id && local) annotations = local.pages as Pages;
		try {
			const { data } = await api.GET('/parts/{partId}/annotations', {
				params: { path: { partId: id } }
			});
			if (!data || partId !== id) return;
			if (local?.dirty) await push(id, local.pages as Pages);
			else {
				annotations = data.pages as Pages;
				await putAnnotations({
					partId: id,
					pages: data.pages,
					updatedAt: data.updatedAt,
					dirty: false
				});
			}
		} catch {
			// Offline: the device's copy it is.
		}
	}

	let timer: ReturnType<typeof setTimeout> | undefined;
	let pending: { id: string; pages: Pages } | null = null;

	function annotate(pages: Pages) {
		annotations = pages;
		putAnnotations({ partId, pages, updatedAt: null, dirty: true }).catch(() => {});
		pending = { id: partId, pages };
		clearTimeout(timer);
		timer = setTimeout(() => flush(partId), 1500);
	}

	function flush(id: string) {
		clearTimeout(timer);
		if (pending?.id === id) {
			push(id, pending.pages);
			pending = null;
		}
	}

	async function push(id: string, pages: Pages) {
		try {
			const { data } = await api.PUT('/parts/{partId}/annotations', {
				params: { path: { partId: id } },
				body: { pages }
			});
			if (data) {
				await putAnnotations({
					partId: id,
					pages: data.pages,
					updatedAt: data.updatedAt,
					dirty: false
				});
			}
		} catch {
			// Offline: stays dirty on the device and goes up next time.
		}
	}
	let error = $state('');

	// Markings shared by leaders (online only). Leaders' changes go up 1.5 s after the last stroke.
	let layers = $state<SharedLayer[]>([]);
	$effect(() => {
		const id = partId;
		layers = [];
		api
			.GET('/parts/{partId}/shared-annotations', { params: { path: { partId: id } } })
			.then(({ data }) => {
				if (partId !== id || !data) return;
				layers = data.map((l) => ({
					id: l.ensembleId,
					label: l.ensemble,
					pages: l.pages as Pages,
					editable: l.canEdit
				}));
			})
			.catch(() => {});
		return () => flushShared();
	});

	let sharedTimer: ReturnType<typeof setTimeout> | undefined;
	// Not reactive: only what still has to go up.
	let sharedPending: Record<string, { part: string; pages: Pages }> = {};
	function share(layerId: string, pages: Pages) {
		layers = layers.map((l) => (l.id === layerId ? { ...l, pages } : l));
		sharedPending[layerId] = { part: partId, pages };
		clearTimeout(sharedTimer);
		sharedTimer = setTimeout(flushShared, 1500);
	}
	function flushShared() {
		clearTimeout(sharedTimer);
		const pending = sharedPending;
		sharedPending = {};
		for (const [ensembleId, { part, pages }] of Object.entries(pending)) {
			api
				.PUT('/parts/{partId}/shared-annotations/{ensembleId}', {
					params: { path: { partId: part, ensembleId } },
					body: { pages }
				})
				.then(({ error }) => error && notify(t('The shared markings could not be saved.'), 'error'))
				.catch(() => notify(t('The shared markings could not be saved.'), 'error'));
		}
	}

	// Recordings of the work, the part's own practice recordings first (online only).
	let recordings = $state<Recording[]>([]);
	$effect(() => {
		const workId = loaded?.workId;
		const id = partId;
		recordings = [];
		if (!workId) return;
		api
			.GET('/works/{workId}/recordings', { params: { path: { workId } } })
			.then(({ data }) => {
				if (partId === id) recordings = forPart(data ?? [], id);
			})
			.catch(() => {});
	});
	let order = $state<{ id: string; label: string; workId: string; workTitle: string }[]>([]);

	/** The stored copy if it is current (or the network is gone), else the server's. */
	async function load(id: string): Promise<Loaded> {
		const stored = await getPart(id).catch(() => undefined);
		let info: Awaited<ReturnType<typeof fetchInfo>> | 'offline';
		try {
			info = await fetchInfo(id);
		} catch {
			info = 'offline';
		}
		if (info === 'offline') {
			if (!stored) throw new Error(t('You are offline and this part is not saved on this device.'));
			return {
				document: await openPdf(await stored.pdf.arrayBuffer()),
				title: stored.workTitle,
				subtitle: stored.label,
				offline: true
			};
		}
		if (!info) throw new Error(t('This part does not exist or is not shared with you.'));
		const subtitle = [info.label, info.composer].filter(Boolean).join(' · ');
		if (stored && stored.updatedAt === info.updatedAt) {
			return {
				document: await openPdf(await stored.pdf.arrayBuffer()),
				title: info.workTitle,
				subtitle,
				offline: true,
				workId: info.workId
			};
		}
		return {
			document: await openPdf(`/api/parts/${id}/file`),
			title: info.workTitle,
			subtitle,
			offline: false,
			workId: info.workId
		};
	}

	async function fetchInfo(id: string) {
		const { data, response } = await api.GET('/parts/{partId}/info', {
			params: { path: { partId: id } }
		});
		if (response.status === 404) return null;
		return data ?? null;
	}

	$effect(() => {
		const id = partId;
		loaded = null;
		error = '';
		let current: PDFDocumentProxy | null = null;
		load(id)
			.then((result) => {
				current = result.document;
				if (partId === id) loaded = result;
				else result.document.loadingTask.destroy();
			})
			.catch((e) => (error = e instanceof Error ? e.message : String(e)));
		return () => {
			current?.loadingTask.destroy();
		};
	});

	// The parts in order – the programme's when one is given, else the ensemble's – for the previous or next piece.
	$effect(() => {
		const ensemble = ensembleId;
		const program = programId;
		if (!ensemble) return;
		const fromList = (music: MyEnsemble[] | undefined) => {
			const entry = music?.find((e) => e.id === ensemble);
			const all = entry?.works ?? [];
			const playing = entry?.programs.find((p) => p.id === program);
			const works = playing ? playing.workIds.flatMap((id) => all.filter((w) => w.id === id)) : all;
			order = works.flatMap((w) =>
				w.parts.map((p) => ({ id: p.id, label: p.label, workId: w.id, workTitle: w.title }))
			);
		};
		api
			.GET('/my/music')
			.then(({ data }) => fromList(data))
			.catch(() => getMeta<MyEnsemble[]>('music').then(fromList));
	});

	const index = $derived(order.findIndex((p) => p.id === partId));
	function neighbour(i: number, atEnd: boolean) {
		const target = order[i];
		if (!target || index < 0) return null;
		// Within the same work the part names it; otherwise the work does.
		const sameWork = target.workId === order[index].workId;
		return {
			label: sameWork ? target.label : target.workTitle,
			go: () => {
				const query =
					`?ensemble=${encodeURIComponent(ensembleId ?? '')}` +
					(programId ? `&program=${encodeURIComponent(programId)}` : '') +
					(atEnd ? '&end' : '');
				// eslint-disable-next-line svelte/no-navigation-without-resolve -- resolved path plus a query
				goto(resolve('/view/[id]', { id: target.id }) + query, { replaceState: true });
			}
		};
	}

	function close() {
		if (document.fullscreenElement) document.exitFullscreen().catch(() => {});
		if (history.length > 1) history.back();
		else goto(resolve('/'));
	}
</script>

<svelte:head>
	<title>{loaded ? `${loaded.title} – notenwart` : 'notenwart'}</title>
</svelte:head>

{#if loaded}
	{#key loaded}
		<Viewer
			document={loaded.document}
			title={loaded.title}
			subtitle={loaded.subtitle}
			{startAtEnd}
			previous={neighbour(index - 1, true)}
			next={neighbour(index + 1, false)}
			{annotations}
			onannotate={annotate}
			{layers}
			onshare={share}
			onprint={(shared) => {
				// Saved strokes first, so the PDF has the latest.
				flush(partId);
				flushShared();
				setTimeout(
					() =>
						window.open(`/api/parts/${partId}/annotated?shared=${shared}`, '_blank', 'noopener'),
					400
				);
			}}
			onclose={close}
		>
			{#snippet below()}
				{#if recordings.length}<AudioPlayer {recordings} dark />{/if}
			{/snippet}
		</Viewer>
	{/key}
{:else if error}
	<p class="error">{error}</p>
	<p><a href={resolve('/')}>{t('Back to my music')}</a></p>
{:else}
	<p class="muted">{t('Opening…')}</p>
{/if}
