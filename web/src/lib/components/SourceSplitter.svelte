<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import type { PDFDocumentProxy } from 'pdfjs-dist';
	import { api, message, type Instrument, type Part, type SourceFile } from '#lib/api/index.js';
	import { guessFromFileName, instrumentOptions, partLabel } from '#lib/instruments.js';
	import { openPdf } from '#lib/pdf.js';
	import { formatPages, range } from '#lib/pages.js';
	import Thumbnail from './Thumbnail.svelte';
	import { plural, t } from '#lib/i18n/index.svelte.js';
	import Select from '#lib/components/Select.svelte';
	import { session } from '#lib/session.svelte.js';
	import { notify } from '#lib/dialog.svelte.js';

	interface Props {
		source: SourceFile;
		instruments: Instrument[];
		/** Parts of the work that come from this source. */
		parts: Part[];
		/** A part to change; its pages and fields are loaded into the form. */
		editing: Part | null;
		onsaved: () => void;
		oncancel: () => void;
	}

	let { source, instruments, parts, editing, onsaved, oncancel }: Props = $props();

	let document = $state<PDFDocumentProxy | null>(null);
	let loadError = $state('');
	let selection = $state<number[]>([]);
	let anchor = $state<number | null>(null);
	let instrumentId = $state('');
	let voice = $state<number | null>(null);
	let label = $state('');
	let labelTouched = $state(false);
	let saving = $state(false);
	let error = $state('');

	// Parts proposed from the page headers of a file with all parts.
	interface Proposal {
		key: number;
		checked: boolean;
		label: string;
		instrumentId: string;
		voice: number | null;
		pages: number[];
	}
	let proposals = $state<Proposal[] | null>(null);
	let proposing = $state(false);
	const canPropose = $derived(
		!!session.status?.recognition.text && source.pageCount > 1 && !editing
	);

	const guess = $derived(guessFromFileName(source.fileName, instruments));
	const instrument = $derived(instruments.find((i) => i.id === instrumentId));
	const usedBy = $derived.by(() => {
		const used: Record<number, string[]> = {};
		for (const part of parts) {
			if (part.id === editing?.id) continue;
			for (const page of part.pages) (used[page] ??= []).push(part.label);
		}
		return used;
	});

	$effect(() => {
		let cancelled = false;
		openPdf(`/api/sources/${source.id}/file`)
			.then((doc) => {
				if (cancelled) doc.loadingTask.destroy();
				else document = doc;
			})
			.catch((e) => (loadError = t('Cannot open the PDF: {reason}', { reason: String(e) })));
		return () => {
			cancelled = true;
			document?.loadingTask.destroy();
		};
	});

	// Load the part being edited, or start with the guess from the file name.
	$effect(() => {
		if (editing) {
			selection = [...editing.pages];
			instrumentId = editing.instrumentId ?? '';
			voice = editing.voice ?? null;
			label = editing.label;
			labelTouched = true;
		} else {
			reset();
		}
	});

	// Follow the instrument and voice until the user writes their own label.
	$effect(() => {
		const suggested = partLabel(instrument, voice);
		if (!labelTouched && suggested) label = suggested;
	});

	function reset() {
		selection = [];
		anchor = null;
		instrumentId = guess?.instrument.id ?? '';
		voice = guess?.voice ?? null;
		labelTouched = false;
		label = partLabel(guess?.instrument, guess?.voice ?? null);
		error = '';
	}

	function toggle(page: number, event: MouseEvent) {
		if (event.shiftKey && anchor !== null) {
			const add = range(anchor, page).filter((p) => !selection.includes(p));
			selection = [...selection, ...add];
		} else if (selection.includes(page)) {
			selection = selection.filter((p) => p !== page);
		} else {
			selection = [...selection, page];
		}
		anchor = page;
	}

	/** "Page 3" or "Pages 1–2, 5". */
	function pagesLabel(pages: number[]) {
		return plural(pages.length, 'Page {pages}', 'Pages {pages}', { pages: formatPages(pages) });
	}

	async function propose() {
		proposing = true;
		error = '';
		const { data, error: failure } = await api.POST('/sources/{sourceId}/split', {
			params: { path: { sourceId: source.id } }
		});
		proposing = false;
		if (!data) {
			error = message(failure);
			return;
		}
		const used = new Set(Object.keys(usedBy).map(Number));
		proposals = data.map((p, key) => {
			const instrument = instruments.find((i) => i.id === p.instrumentId);
			const label = instrument
				? partLabel(instrument, p.voice ?? null)
				: (p.printed ?? (p.score ? t('Full score') : pagesLabel(p.pages)));
			return {
				key,
				// Pages before the first header (cover, notes) and pages already in parts stay unchecked.
				checked: (!!instrument || p.score) && !p.pages.some((page) => used.has(page)),
				label,
				instrumentId: instrument?.id ?? '',
				voice: p.voice ?? null,
				pages: p.pages
			};
		});
		if (!data.some((p) => p.instrumentId || p.score))
			notify(t('No part names found at the top of the pages.'), 'error');
	}

	/** Moves a proposal into the form below for changing its pages. */
	function adjust(proposal: Proposal) {
		selection = [...proposal.pages];
		instrumentId = proposal.instrumentId;
		voice = proposal.voice;
		label = proposal.label;
		labelTouched = true;
		proposals = proposals?.filter((p) => p.key !== proposal.key) ?? null;
	}

	async function createProposed() {
		if (!proposals) return;
		saving = true;
		let created = 0;
		for (const proposal of proposals.filter((p) => p.checked && p.label.trim())) {
			const { error: failure } = await api.POST('/works/{workId}/parts', {
				params: { path: { workId: source.workId } },
				body: {
					label: proposal.label.trim(),
					instrumentId: proposal.instrumentId || null,
					voice: proposal.instrumentId ? proposal.voice || null : null,
					sourceId: source.id,
					pages: proposal.pages
				}
			});
			if (failure) {
				error = `${proposal.label}: ${message(failure)}`;
				break;
			}
			created++;
			proposals = proposals.filter((p) => p.key !== proposal.key);
		}
		saving = false;
		if (created) notify(plural(created, '{n} part created.', '{n} parts created.'));
		if (proposals && !proposals.some((p) => p.checked)) proposals = null;
		onsaved();
	}

	function selectAll() {
		selection = range(1, source.pageCount);
	}

	async function save(event: SubmitEvent) {
		event.preventDefault();
		saving = true;
		error = '';
		const body = {
			label: label.trim(),
			instrumentId: instrumentId || null,
			voice: voice || null,
			sourceId: source.id,
			pages: selection
		};
		const { error: failure } = editing
			? await api.PUT('/parts/{partId}', { params: { path: { partId: editing.id } }, body })
			: await api.POST('/works/{workId}/parts', {
					params: { path: { workId: source.workId } },
					body
				});
		saving = false;
		if (failure) {
			error = message(failure);
			return;
		}
		reset();
		onsaved();
	}
</script>

<section class="splitter">
	<header>
		<h3>{source.fileName}</h3>
		<span class="muted">{plural(source.pageCount, '{n} page', '{n} pages')}</span>
		{#if guess && !editing}
			<span class="muted"
				>· {t('looks like {part}', { part: partLabel(guess.instrument, guess.voice) })}</span
			>
		{/if}
		{#if canPropose && !proposals}
			<button
				class="propose"
				onclick={propose}
				disabled={proposing}
				title={t('Reads the part names at the top of every page and proposes the parts.')}
			>
				{proposing ? t('Reading pages …') : t('Recognise parts')}
			</button>
		{/if}
	</header>

	{#if proposals}
		<div class="proposals">
			<p class="muted">
				{t(
					'Proposed from the page headers – check them, then create. “Adjust” moves one into the form below.'
				)}
			</p>
			<table>
				<tbody>
					{#each proposals as proposal (proposal.key)}
						<tr class:off={!proposal.checked}>
							<td>
								<input
									type="checkbox"
									bind:checked={proposal.checked}
									aria-label={t('Create {name}', { name: proposal.label })}
								/>
							</td>
							<td>
								<input bind:value={proposal.label} maxlength="120" aria-label={t('Label')} />
							</td>
							<td>
								<Select
									bind:value={proposal.instrumentId}
									label={t('Instrument')}
									options={[
										{ value: '', label: t('— none (score, choir …) —') },
										...instrumentOptions(instruments)
									]}
								/>
							</td>
							<td>
								<input
									class="voice-input"
									type="number"
									min="1"
									max="99"
									bind:value={proposal.voice}
									aria-label={t('Part no.')}
								/>
							</td>
							<td class="muted">{pagesLabel(proposal.pages)}</td>
							<td><button class="link" onclick={() => adjust(proposal)}>{t('Adjust')}</button></td>
						</tr>
					{/each}
				</tbody>
			</table>
			<div class="actions">
				<button onclick={() => (proposals = null)}>{t('Discard')}</button>
				<button
					class="primary"
					disabled={saving || !proposals.some((p) => p.checked)}
					onclick={createProposed}
				>
					{plural(proposals.filter((p) => p.checked).length, 'Create {n} part', 'Create {n} parts')}
				</button>
			</div>
		</div>
	{/if}

	<form onsubmit={save}>
		<label>
			{t('Instrument')}
			<Select
				bind:value={instrumentId}
				label={t('Instrument')}
				options={[
					{ value: '', label: t('— none (score, choir …) —') },
					...instrumentOptions(instruments)
				]}
			/>
		</label>
		<label class="voice">
			{t('Part no.')}
			<input type="number" min="1" max="99" bind:value={voice} />
		</label>
		<label class="label">
			{t('Label')}
			<input
				bind:value={label}
				oninput={() => (labelTouched = true)}
				required
				maxlength="120"
				placeholder={t('e.g. Full score')}
			/>
		</label>
		<div class="actions">
			<span class="muted">
				{selection.length
					? pagesLabel(selection)
					: t('Click pages to select them (Shift-click for a range).')}
			</span>
			<button type="button" onclick={selectAll}>{t('All pages')}</button>
			<button type="button" onclick={() => (selection = [])} disabled={!selection.length}
				>{t('Clear')}</button
			>
			{#if editing}<button type="button" onclick={oncancel}>{t('Cancel')}</button>{/if}
			<button class="primary" disabled={saving || !selection.length || !label.trim()}>
				{editing ? t('Save part') : t('Create part')}
			</button>
		</div>
		{#if error}<p class="error">{error}</p>{/if}
	</form>

	{#if loadError}
		<p class="error">{loadError}</p>
	{:else if !document}
		<p class="muted">{t('Loading pages…')}</p>
	{:else}
		<div class="pages">
			{#each range(1, source.pageCount) as number (number)}
				<Thumbnail
					{document}
					{number}
					selected={selection.includes(number)}
					order={selection.includes(number) ? selection.indexOf(number) + 1 : null}
					used={usedBy[number] ?? []}
					onclick={(event) => toggle(number, event)}
				/>
			{/each}
		</div>
	{/if}
</section>

<style>
	.splitter {
		border: 1px solid var(--line);
		border-radius: var(--radius);
		padding: 1rem;
		margin-bottom: 1.5rem;
	}
	header {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 0.5rem;
	}
	.propose {
		margin-left: auto;
	}
	.proposals {
		margin: 0.75rem 0;
		padding: 0.75rem;
		border-radius: var(--radius);
		background: var(--surface);
		border: 1px solid var(--line);
	}
	.proposals p {
		margin: 0 0 0.5rem;
	}
	.proposals table {
		width: 100%;
		border-collapse: collapse;
	}
	.proposals td {
		padding: 0.25rem 0.4rem;
		border-top: 1px solid var(--line);
	}
	.proposals tr.off td:not(:first-child) {
		opacity: 0.5;
	}
	.proposals .voice-input {
		width: 4.5rem;
	}
	.proposals .actions {
		display: flex;
		justify-content: flex-end;
		gap: 0.5rem;
		margin-top: 0.6rem;
	}
	h3 {
		margin: 0;
		font-size: 1.05rem;
		word-break: break-all;
	}
	form {
		background: var(--bg);
		display: flex;
		flex-wrap: wrap;
		gap: 0.75rem;
		align-items: end;
		padding: 0.75rem 0;
		border-bottom: 1px solid var(--line);
		margin-bottom: 0.75rem;
	}
	label {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		font-size: 0.85em;
		color: var(--muted);
	}
	label > :global(*) {
		color: var(--text);
		font-size: 1rem;
	}
	/* On wide screens the form stays visible while scrolling through the pages. */
	@media (min-width: 48rem) {
		form {
			position: sticky;
			top: 0;
			z-index: 1;
		}
	}
	.voice input {
		width: 5rem;
	}
	.label {
		flex: 1 1 12rem;
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.5rem;
		flex: 1 1 100%;
	}
	.actions .muted {
		flex: 1 1 12rem;
	}
	.pages {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(156px, 1fr));
		gap: 0.75rem;
		justify-items: center;
	}
</style>
