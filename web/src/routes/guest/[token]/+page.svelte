<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- What a substitute sees through a guest link: their parts, in the viewer, without an account. -->
<script lang="ts">
	import type { PDFDocumentProxy } from 'pdfjs-dist';
	import { page } from '$app/state';
	import { api, type GuestView } from '#lib/api/index.js';
	import { formatDate } from '#lib/dates.js';
	import { openPdf } from '#lib/pdf.js';
	import { t } from '#lib/i18n/index.svelte.js';
	import Viewer from '#lib/components/Viewer.svelte';

	type GuestPart = GuestView['parts'][number];

	const token = $derived(page.params.token ?? '');
	let view = $state<GuestView | null>(null);
	let missing = $state(false);
	let open = $state<{ part: GuestPart; document: PDFDocumentProxy } | null>(null);
	let loading = $state('');

	$effect(() => {
		api.GET('/guest/{token}', { params: { path: { token } } }).then(({ data }) => {
			if (data) view = data;
			else missing = true;
		});
	});

	async function show(part: GuestPart) {
		loading = part.id;
		try {
			const document = await openPdf(`/api/guest/${token}/parts/${part.id}/file`);
			open = { part, document };
		} finally {
			loading = '';
		}
	}
</script>

{#if open}
	<Viewer
		document={open.document}
		title={open.part.workTitle}
		subtitle={open.part.label}
		onclose={() => {
			open?.document.loadingTask.destroy();
			open = null;
		}}
	/>
{:else if missing}
	<div class="narrow">
		<h1>{t('This link does not work')}</h1>
		<p class="muted">{t('It has expired or was withdrawn. Ask whoever sent it for a new one.')}</p>
	</div>
{:else if view}
	<div class="narrow">
		<p class="muted ensemble">{view.ensemble}</p>
		<h1>{t('Music for {name}', { name: view.name })}</h1>
		<p class="muted">{t('This link works until {date}.', { date: formatDate(view.expiresOn) })}</p>
		<ul class="parts">
			{#each view.parts as part (part.id)}
				<li>
					<button class="link" disabled={loading === part.id} onclick={() => show(part)}>
						<strong>{part.workTitle}</strong>
						<span>{part.label}</span>
					</button>
					<span class="muted">{part.composer ?? ''}</span>
					<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- served by the API, not a route -->
					<a class="download" href="/api/guest/{token}/parts/{part.id}/file" download
						>{t('Download')}</a
					>
				</li>
			{/each}
		</ul>
		<p class="muted small">
			{t('Please use this music only for playing with the ensemble and do not pass the link on.')}
		</p>
	</div>
{:else}
	<p class="muted">{t('Loading…')}</p>
{/if}

<style>
	.ensemble {
		margin: 0;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		font-size: 0.85em;
	}
	h1 {
		margin-top: 0.2rem;
	}
	.parts {
		list-style: none;
		padding: 0;
		margin: 1rem 0;
		border-top: 1px solid var(--line);
	}
	.parts li {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 0.25rem 1rem;
		padding: 0.6rem 0;
		border-bottom: 1px solid var(--line);
	}
	.parts button {
		display: flex;
		gap: 0.6rem;
		color: var(--text);
		text-align: left;
	}
	.download {
		margin-left: auto;
	}
	.small {
		font-size: 0.85em;
	}
</style>
