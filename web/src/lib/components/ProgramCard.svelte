<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
	A programme of an ensemble: works and breaks in playing order with their durations, the total and – with a
	start time – when each item begins. Leaders reorder by dragging (or the arrows), add breaks and notes.
-->
<script lang="ts">
	import { resolve } from '#lib/router.svelte.js';
	import {
		api,
		message,
		type EnsembleDetail,
		type Program,
		type ProgramItem
	} from '#lib/api/index.js';
	import { formatDate } from '#lib/dates.js';
	import { workInput } from '#lib/works.js';
	import { clockAfter, formatDuration, parseDuration } from '#lib/durations.js';
	import { plural, t } from '#lib/i18n/index.svelte.js';
	import { confirm, notify } from '#lib/dialog.svelte.js';
	import Icon from '#lib/components/Icon.svelte';
	import Select from '#lib/components/Select.svelte';

	interface Props {
		program: Program;
		ensemble: EnsembleDetail;
		/** Called after every change, to reload the ensemble. */
		onchange: () => Promise<void>;
	}

	let { program, ensemble, onchange }: Props = $props();

	let editing = $state(false);
	let name = $state('');
	let date = $state('');
	let startTime = $state('');
	let venue = $state('');
	let notes = $state('');
	let adding = $state('');
	let breakLabel = $state('');
	let breakMinutes = $state('');
	let error = $state('');
	let dragged = $state<number | null>(null);
	let noteFor = $state<number | null>(null);
	let noteText = $state('');
	let durationFor = $state<string | null>(null);
	let durationText = $state('');

	const works = $derived(new Map(ensemble.works.map((w) => [w.id, w])));
	const available = $derived(ensemble.works.filter((w) => !program.workIds.includes(w.id)));

	function duration(item: ProgramItem): number | null {
		return item.kind === 'work'
			? (works.get(item.workId)?.durationSeconds ?? null)
			: (item.durationSeconds ?? null);
	}

	/** Where each item starts, in seconds from the beginning (unknown durations count as 0). */
	const starts = $derived.by(() => {
		let at = 0;
		return program.items.map((item) => {
			const start = at;
			at += duration(item) ?? 0;
			return start;
		});
	});
	const total = $derived(program.items.reduce((sum, item) => sum + (duration(item) ?? 0), 0));
	const missing = $derived(
		program.items.filter((i) => i.kind === 'work' && duration(i) == null).length
	);

	async function saveItems(items: ProgramItem[]) {
		error = '';
		const { error: failure } = await api.PUT('/programs/{programId}/items', {
			params: { path: { programId: program.id } },
			body: { items }
		});
		if (failure) error = message(failure);
		await onchange();
	}

	function move(from: number, to: number) {
		if (to < 0 || to >= program.items.length || from === to) return;
		const items = [...program.items];
		const [item] = items.splice(from, 1);
		items.splice(to, 0, item);
		saveItems(items);
	}

	function remove(index: number) {
		saveItems(program.items.filter((_, i) => i !== index));
	}

	function addBreak(event: SubmitEvent) {
		event.preventDefault();
		const minutes = Number(breakMinutes.replace(',', '.'));
		saveItems([
			...program.items,
			{
				kind: 'break',
				label: breakLabel.trim() || t('Interval'),
				durationSeconds: breakMinutes && minutes >= 0 ? Math.round(minutes * 60) : null
			}
		]).then(() => (breakLabel = breakMinutes = ''));
	}

	function saveNote(index: number) {
		const items = program.items.map((item, i) =>
			i === index ? { ...item, note: noteText.trim() || null } : item
		);
		noteFor = null;
		saveItems(items);
	}

	/** A work's duration is the work's own; set here because planning is where it is missed. */
	async function saveDuration(workId: string) {
		const seconds = parseDuration(durationText);
		if (Number.isNaN(seconds))
			return notify(t('Write the duration as minutes:seconds, e.g. 4:30.'), 'error');
		const { data: work } = await api.GET('/works/{workId}', { params: { path: { workId } } });
		if (!work) return;
		const { error: failure } = await api.PUT('/works/{workId}', {
			params: { path: { workId } },
			body: workInput(work, { durationSeconds: seconds })
		});
		if (failure) notify(message(failure), 'error');
		durationFor = null;
		await onchange();
	}

	function startEditing() {
		name = program.name;
		date = program.date ?? '';
		startTime = program.startTime ?? '';
		venue = program.venue ?? '';
		notes = program.notes ?? '';
		editing = true;
	}

	async function saveDetails(event: SubmitEvent) {
		event.preventDefault();
		const { error: failure } = await api.PUT('/programs/{programId}', {
			params: { path: { programId: program.id } },
			body: {
				name,
				date: date || null,
				startTime: startTime || null,
				venue: venue || null,
				notes: notes || null
			}
		});
		if (failure) error = message(failure);
		else editing = false;
		await onchange();
	}

	async function removeProgram() {
		if (
			!(await confirm({
				title: t('Delete the programme “{name}”? The works stay with the ensemble.', {
					name: program.name
				}),
				confirm: t('Delete'),
				danger: true
			}))
		)
			return;
		const { error: failure } = await api.DELETE('/programs/{programId}', {
			params: { path: { programId: program.id } }
		});
		if (failure) error = message(failure);
		await onchange();
	}
</script>

<article class="program">
	{#if editing && ensemble.canManage}
		<form class="details" onsubmit={saveDetails}>
			<label class="wide">{t('Name')} <input bind:value={name} required maxlength="120" /></label>
			<label>{t('Date')} <input type="date" bind:value={date} /></label>
			<label>{t('Starts at')} <input type="time" bind:value={startTime} /></label>
			<label class="wide">{t('Venue')} <input bind:value={venue} maxlength="200" /></label>
			<label class="full">{t('Notes')} <textarea rows="2" bind:value={notes}></textarea></label>
			<div class="full row">
				<button class="primary">{t('Save')}</button>
				<button type="button" onclick={() => (editing = false)}>{t('Cancel')}</button>
				<button type="button" class="danger" onclick={removeProgram}>{t('Delete programme')}</button
				>
			</div>
		</form>
	{:else}
		<header>
			<h3>{program.name}</h3>
			<span class="muted">
				{[
					program.date && formatDate(program.date),
					program.startTime && t('at {time}', { time: program.startTime }),
					program.venue
				]
					.filter(Boolean)
					.join(' · ')}
			</span>
			<span class="spacer"></span>
			<a
				class="button"
				href={resolve('/ensembles/[id]/programs/[programId]', {
					id: ensemble.id,
					programId: program.id
				})}>{t('Running order')}</a
			>
			<a
				class="button"
				href={resolve('/ensembles/[id]/programs/[programId]/setlist', {
					id: ensemble.id,
					programId: program.id
				})}>{t('Set list')}</a
			>
			{#if ensemble.canManage}<button onclick={startEditing}>{t('Edit')}</button>{/if}
		</header>
		{#if program.notes}<p class="notes">{program.notes}</p>{/if}
	{/if}

	{#if program.items.length === 0}
		<p class="muted">{t('No works in this programme yet.')}</p>
	{:else}
		<ol class="items">
			{#each program.items as item, i (item.kind === 'work' ? item.workId : `break-${i}`)}
				{@const work = item.kind === 'work' ? works.get(item.workId) : undefined}
				<li
					class:break={item.kind === 'break'}
					class:dragging={dragged === i}
					draggable={ensemble.canManage}
					ondragstart={() => (dragged = i)}
					ondragend={() => (dragged = null)}
					ondragover={(e) => e.preventDefault()}
					ondrop={(e) => {
						e.preventDefault();
						if (dragged !== null) move(dragged, i);
						dragged = null;
					}}
				>
					{#if ensemble.canManage}
						<span class="grip" aria-hidden="true"><Icon name="grip" size={16} /></span>
					{/if}
					{#if program.startTime}
						<span class="clock">{clockAfter(program.startTime, starts[i])}</span>
					{/if}
					<span class="what">
						{#if item.kind === 'work'}
							<strong>{work?.title ?? '?'}</strong>
							{#if work?.composer || work?.arranger}
								<span class="muted">
									{[work.composer, work.arranger && t('arr. {name}', { name: work.arranger })]
										.filter(Boolean)
										.join(' · ')}
								</span>
							{/if}
						{:else}
							<em>{item.label}</em>
						{/if}
						{#if noteFor === i}
							<form
								class="row"
								onsubmit={(e) => {
									e.preventDefault();
									saveNote(i);
								}}
							>
								<input bind:value={noteText} maxlength="300" aria-label={t('Note')} />
								<button class="primary">{t('Save')}</button>
								<button type="button" onclick={() => (noteFor = null)}>{t('Cancel')}</button>
							</form>
						{:else if item.note}
							<span class="note">{item.note}</span>
						{/if}
					</span>
					<span class="duration">
						{#if item.kind === 'work' && durationFor === item.workId}
							<form
								class="row"
								onsubmit={(e) => {
									e.preventDefault();
									saveDuration(item.workId);
								}}
							>
								<input
									class="short"
									bind:value={durationText}
									placeholder="4:30"
									aria-label={t('Duration')}
								/>
								<button class="primary">{t('Save')}</button>
							</form>
						{:else if duration(item) != null}
							{formatDuration(duration(item))}
						{:else if item.kind === 'work' && ensemble.canManage}
							<button
								class="link"
								onclick={() => {
									durationFor = item.workId;
									durationText = '';
								}}>{t('Add duration')}</button
							>
						{:else}
							<span class="muted">–</span>
						{/if}
					</span>
					{#if ensemble.canManage}
						<span class="controls">
							<button
								class="icon"
								title={t('Note')}
								onclick={() => {
									noteFor = i;
									noteText = item.note ?? '';
								}}><Icon name="edit" size={16} label={t('Note')} /></button
							>
							<button
								class="icon"
								disabled={i === 0}
								title={t('Move up')}
								onclick={() => move(i, i - 1)}
								><Icon name="chevronUp" size={16} label={t('Move up')} /></button
							>
							<button
								class="icon"
								disabled={i === program.items.length - 1}
								title={t('Move down')}
								onclick={() => move(i, i + 1)}
								><Icon name="chevronDown" size={16} label={t('Move down')} /></button
							>
							<button class="icon danger" title={t('Remove')} onclick={() => remove(i)}
								><Icon name="x" size={16} label={t('Remove')} /></button
							>
						</span>
					{/if}
				</li>
			{/each}
		</ol>
		<p class="total">
			{t('Total {duration}', { duration: formatDuration(total) })}
			{#if program.startTime}· {t('ends about {time}', {
					time: clockAfter(program.startTime, total)
				})}{/if}
			{#if missing}
				· <span class="error"
					>{plural(missing, '{n} work without duration', '{n} works without duration')}</span
				>
			{/if}
		</p>
	{/if}

	{#if ensemble.canManage}
		<div class="adders">
			{#if available.length}
				<form
					class="row"
					onsubmit={(e) => {
						e.preventDefault();
						if (adding)
							saveItems([...program.items, { kind: 'work', workId: adding }]).then(
								() => (adding = '')
							);
					}}
				>
					<Select
						bind:value={adding}
						label={t('Work to add')}
						placeholder={t('Add a work of the ensemble …')}
						options={available.map((w) => ({
							value: w.id,
							label: w.title,
							hint: w.composer ?? undefined
						}))}
					/>
					<button disabled={!adding}>{t('Add')}</button>
				</form>
			{/if}
			<form class="row" onsubmit={addBreak}>
				<input bind:value={breakLabel} maxlength="120" placeholder={t('Interval, speech …')} />
				<input
					class="short"
					bind:value={breakMinutes}
					inputmode="decimal"
					placeholder={t('min')}
					aria-label={t('Minutes')}
				/>
				<button>{t('Add break')}</button>
			</form>
		</div>
	{/if}
	{#if error}<p class="error">{error}</p>{/if}
</article>

<style>
	.program {
		border: 1px solid var(--line);
		border-radius: var(--radius);
		padding: 0.75rem 1rem;
		margin-bottom: 0.75rem;
	}
	header {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.5rem 0.75rem;
	}
	.spacer {
		flex: 1;
	}
	h3 {
		margin: 0;
		font-family: var(--serif);
		font-size: 1.05rem;
	}
	.notes {
		margin: 0.4rem 0 0;
		color: var(--muted);
		white-space: pre-line;
	}
	.details {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(10rem, 1fr));
		gap: 0.6rem;
	}
	.details label {
		display: flex;
		flex-direction: column;
		gap: 0.2rem;
		font-size: 0.85em;
		color: var(--muted);
	}
	.details label > :global(*) {
		color: var(--text);
		font-size: 1rem;
	}
	.details .wide {
		grid-column: span 2;
	}
	.details .full {
		grid-column: 1 / -1;
	}
	.items {
		list-style: none;
		margin: 0.6rem 0 0;
		padding: 0;
		border-top: 1px solid var(--line);
	}
	.items li {
		display: flex;
		align-items: baseline;
		gap: 0.6rem;
		padding: 0.4rem 0;
		border-bottom: 1px solid var(--line);
	}
	.items li[draggable='true'] {
		cursor: grab;
	}
	.items li.dragging {
		opacity: 0.4;
	}
	.items li.break {
		background: var(--surface);
	}
	.grip {
		color: var(--muted);
		align-self: center;
	}
	.clock {
		font-variant-numeric: tabular-nums;
		color: var(--muted);
		min-width: 3rem;
	}
	.what {
		flex: 1;
		display: flex;
		flex-wrap: wrap;
		gap: 0.15rem 0.6rem;
	}
	.note {
		flex-basis: 100%;
		font-size: 0.9em;
		color: var(--accent);
	}
	.duration {
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}
	.controls {
		display: inline-flex;
		gap: 0.15rem;
	}
	.controls .icon {
		padding: 0.2rem;
	}
	.total {
		margin: 0.5rem 0 0;
		font-weight: 600;
	}
	.adders {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem 1.5rem;
		margin-top: 0.6rem;
	}
	.row {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
		align-items: center;
	}
	.short {
		width: 5rem;
	}
</style>
