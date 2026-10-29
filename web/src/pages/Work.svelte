<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import { goto, page, resolve } from '#lib/router.svelte.js';
	import {
		api,
		message,
		uploadSource,
		type Borrower,
		type Instrument,
		type Loan,
		type Part,
		type Recognition,
		type WorkDetail
	} from '#lib/api/index.js';
	import { formatPages } from '#lib/pages.js';
	import { workInput } from '#lib/works.js';
	import { formatDate } from '#lib/dates.js';
	import { overdue, returnLoan } from '#lib/loans.js';
	import LoanForm from '#lib/components/LoanForm.svelte';
	import Recordings from '#lib/components/Recordings.svelte';
	import SourceSplitter from '#lib/components/SourceSplitter.svelte';
	import WorkForm from '#lib/components/WorkForm.svelte';
	import { isStaff, session } from '#lib/session.svelte.js';
	import { plural, t } from '#lib/i18n/index.svelte.js';
	import { confirm, notify } from '#lib/dialog.svelte.js';

	interface Upload {
		name: string;
		progress: number;
		error: string;
	}

	const workId = $derived(page.params.id ?? '');
	let work = $state<WorkDetail | null>(null);
	let instruments = $state<Instrument[]>([]);
	let error = $state('');
	let uploads = $state<Upload[]>([]);
	let dragging = $state(false);
	let showDetails = $state(false);
	let editing = $state<Part | null>(null);

	// Details read from the first page of a file, offered field by field.
	type Detail = 'title' | 'composer' | 'arranger' | 'lyricist' | 'publisher';
	const details: Detail[] = ['title', 'composer', 'arranger', 'lyricist', 'publisher'];
	let found = $state<{ field: Detail; value: string; take: boolean }[] | null>(null);
	let reading = $state(false);
	const canRead = $derived(
		!!session.status &&
			(session.status.recognition.text ||
				session.status.recognition.ocr ||
				session.status.recognition.ai)
	);
	const detailNames = $derived<Record<Detail, string>>({
		title: t('Title'),
		composer: t('Composer'),
		arranger: t('Arranger'),
		lyricist: t('Lyricist'),
		publisher: t('Publisher')
	});

	async function readDetails(sourceId: string, quiet = false) {
		if (!work) return;
		reading = true;
		const { data } = await api.POST('/sources/{sourceId}/recognize', {
			params: { path: { sourceId } }
		});
		reading = false;
		const current = work;
		const offers = details
			.map((field) => ({
				field,
				value: (data?.[field as keyof Recognition] as string | null) ?? ''
			}))
			.filter(({ field, value }) => value && value !== (current[field] ?? ''))
			// Empty fields are filled; existing ones only if someone ticks them.
			.map((offer) => ({ ...offer, take: !current[offer.field] }));
		if (offers.length) found = offers;
		else if (!quiet) notify(t('The first page says nothing new about this work.'));
	}

	async function takeDetails() {
		if (!work || !found) return;
		const changed = Object.fromEntries(found.filter((f) => f.take).map((f) => [f.field, f.value]));
		const { data, error: failure } = await api.PUT('/works/{workId}', {
			params: { path: { workId } },
			body: workInput(work, changed)
		});
		if (failure) return notify(message(failure), 'error');
		if (data) Object.assign(work, data);
		found = null;
		notify(t('Saved.'));
	}

	async function load() {
		const { data, error: failure } = await api.GET('/works/{workId}', {
			params: { path: { workId } }
		});
		if (data) work = data;
		else error = message(failure);
	}

	$effect(() => {
		void workId;
		load();
	});

	$effect(() => {
		api.GET('/instruments').then(({ data }) => (instruments = data ?? []));
	});

	// The paper archive (staff only): loans of this work and people to lend to.
	let loans = $state<Loan[]>([]);
	let borrowers = $state<Borrower[]>([]);
	async function loadLoans() {
		const { data } = await api.GET('/works/{workId}/loans', { params: { path: { workId } } });
		loans = data ?? [];
	}
	$effect(() => {
		if (!isStaff(session.me)) return;
		void workId;
		loadLoans();
		api.GET('/borrowers').then(({ data }) => (borrowers = data ?? []));
	});
	const lendable = $derived([t('Complete set'), ...(work?.parts.map((p) => p.label) ?? [])]);
	async function giveBack(loan: Loan) {
		const { error: failure } = await returnLoan(loan);
		if (failure) notify(message(failure), 'error');
		await loadLoans();
	}

	async function upload(files: FileList | File[]) {
		const pdfs = [...files].filter(
			(f) => f.type === 'application/pdf' || f.name.toLowerCase().endsWith('.pdf')
		);
		const entries = pdfs.map((f) => ({ name: f.name, progress: 0, error: '' }));
		uploads = [...uploads, ...entries];
		// One after another: large scans would otherwise compete for the same connection.
		for (const [i, file] of pdfs.entries()) {
			const entry = uploads[uploads.length - pdfs.length + i];
			try {
				const source = await uploadSource(workId, file, (p) => (entry.progress = p));
				uploads = uploads.filter((u) => u !== entry);
				// The first file of a work without details: see what its first page says.
				if (canRead && !found && work && !work.composer && !work.arranger && !work.publisher)
					readDetails(source.id, true);
			} catch (e) {
				entry.error = e instanceof Error ? e.message : String(e);
			}
			await load();
		}
	}

	function drop(event: DragEvent) {
		event.preventDefault();
		dragging = false;
		if (event.dataTransfer?.files.length) upload(event.dataTransfer.files);
	}

	async function deleteSource(id: string, name: string, partCount: number) {
		const question = partCount
			? plural(
					partCount,
					'Delete “{name}” and its part? This cannot be undone.',
					'Delete “{name}” and its {n} parts? This cannot be undone.',
					{ name }
				)
			: t('Delete “{name}”? This cannot be undone.', { name });
		if (!(await confirm({ title: question, confirm: t('Delete'), danger: true }))) return;
		const { error: failure } = await api.DELETE('/sources/{sourceId}', {
			params: { path: { sourceId: id } }
		});
		if (failure) error = message(failure);
		await load();
	}

	async function deletePart(part: Part) {
		if (
			!(await confirm({
				title: t('Delete the part “{name}”?', { name: part.label }),
				confirm: t('Delete'),
				danger: true
			}))
		)
			return;
		const { error: failure } = await api.DELETE('/parts/{partId}', {
			params: { path: { partId: part.id } }
		});
		if (failure) error = message(failure);
		if (editing?.id === part.id) editing = null;
		await load();
	}

	async function deleteWork() {
		if (
			!work ||
			!(await confirm({
				title: t('Delete “{name}” with all files and parts? This cannot be undone.', {
					name: work.title
				}),
				confirm: t('Delete'),
				danger: true
			}))
		)
			return;
		const { error: failure } = await api.DELETE('/works/{workId}', {
			params: { path: { workId } }
		});
		if (failure) error = message(failure);
		else await goto(resolve('/works'));
	}

	function instrumentName(id: string | null | undefined) {
		return instruments.find((i) => i.id === id)?.name ?? '—';
	}

	function sourceName(id: string) {
		return work?.sources.find((s) => s.id === id)?.fileName ?? '';
	}
</script>

<p><a href={resolve('/works')}>← {t('Library')}</a></p>

{#if error}<p class="error">{error}</p>{/if}

{#if work}
	<header class="title">
		<div>
			<h1>{work.title}</h1>
			<p class="muted">
				{[work.composer, work.arranger && t('arr. {name}', { name: work.arranger }), work.publisher]
					.filter(Boolean)
					.join(' · ') || t('No composer yet')}
			</p>
		</div>
		{#if isStaff(session.me)}
			<div class="header-actions">
				{#if canRead && work.sources.length}
					<button
						disabled={reading}
						onclick={() => work && readDetails(work.sources[0].id)}
						title={t(
							'Reads title, composer, arranger and publisher from the first page of the first file.'
						)}
					>
						{reading ? t('Reading …') : t('Read details from the PDF')}
					</button>
				{/if}
				<button onclick={() => (showDetails = !showDetails)} aria-expanded={showDetails}>
					{showDetails ? t('Hide details') : t('Edit details')}
				</button>
			</div>
		{/if}
	</header>

	{#if found}
		<section class="card found">
			<p><strong>{t('Found on the first page')}</strong></p>
			{#each found as offer (offer.field)}
				<label>
					<input type="checkbox" bind:checked={offer.take} />
					<span class="field">{detailNames[offer.field]}</span>
					<span>{offer.value}</span>
					{#if work[offer.field]}
						<span class="muted"
							>{t('instead of “{value}”', { value: work[offer.field] ?? '' })}</span
						>
					{/if}
				</label>
			{/each}
			<p class="buttons">
				<button class="primary" disabled={!found.some((f) => f.take)} onclick={takeDetails}
					>{t('Take over')}</button
				>
				<button onclick={() => (found = null)}>{t('Discard')}</button>
			</p>
		</section>
	{/if}

	{#if showDetails}
		<section class="card">
			{#key work.id}
				<WorkForm {work} onsaved={(saved) => work && Object.assign(work, saved)} />
			{/key}
			<p><button class="danger" onclick={deleteWork}>{t('Delete work')}</button></p>
		</section>
	{/if}

	<section>
		<h2>{t('Parts')}</h2>
		{#if work.parts.length === 0}
			<p class="muted">{t('No parts yet. Upload a PDF, select its pages and create a part.')}</p>
		{:else}
			<table>
				<thead>
					<tr>
						<th>{t('Part')}</th><th>{t('Instrument')}</th><th>{t('Pages')}</th><th>{t('From')}</th
						><th></th>
					</tr>
				</thead>
				<tbody>
					{#each work.parts as part (part.id)}
						<tr class:editing={editing?.id === part.id}>
							<td>
								<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- served by the API, not a route -->
								<a href="/api/parts/{part.id}/file" target="_blank" rel="noopener">{part.label}</a>
							</td>
							<td>{instrumentName(part.instrumentId)}</td>
							<td>{formatPages(part.pages)}</td>
							<td class="muted">{sourceName(part.sourceId)}</td>
							<td class="row-actions">
								{#if isStaff(session.me)}
									<button onclick={() => (editing = part)}>{t('Edit')}</button>
									<button class="danger" onclick={() => deletePart(part)}>{t('Delete')}</button>
								{/if}
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{/if}
	</section>

	<section>
		<h2>{t('Recordings')}</h2>
		<Recordings
			{work}
			{instruments}
			canManage={isStaff(session.me) || session.me?.role === 'leader'}
		/>
	</section>

	{#if isStaff(session.me)}
		<section>
			<h2>{t('Paper archive')}</h2>
			<p class="muted">
				{work.archiveLocation
					? t('Kept in: {place}', { place: work.archiveLocation })
					: t('No place in the archive yet (set it under “Edit details”).')}
			</p>
			{#if loans.length}
				<table class="loans">
					<tbody>
						{#each loans as loan (loan.id)}
							<tr class:returned={!!loan.returnedOn}>
								<td>{loan.item}{loan.quantity > 1 ? ` × ${loan.quantity}` : ''}</td>
								<td>{loan.borrowerName}</td>
								<td class="muted">{t('since {date}', { date: formatDate(loan.lentOn) })}</td>
								<td class:late={overdue(loan)}>
									{loan.returnedOn
										? t('returned {date}', { date: formatDate(loan.returnedOn) })
										: loan.dueOn
											? t('until {date}', { date: formatDate(loan.dueOn) })
											: ''}
								</td>
								<td class="row-actions">
									{#if !loan.returnedOn}
										<button onclick={() => giveBack(loan)}>{t('Returned')}</button>
									{/if}
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			{/if}
			<LoanForm {workId} items={lendable} {borrowers} onlent={loadLoans} />
		</section>

		<section>
			<h2>{t('Files')}</h2>
			<label
				class="drop"
				class:dragging
				ondragover={(e) => {
					e.preventDefault();
					dragging = true;
				}}
				ondragleave={() => (dragging = false)}
				ondrop={drop}
			>
				<input
					type="file"
					accept="application/pdf,.pdf"
					multiple
					onchange={(e) => {
						const input = e.currentTarget;
						if (input.files?.length) upload(input.files).then(() => (input.value = ''));
					}}
				/>
				<strong>{t('Drop PDFs here')}</strong>
				{t(
					'or click to choose. A score, single parts or a scan with everything – you split it below.'
				)}
			</label>
			{#each uploads as entry, i (i)}
				<div class="upload">
					<span>{entry.name}</span>
					{#if entry.error}
						<span class="error">{entry.error}</span>
						<button onclick={() => (uploads = uploads.filter((u) => u !== entry))}
							>{t('Dismiss')}</button
						>
					{:else}
						<progress max="1" value={entry.progress}></progress>
					{/if}
				</div>
			{/each}

			{#each work.sources as source (source.id)}
				{@const parts = work.parts.filter((p) => p.sourceId === source.id)}
				<div class="source-actions">
					<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- served by the API, not a route -->
					<a href="/api/sources/{source.id}/file" target="_blank" rel="noopener"
						>{t('Open original')}</a
					>
					<button
						class="danger"
						onclick={() => deleteSource(source.id, source.fileName, parts.length)}
					>
						{t('Delete file')}
					</button>
				</div>
				<SourceSplitter
					{source}
					{instruments}
					{parts}
					editing={editing?.sourceId === source.id ? editing : null}
					onsaved={() => {
						editing = null;
						load();
					}}
					oncancel={() => (editing = null)}
				/>
			{/each}
		</section>
	{/if}
{:else if !error}
	<p class="muted">{t('Loading…')}</p>
{/if}

<style>
	.title {
		display: flex;
		flex-wrap: wrap;
		justify-content: space-between;
		align-items: start;
		gap: 1rem;
	}
	h1 {
		margin: 0;
	}
	.loans tr.returned td {
		color: var(--muted);
	}
	.loans .late {
		color: var(--danger);
		font-weight: 600;
	}
	.loans {
		margin-bottom: 0.75rem;
	}
	.header-actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
	}
	.found {
		margin-top: 1rem;
	}
	.found p {
		margin: 0 0 0.5rem;
	}
	.found label {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 0.5rem;
		padding: 0.2rem 0;
	}
	.found .field {
		min-width: 7rem;
		color: var(--muted);
	}
	.found .buttons {
		display: flex;
		gap: 0.5rem;
		margin: 0.6rem 0 0;
	}
	h2 {
		margin-top: 2rem;
		font-size: 1.2rem;
	}
	.card {
		background: var(--surface);
		border-radius: var(--radius);
		padding: 1rem;
	}
	table {
		width: 100%;
		border-collapse: collapse;
	}
	th,
	td {
		text-align: left;
		padding: 0.5rem 0.5rem 0.5rem 0;
		border-bottom: 1px solid var(--line);
	}
	th {
		font-size: 0.85em;
		color: var(--muted);
		font-weight: 600;
	}
	tr.editing {
		background: var(--selected);
	}
	.row-actions {
		text-align: right;
		white-space: nowrap;
	}
	@media (max-width: 40rem) {
		thead,
		td:nth-child(4) {
			display: none;
		}
		td:nth-child(2),
		td:nth-child(3) {
			font-size: 0.85em;
		}
	}
	.drop {
		display: block;
		border: 2px dashed var(--line);
		border-radius: var(--radius);
		padding: 1.5rem;
		text-align: center;
		cursor: pointer;
		margin-bottom: 1rem;
	}
	.drop.dragging {
		border-color: var(--accent);
		background: var(--selected);
	}
	.drop input {
		display: none;
	}
	.upload {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.75rem;
		margin-bottom: 0.5rem;
	}
	.source-actions {
		display: flex;
		justify-content: flex-end;
		align-items: center;
		gap: 0.75rem;
		margin-bottom: 0.25rem;
	}
</style>
