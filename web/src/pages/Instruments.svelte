<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- The organisation's instrument catalogue: families and instruments (admins and librarians). -->
<script lang="ts">
	import {
		api,
		message,
		type Instrument,
		type InstrumentFamily,
		type InstrumentInput
	} from '#lib/api/index.js';
	import { isStaff, session } from '#lib/session.svelte.js';
	import { plural, t } from '#lib/i18n/index.svelte.js';
	import { confirm, notify } from '#lib/dialog.svelte.js';
	import DataTable, { type Column } from '#lib/components/DataTable.svelte';
	import { instrumentOptions } from '#lib/instruments.js';
	import Icon from '#lib/components/Icon.svelte';
	import Select from '#lib/components/Select.svelte';

	let families = $state<InstrumentFamily[]>([]);
	let instruments = $state<Instrument[]>([]);
	let selected = $state<string[]>([]);
	let busy = $state(false);

	// The instrument form adds a new one, or edits `editing`.
	let editing = $state<Instrument | null>(null);
	let name = $state('');
	let familyId = $state('');
	let transposition = $state('');
	let aliases = $state('');

	let newFamily = $state('');
	let renaming = $state<string | null>(null);
	let familyName = $state('');
	let moveTo = $state('');
	let moving = $state(false);

	const familyOptions = $derived(families.map((f) => ({ value: f.id, label: f.name })));

	const columns = $derived<Column<Instrument>[]>([
		{ id: 'name', label: t('Name'), value: (i) => i.name },
		{ id: 'family', label: t('Family'), value: (i) => i.family },
		{ id: 'transposition', label: t('Key'), value: (i) => i.transposition },
		{ id: 'aliases', label: t('Other names'), value: (i) => i.aliases.join(', '), wide: true },
		{ id: 'usage', label: t('In use'), value: (i) => i.usage, align: 'right', wide: true },
		{ id: 'actions', label: '', value: () => '', sortable: false, align: 'right' }
	]);

	async function load() {
		const [f, i] = await Promise.all([api.GET('/instrument-families'), api.GET('/instruments')]);
		families = f.data ?? [];
		instruments = i.data ?? [];
		selected = selected.filter((id) => instruments.some((i) => i.id === id));
		if (!familyId && families.length) familyId = families[0].id;
	}

	$effect(() => {
		load();
	});

	function edit(instrument: Instrument | null) {
		editing = instrument;
		name = instrument?.name ?? '';
		familyId = instrument?.familyId ?? familyId;
		transposition = instrument?.transposition ?? '';
		aliases = instrument?.aliases.join(', ') ?? '';
		if (instrument)
			document.querySelector('.instrument-form')?.scrollIntoView({ block: 'nearest' });
	}

	async function save(event: SubmitEvent) {
		event.preventDefault();
		const body: InstrumentInput = {
			name,
			familyId,
			transposition: transposition || null,
			aliases: aliases.split(',').map((a) => a.trim()),
			// Moving to another family puts it last there.
			sort: editing && editing.familyId === familyId ? editing.sort : null
		};
		busy = true;
		const { error } = editing
			? await api.PUT('/instruments/{instrumentId}', {
					params: { path: { instrumentId: editing.id } },
					body
				})
			: await api.POST('/instruments', { body });
		busy = false;
		if (error) return notify(message(error), 'error');
		notify(editing ? t('Saved.') : t('Added {name}.', { name }));
		edit(null);
		await load();
	}

	// Folding a duplicate into the instrument that stays.
	let merging = $state<Instrument | null>(null);
	let mergeInto = $state('');

	async function merge() {
		if (!merging || !mergeInto) return;
		const stays = instruments.find((i) => i.id === mergeInto);
		const ok = await confirm({
			title: t('Merge {name} into {other}?', { name: merging.name, other: stays?.name ?? '' }),
			message: t(
				'Its parts, members and recordings move over; its names stay as other names. This cannot be undone.'
			),
			confirm: t('Merge')
		});
		if (!ok) return;
		const { error } = await api.POST('/instruments/{instrumentId}/merge', {
			params: { path: { instrumentId: merging.id } },
			body: { into: mergeInto }
		});
		if (error) return notify(message(error), 'error');
		notify(t('Merged.'));
		merging = null;
		mergeInto = '';
		await load();
	}

	async function remove(instrument: Instrument) {
		const ok = await confirm({
			title: t('Delete {name}?', { name: instrument.name }),
			confirm: t('Delete'),
			danger: true
		});
		if (!ok) return;
		const { error } = await api.DELETE('/instruments/{instrumentId}', {
			params: { path: { instrumentId: instrument.id } }
		});
		if (error) notify(message(error), 'error');
		await load();
	}

	async function moveSelected() {
		busy = true;
		let failed = 0;
		for (const instrument of instruments.filter((i) => selected.includes(i.id))) {
			const { error } = await api.PUT('/instruments/{instrumentId}', {
				params: { path: { instrumentId: instrument.id } },
				body: { ...instrument, familyId: moveTo, sort: null }
			});
			if (error) failed++;
		}
		busy = false;
		moving = false;
		if (failed) notify(t('Some instruments could not be moved.'), 'error');
		await load();
	}

	async function deleteSelected() {
		const unused = instruments.filter((i) => selected.includes(i.id) && i.usage === 0);
		if (!unused.length) return notify(t('Only instruments nobody uses can be deleted.'), 'error');
		const ok = await confirm({
			title: plural(unused.length, 'Delete {n} instrument?', 'Delete {n} instruments?'),
			message: unused.length < selected.length ? t('Instruments in use stay.') : undefined,
			confirm: t('Delete'),
			danger: true
		});
		if (!ok) return;
		for (const instrument of unused)
			await api.DELETE('/instruments/{instrumentId}', {
				params: { path: { instrumentId: instrument.id } }
			});
		await load();
	}

	async function addFamily(event: SubmitEvent) {
		event.preventDefault();
		const { error } = await api.POST('/instrument-families', { body: { name: newFamily } });
		if (error) return notify(message(error), 'error');
		newFamily = '';
		await load();
	}

	async function saveFamily(family: InstrumentFamily, body: { name: string; sort?: number }) {
		const { error } = await api.PUT('/instrument-families/{familyId}', {
			params: { path: { familyId: family.id } },
			body: { sort: family.sort, ...body }
		});
		if (error) notify(message(error), 'error');
		renaming = null;
		await load();
	}

	/** Swaps a family with its neighbour (the order of the groups in every instrument list). */
	async function shift(index: number, by: -1 | 1) {
		const a = families[index];
		const b = families[index + by];
		if (!a || !b) return;
		// Equal sort values would not move anything.
		const [sa, sb] = a.sort === b.sort ? [index * 10, (index + by) * 10] : [a.sort, b.sort];
		await saveFamily(a, { name: a.name, sort: sb });
		await saveFamily(b, { name: b.name, sort: sa });
	}

	async function deleteFamily(family: InstrumentFamily) {
		const ok = await confirm({
			title: t('Delete {name}?', { name: family.name }),
			confirm: t('Delete'),
			danger: true
		});
		if (!ok) return;
		const { error } = await api.DELETE('/instrument-families/{familyId}', {
			params: { path: { familyId: family.id } }
		});
		if (error) notify(message(error), 'error');
		await load();
	}
</script>

<h1>{t('Instruments')}</h1>

{#if !isStaff(session.me)}
	<p class="muted">{t('Only admins and librarians change the instruments.')}</p>
{:else}
	<p class="muted">
		{t(
			'Name the instruments the way your organisation writes them. Other names and abbreviations help to recognise file names and imports.'
		)}
	</p>

	<form class="instrument-form" onsubmit={save}>
		<h2>{editing ? t('Edit {name}', { name: editing.name }) : t('New instrument')}</h2>
		<div class="fields">
			<label>
				{t('Name')}
				<input bind:value={name} required maxlength="80" />
			</label>
			<label>
				{t('Family')}
				<Select bind:value={familyId} label={t('Family')} options={familyOptions} />
			</label>
			<label class="short">
				{t('Key')}
				<input bind:value={transposition} maxlength="10" placeholder={t('Bb, Eb, F …')} />
			</label>
			<label class="grow">
				{t('Other names, separated by commas')}
				<input bind:value={aliases} placeholder={t('e.g. Alto Sax, A-Sax')} />
			</label>
		</div>
		<div class="buttons">
			<button class="primary" disabled={busy || !name.trim() || !familyId}>
				{editing ? t('Save') : t('Add')}
			</button>
			{#if editing}<button type="button" onclick={() => edit(null)}>{t('Cancel')}</button>{/if}
		</div>
	</form>

	{#if merging}
		<div class="merge">
			<span>{t('Merge {name} into', { name: merging.name })}</span>
			<Select
				bind:value={mergeInto}
				label={t('Instrument that stays')}
				placeholder={t('Instrument …')}
				options={instrumentOptions(instruments.filter((i) => i.id !== merging?.id))}
			/>
			<button class="primary" disabled={!mergeInto} onclick={merge}>{t('Merge')}</button>
			<button onclick={() => (merging = null)}>{t('Cancel')}</button>
		</div>
	{/if}

	<DataTable
		id="instruments"
		rows={instruments}
		key={(i) => i.id}
		{columns}
		bind:selected
		filterPlaceholder={t('Filter …')}
	>
		{#snippet cell(instrument, column)}
			{#if column.id === 'actions'}
				<span class="row-actions">
					<button class="icon" title={t('Edit')} onclick={() => edit(instrument)}>
						<Icon name="edit" label={t('Edit')} />
					</button>
					<button
						class="icon"
						title={t('Merge into another instrument …')}
						onclick={() => {
							merging = instrument;
							mergeInto = '';
						}}
					>
						<Icon name="columns" label={t('Merge into another instrument …')} />
					</button>
					<button
						class="icon danger"
						title={instrument.usage ? t('In use – cannot be deleted') : t('Delete')}
						disabled={instrument.usage > 0}
						onclick={() => remove(instrument)}
					>
						<Icon name="trash" label={t('Delete')} />
					</button>
				</span>
			{:else if column.id === 'aliases'}
				<span class="muted">{instrument.aliases.join(', ')}</span>
			{:else}
				{column.value(instrument) ?? ''}
			{/if}
		{/snippet}
		{#snippet bulk()}
			{#if moving}
				<Select
					bind:value={moveTo}
					label={t('Family')}
					placeholder={t('Family …')}
					options={familyOptions}
				/>
				<button class="primary" disabled={busy || !moveTo} onclick={moveSelected}
					>{t('Apply')}</button
				>
				<button onclick={() => (moving = false)}>{t('Cancel')}</button>
			{:else}
				<button onclick={() => (moving = true)}>{t('Move to family …')}</button>
				<button class="danger" disabled={busy} onclick={deleteSelected}>{t('Delete')}</button>
			{/if}
		{/snippet}
	</DataTable>

	<h2>{t('Families')}</h2>
	<p class="muted">{t('Instrument lists are grouped by family in this order.')}</p>
	<ol class="families">
		{#each families as family, index (family.id)}
			<li>
				{#if renaming === family.id}
					<form
						class="rename"
						onsubmit={(e) => {
							e.preventDefault();
							saveFamily(family, { name: familyName });
						}}
					>
						<input bind:value={familyName} required maxlength="80" aria-label={t('Name')} />
						<button class="primary">{t('Save')}</button>
						<button type="button" onclick={() => (renaming = null)}>{t('Cancel')}</button>
					</form>
				{:else}
					<span class="name">{family.name}</span>
					<span class="muted"
						>{plural(family.instrumentCount, '{n} instrument', '{n} instruments')}</span
					>
					<span class="row-actions">
						<button
							class="icon"
							title={t('Move up')}
							disabled={index === 0}
							onclick={() => shift(index, -1)}
							><Icon name="chevronUp" label={t('Move up')} /></button
						>
						<button
							class="icon"
							title={t('Move down')}
							disabled={index === families.length - 1}
							onclick={() => shift(index, 1)}
							><Icon name="chevronDown" label={t('Move down')} /></button
						>
						<button
							class="icon"
							title={t('Rename')}
							onclick={() => {
								renaming = family.id;
								familyName = family.name;
							}}><Icon name="edit" label={t('Rename')} /></button
						>
						<button
							class="icon danger"
							title={family.instrumentCount
								? t('Has instruments – cannot be deleted')
								: t('Delete')}
							disabled={family.instrumentCount > 0}
							onclick={() => deleteFamily(family)}><Icon name="trash" label={t('Delete')} /></button
						>
					</span>
				{/if}
			</li>
		{/each}
	</ol>
	<form class="add-family" onsubmit={addFamily}>
		<input bind:value={newFamily} placeholder={t('New family')} required maxlength="80" />
		<button disabled={!newFamily.trim()}>{t('Add')}</button>
	</form>
{/if}

<style>
	h2 {
		font-size: 1.15rem;
		margin: 2rem 0 0.5rem;
	}
	.instrument-form {
		background: var(--surface);
		border: 1px solid var(--line);
		border-radius: var(--radius);
		padding: 0.75rem 1rem 1rem;
		margin-bottom: 1.5rem;
	}
	.instrument-form h2 {
		margin-top: 0.25rem;
	}
	.fields {
		display: flex;
		flex-wrap: wrap;
		gap: 0.75rem;
	}
	.fields label {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		font-size: 0.9em;
		color: var(--muted);
	}
	.fields .short input {
		width: 7rem;
	}
	.fields .grow {
		flex: 1 1 16rem;
	}
	.buttons {
		display: flex;
		gap: 0.5rem;
		margin-top: 0.75rem;
	}
	.merge {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.5rem;
		padding: 0.6rem 0.9rem;
		margin-bottom: 0.75rem;
		border-radius: var(--radius);
		background: var(--selected);
	}
	.row-actions {
		display: inline-flex;
		gap: 0.25rem;
		white-space: nowrap;
	}
	.families {
		list-style: none;
		padding: 0;
		margin: 0 0 0.75rem;
		border-top: 1px solid var(--line);
	}
	.families li {
		display: flex;
		align-items: center;
		gap: 1rem;
		padding: 0.4rem 0;
		border-bottom: 1px solid var(--line);
	}
	.families .name {
		flex: 1;
		font-weight: 600;
	}
	.rename,
	.add-family {
		display: flex;
		gap: 0.5rem;
		flex: 1;
	}
	.rename input {
		flex: 1;
	}
	.add-family input {
		flex: 0 1 20rem;
	}
</style>
