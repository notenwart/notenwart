<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import { goto, resolve } from '#lib/router.svelte.js';
	import { api, message, type Ensemble, type WorkSummary } from '#lib/api/index.js';
	import { isStaff, session } from '#lib/session.svelte.js';
	import { i18n, plural, t } from '#lib/i18n/index.svelte.js';
	import { confirm, notify } from '#lib/dialog.svelte.js';
	import DataTable, { type Column } from '#lib/components/DataTable.svelte';
	import Autocomplete from '#lib/components/Autocomplete.svelte';
	import Select from '#lib/components/Select.svelte';

	let works = $state<WorkSummary[]>([]);
	let tags = $state<string[]>([]);
	let composers = $state<string[]>([]);
	let ensembles = $state<Ensemble[]>([]);
	let tag = $state('');
	let loading = $state(true);
	let error = $state('');
	let selected = $state<string[]>([]);
	let giving = $state(false);
	let ensemble = $state('');
	let busy = $state(false);

	let title = $state('');
	let composer = $state('');

	const staff = $derived(isStaff(session.me));
	const empty = $derived(tag ? t('No works match.') : t('No works yet. Add the first one above.'));

	function duration(seconds: number | null | undefined) {
		if (!seconds) return '';
		return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
	}

	const columns = $derived<Column<WorkSummary>[]>([
		{ id: 'title', label: t('Title'), value: (w) => w.title },
		{ id: 'composer', label: t('Composer'), value: (w) => w.composer },
		{ id: 'arranger', label: t('Arranger'), value: (w) => w.arranger, wide: true },
		{ id: 'publisher', label: t('Publisher'), value: (w) => w.publisher, hidden: true, wide: true },
		{
			id: 'instrumentation',
			label: t('Instrumentation'),
			value: (w) => w.instrumentation,
			hidden: true,
			wide: true
		},
		{
			id: 'difficulty',
			label: t('Difficulty'),
			value: (w) => w.difficulty,
			hidden: true,
			align: 'right'
		},
		{
			id: 'duration',
			label: t('Duration'),
			value: (w) => w.durationSeconds,
			hidden: true,
			align: 'right'
		},
		{ id: 'tags', label: t('Tags'), value: (w) => w.tags.join(', '), wide: true },
		{ id: 'parts', label: t('Parts'), value: (w) => w.partCount, align: 'right' },
		{ id: 'files', label: t('Files'), value: (w) => w.sourceCount, align: 'right', hidden: true },
		{
			id: 'archive',
			label: t('Place in the archive'),
			value: (w) => w.archiveLocation,
			hidden: true,
			wide: true
		},
		{ id: 'updated', label: t('Changed'), value: (w) => w.updatedAt, hidden: true, wide: true }
	]);

	async function load() {
		const { data, error: failure } = await api.GET('/works', {
			params: { query: { tag: tag || undefined } }
		});
		loading = false;
		if (data) {
			works = data;
			selected = selected.filter((id) => data.some((w) => w.id === id));
		} else error = message(failure);
	}

	$effect(() => {
		void tag;
		load();
	});

	$effect(() => {
		api.GET('/works/facets').then(({ data }) => {
			tags = [...(data?.tags ?? [])].sort((a, b) => a.localeCompare(b));
			composers = data?.composers ?? [];
		});
		if (staff) api.GET('/ensembles').then(({ data }) => (ensembles = data ?? []));
	});

	async function create(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		const { data, error: failure } = await api.POST('/works', {
			body: { title, composer: composer || null, tags: [] }
		});
		busy = false;
		if (data) await goto(resolve('/works/[id]', { id: data.id }));
		else error = message(failure);
	}

	/** Runs `step` for each selected work one after another and reports how many succeeded. */
	async function forEachSelected(step: (id: string) => Promise<{ error?: unknown }>) {
		busy = true;
		let done = 0;
		let failure: unknown;
		for (const id of selected) {
			const result = await step(id);
			if (result.error) failure = result.error;
			else done++;
		}
		busy = false;
		if (failure) notify(message(failure), 'error');
		if (done) notify(plural(done, 'Done for {n} work.', 'Done for {n} works.'));
		await load();
		return done;
	}

	async function give() {
		const ensembleId = ensemble;
		const done = await forEachSelected((workId) =>
			api.PUT('/ensembles/{ensembleId}/works/{workId}', {
				params: { path: { ensembleId, workId } }
			})
		);
		if (done) giving = false;
	}

	async function remove() {
		const ok = await confirm({
			title: plural(selected.length, 'Delete {n} work?', 'Delete {n} works?'),
			message: t('Their files and parts are deleted too. This cannot be undone.'),
			confirm: t('Delete'),
			danger: true
		});
		if (!ok) return;
		await forEachSelected((workId) =>
			api.DELETE('/works/{workId}', { params: { path: { workId } } })
		);
	}
</script>

<h1>{t('Library')}</h1>

{#if staff}
	<form class="create" onsubmit={create}>
		<input placeholder={t('Title of a new work')} bind:value={title} required maxlength="300" />
		<Autocomplete
			placeholder={t('Composer')}
			label={t('Composer')}
			bind:value={composer}
			suggestions={composers}
		/>
		<button class="primary" disabled={busy || !title.trim()}>{t('Add work')}</button>
		<a class="button" href={resolve('/works/import')}>{t('Import a folder …')}</a>
		<a class="button" href={resolve('/loans')}>{t('Loans')}</a>
	</form>
{/if}

{#if error}<p class="error">{error}</p>{/if}

{#snippet tagFilter()}
	<Select
		bind:value={tag}
		label={t('Tag')}
		options={[
			{ value: '', label: t('All tags') },
			...tags.map((name) => ({ value: name, label: name }))
		]}
	/>
{/snippet}

{#snippet workCell(work: WorkSummary, column: Column<WorkSummary>)}
	{#if column.id === 'title'}
		<a href={resolve('/works/[id]', { id: work.id })}><strong>{work.title}</strong></a>
	{:else if column.id === 'tags'}
		<span class="tags">
			{#each work.tags as name (name)}<span class="tag">{name}</span>{/each}
		</span>
	{:else if column.id === 'duration'}
		{duration(work.durationSeconds)}
	{:else if column.id === 'updated'}
		<span class="muted">{new Date(work.updatedAt).toLocaleDateString(i18n.locale)}</span>
	{:else}
		{column.value(work) ?? ''}
	{/if}
{/snippet}

{#if loading}
	<p class="muted">{t('Loading…')}</p>
{:else if staff}
	<DataTable
		id="works"
		rows={works}
		key={(w) => w.id}
		{columns}
		cell={workCell}
		actions={tagFilter}
		bind:selected
		{empty}
		filterPlaceholder={t('Filter by title, composer, tag …')}
	>
		{#snippet bulk()}
			{#if giving}
				<Select
					bind:value={ensemble}
					label={t('Ensemble')}
					placeholder={t('Ensemble …')}
					options={ensembles.map((e) => ({ value: e.id, label: e.name }))}
				/>
				<button class="primary" disabled={busy || !ensemble} onclick={give}>{t('Apply')}</button>
				<button onclick={() => (giving = false)}>{t('Cancel')}</button>
			{:else}
				<button onclick={() => (giving = true)}>{t('Give to ensemble …')}</button>
				<button class="danger" disabled={busy} onclick={remove}>{t('Delete')}</button>
			{/if}
		{/snippet}
	</DataTable>
{:else}
	<DataTable
		id="works"
		rows={works}
		key={(w) => w.id}
		{columns}
		cell={workCell}
		actions={tagFilter}
		{empty}
		filterPlaceholder={t('Filter by title, composer, tag …')}
	/>
{/if}

<style>
	.create {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
		margin-bottom: 1rem;
	}
	.create input:first-child {
		flex: 1 1 16rem;
	}
	a {
		color: var(--text);
		text-decoration: none;
	}
	a:hover {
		text-decoration: underline;
	}
	.tags {
		display: inline-flex;
		flex-wrap: wrap;
		gap: 0.25rem;
	}
	.tag {
		font-size: 0.8em;
		background: var(--surface);
		border: 1px solid var(--line);
		border-radius: 999px;
		padding: 0.1rem 0.5rem;
	}
</style>
