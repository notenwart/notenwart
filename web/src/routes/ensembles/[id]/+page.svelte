<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import {
		api,
		message,
		type EnsembleDetail,
		type Instrument,
		type PlayedPart,
		type User,
		type WorkSummary
	} from '#lib/api/index.js';
	import GrantsEditor from '#lib/components/GrantsEditor.svelte';
	import MemberRow from '#lib/components/MemberRow.svelte';
	import ProgramCard from '#lib/components/ProgramCard.svelte';
	import GuestLinks from '#lib/components/GuestLinks.svelte';
	import Avatar from '#lib/components/Avatar.svelte';
	import EnsembleLook from '#lib/components/EnsembleLook.svelte';
	import { isStaff, readsLibrary, session } from '#lib/session.svelte.js';
	import { plural, t } from '#lib/i18n/index.svelte.js';
	import { confirm } from '#lib/dialog.svelte.js';
	import Select from '#lib/components/Select.svelte';

	const ensembleId = $derived(page.params.id ?? '');
	let ensemble = $state<EnsembleDetail | null>(null);
	let instruments = $state<Instrument[]>([]);
	let users = $state<User[]>([]);
	let error = $state('');

	let editing = $state(false);
	let name = $state('');
	let description = $state('');

	let newMember = $state('');
	let search = $state('');
	let results = $state<WorkSummary[]>([]);

	const staff = $derived(isStaff(session.me));
	const me = $derived(ensemble?.members.find((m) => m.userId === session.me?.id));
	const candidates = $derived(
		users.filter((u) => !ensemble?.members.some((m) => m.userId === u.id))
	);

	async function load() {
		const { data, error: failure } = await api.GET('/ensembles/{ensembleId}', {
			params: { path: { ensembleId } }
		});
		if (data) ensemble = data;
		else error = message(failure);
	}

	$effect(() => {
		void ensembleId;
		load();
		api.GET('/instruments').then(({ data }) => (instruments = data ?? []));
	});

	$effect(() => {
		if (ensemble?.canManage && readsLibrary(session.me)) {
			api.GET('/users').then(({ data }) => (users = data ?? []));
		}
	});

	// Library search for works to add, debounced.
	$effect(() => {
		const q = search.trim();
		if (!q) {
			results = [];
			return;
		}
		const timer = setTimeout(async () => {
			const { data } = await api.GET('/works', { params: { query: { q } } });
			results = (data ?? []).filter((w) => !ensemble?.works.some((e) => e.id === w.id));
		}, 200);
		return () => clearTimeout(timer);
	});

	function startEditing() {
		if (!ensemble) return;
		name = ensemble.name;
		description = ensemble.description ?? '';
		editing = true;
	}

	async function saveDetails(event: SubmitEvent) {
		event.preventDefault();
		const { error: failure } = await api.PUT('/ensembles/{ensembleId}', {
			params: { path: { ensembleId } },
			body: { name, description: description || null }
		});
		if (failure) error = message(failure);
		else editing = false;
		await load();
	}

	async function remove() {
		if (
			!ensemble ||
			!(await confirm({
				title: t('Delete the ensemble “{name}”? Works and people stay.', { name: ensemble.name }),
				confirm: t('Delete'),
				danger: true
			}))
		)
			return;
		const { error: failure } = await api.DELETE('/ensembles/{ensembleId}', {
			params: { path: { ensembleId } }
		});
		if (failure) error = message(failure);
		else await goto(resolve('/ensembles'));
	}

	async function saveMember(userId: string, leader: boolean, plays: PlayedPart[]) {
		error = '';
		const { error: failure } = await api.PUT('/ensembles/{ensembleId}/members/{userId}', {
			params: { path: { ensembleId, userId } },
			body: { leader, plays }
		});
		if (failure) error = message(failure);
		await load();
	}

	async function removeMember(userId: string, memberName: string) {
		if (
			!(await confirm({
				title: t('Remove {name} from this ensemble?', { name: memberName }),
				confirm: t('Remove'),
				danger: true
			}))
		)
			return;
		const { error: failure } = await api.DELETE('/ensembles/{ensembleId}/members/{userId}', {
			params: { path: { ensembleId, userId } }
		});
		if (failure) error = message(failure);
		await load();
	}

	async function addMember(event: SubmitEvent) {
		event.preventDefault();
		if (!newMember) return;
		await saveMember(newMember, false, []);
		newMember = '';
	}

	let programName = $state('');
	let programDate = $state('');

	async function createProgram(event: SubmitEvent) {
		event.preventDefault();
		const { error: failure } = await api.POST('/ensembles/{ensembleId}/programs', {
			params: { path: { ensembleId } },
			body: { name: programName, date: programDate || null }
		});
		if (failure) error = message(failure);
		else programName = programDate = '';
		await load();
	}

	async function addWork(workId: string) {
		const { error: failure } = await api.PUT('/ensembles/{ensembleId}/works/{workId}', {
			params: { path: { ensembleId, workId } }
		});
		if (failure) error = message(failure);
		search = '';
		await load();
	}

	async function removeWork(workId: string) {
		const { error: failure } = await api.DELETE('/ensembles/{ensembleId}/works/{workId}', {
			params: { path: { ensembleId, workId } }
		});
		if (failure) error = message(failure);
		await load();
	}

	function instrumentLabel(part: PlayedPart) {
		const name = t(instruments.find((i) => i.id === part.instrumentId)?.name ?? '?');
		return part.voice ? `${name} ${part.voice}` : name;
	}
</script>

<p><a href={resolve('/ensembles')}>← {t('Ensembles')}</a></p>
{#if error}<p class="error">{error}</p>{/if}

{#if ensemble}
	{#if editing}
		<form class="stack narrow" onsubmit={saveDetails}>
			<label>{t('Name')} <input bind:value={name} required maxlength="120" /></label>
			<label>{t('Description')} <input bind:value={description} /></label>
			<div class="row">
				<button class="primary">{t('Save')}</button>
				<button type="button" onclick={() => (editing = false)}>{t('Cancel')}</button>
				{#if staff}<button type="button" class="danger" onclick={remove}
						>{t('Delete ensemble')}</button
					>{/if}
			</div>
		</form>
	{:else}
		<header class="title">
			<div class="named">
				<Avatar
					name={ensemble.name}
					url={ensemble.logoUrl}
					symbol={ensemble.symbol}
					color={ensemble.color}
					size={48}
					square
				/>
				<h1>{ensemble.name}</h1>
			</div>
			{#if ensemble.canManage}<button onclick={startEditing}>{t('Edit')}</button>{/if}
		</header>
		{#if ensemble.description}<p class="muted">{ensemble.description}</p>{/if}
		{#if ensemble.canManage}
			<details class="appearance">
				<summary>{t('Appearance')}</summary>
				<EnsembleLook {ensemble} onchange={load} />
			</details>
		{/if}
	{/if}

	{#if !ensemble.canManage && me}
		<p>
			{t('You play:')}
			{me.plays.length
				? me.plays.map(instrumentLabel).join(', ')
				: t('nothing assigned yet – ask your leader.')}
		</p>
	{/if}

	<section>
		<h2>{t('Works')}</h2>
		{#if ensemble.works.length === 0}
			<p class="muted">{t('No works yet.')}</p>
		{:else}
			<ul class="works">
				{#each ensemble.works as work (work.id)}
					<li>
						{#if readsLibrary(session.me)}
							<a href={resolve('/works/[id]', { id: work.id })}>{work.title}</a>
						{:else}
							{work.title}
						{/if}
						{#if work.composer}<span class="muted">· {work.composer}</span>{/if}
						{#if ensemble.canManage}
							<button class="link danger" onclick={() => removeWork(work.id)}>{t('Remove')}</button>
						{/if}
					</li>
				{/each}
			</ul>
		{/if}
		{#if ensemble.canManage && readsLibrary(session.me)}
			<div class="search">
				<input type="search" placeholder={t('Add a work from the library …')} bind:value={search} />
				{#if results.length}
					<ul class="results">
						{#each results as work (work.id)}
							<li>
								<button class="link" onclick={() => addWork(work.id)}>
									+ {work.title}{work.composer ? ` · ${work.composer}` : ''}
								</button>
								<span class="muted">{plural(work.partCount, '{n} part', '{n} parts')}</span>
							</li>
						{/each}
					</ul>
				{/if}
			</div>
		{/if}
	</section>

	<section>
		<h2>{t('Programmes')}</h2>
		{#if ensemble.programs.length === 0}
			<p class="muted">{t('No programmes yet.')}</p>
		{/if}
		{#each ensemble.programs as program (program.id)}
			<ProgramCard {program} {ensemble} onchange={load} />
		{/each}
		{#if ensemble.canManage}
			<form class="row add" onsubmit={createProgram}>
				<input
					bind:value={programName}
					required
					maxlength="120"
					placeholder={t('New programme, e.g. Spring concert')}
				/>
				<input type="date" bind:value={programDate} aria-label={t('Date')} />
				<button disabled={!programName.trim()}>{t('Create')}</button>
			</form>
		{/if}
	</section>

	{#if ensemble.canManage}
		<section>
			<h2>{t('Guest links')}</h2>
			<GuestLinks {ensemble} {instruments} />
		</section>

		<section>
			<h2>{t('Members')}</h2>
			<p class="muted">
				{t(
					"Members see the parts of this ensemble's works for the instruments they play here – all part numbers, or just the one given. Leaders see every part, including scores."
				)}
			</p>
			<table>
				<tbody>
					{#each ensemble.members as member (member.userId)}
						<MemberRow
							{member}
							{instruments}
							{staff}
							onsave={(leader, plays) => saveMember(member.userId, leader, plays)}
							onremove={() => removeMember(member.userId, member.name)}
						/>
					{/each}
				</tbody>
			</table>
			<form class="row add" onsubmit={addMember}>
				<Select
					bind:value={newMember}
					label={t('Person to add')}
					placeholder={t('Add a person …')}
					options={candidates.map((u) => ({ value: u.id, label: u.name }))}
				/>
				<button disabled={!newMember}>{t('Add')}</button>
				{#if session.me?.role === 'admin'}
					<a href={resolve('/people')}>{t('Invite someone new')}</a>
				{/if}
			</form>
		</section>
		<GrantsEditor {ensemble} />
	{/if}
{:else if !error}
	<p class="muted">{t('Loading…')}</p>
{/if}

<style>
	.named {
		display: flex;
		align-items: center;
		gap: 0.75rem;
	}
	.appearance {
		margin: 0.5rem 0 1rem;
	}
	.appearance summary {
		color: var(--muted);
	}
	.appearance[open] summary {
		margin-bottom: 0.75rem;
	}
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
	h2 {
		margin-top: 2rem;
		font-size: 1.2rem;
	}
	.row {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
		align-items: center;
	}
	.add {
		margin-top: 0.75rem;
	}
	.add input:not([type='date']) {
		flex: 1 1 16rem;
	}
	.works,
	.results {
		list-style: none;
		padding: 0;
		margin: 0;
	}
	.works li {
		display: flex;
		gap: 0.5rem;
		align-items: baseline;
		padding: 0.4rem 0;
		border-bottom: 1px solid var(--line);
	}
	.works li button {
		margin-left: auto;
	}
	.search {
		margin-top: 0.75rem;
	}
	.search input {
		width: 100%;
		max-width: 30rem;
	}
	.results li {
		display: flex;
		gap: 0.75rem;
		padding: 0.3rem 0;
	}
	table {
		width: 100%;
		border-collapse: collapse;
	}
</style>
