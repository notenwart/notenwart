<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { api, message, type Ensemble } from '#lib/api/index.js';
	import { isStaff, session } from '#lib/session.svelte.js';
	import { t } from '#lib/i18n/index.svelte.js';
	import DataTable, { type Column } from '#lib/components/DataTable.svelte';
	import Avatar from '#lib/components/Avatar.svelte';

	let ensembles = $state<Ensemble[] | null>(null);
	let error = $state('');
	let name = $state('');
	let busy = $state(false);

	const columns = $derived<Column<Ensemble>[]>([
		{ id: 'name', label: t('Name'), value: (e) => e.name },
		{ id: 'description', label: t('Description'), value: (e) => e.description, wide: true },
		{ id: 'members', label: t('Members'), value: (e) => e.memberCount, align: 'right' },
		{ id: 'works', label: t('Works'), value: (e) => e.workCount, align: 'right' }
	]);

	$effect(() => {
		api.GET('/ensembles').then(({ data, error: failure }) => {
			if (data) ensembles = data;
			else error = message(failure);
		});
	});

	async function create(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		const { data, error: failure } = await api.POST('/ensembles', { body: { name } });
		busy = false;
		if (data) await goto(resolve('/ensembles/[id]', { id: data.id }));
		else error = message(failure);
	}
</script>

<h1>{t('Ensembles')}</h1>

{#if isStaff(session.me)}
	<form class="create" onsubmit={create}>
		<input
			placeholder={t('Name of a new ensemble, class or course')}
			bind:value={name}
			required
			maxlength="120"
		/>
		<button class="primary" disabled={busy || !name.trim()}>{t('Create')}</button>
	</form>
{/if}

{#if error}<p class="error">{error}</p>{/if}

{#if ensembles === null}
	<p class="muted">{t('Loading…')}</p>
{:else if ensembles.length === 0}
	<p class="muted">
		{isStaff(session.me) ? t('No ensembles yet.') : t('You are not in an ensemble yet.')}
	</p>
{:else}
	<DataTable
		id="ensembles"
		rows={ensembles}
		key={(e) => e.id}
		{columns}
		filterPlaceholder={t('Filter …')}
	>
		{#snippet cell(ensemble, column)}
			{#if column.id === 'name'}
				<a class="named" href={resolve('/ensembles/[id]', { id: ensemble.id })}
					><Avatar
						name={ensemble.name}
						url={ensemble.logoUrl}
						symbol={ensemble.symbol}
						color={ensemble.color}
						size={28}
						square
					/><strong>{ensemble.name}</strong></a
				>
			{:else if column.id === 'description'}
				<span class="muted">{ensemble.description ?? ''}</span>
			{:else}
				{column.value(ensemble) ?? ''}
			{/if}
		{/snippet}
	</DataTable>
{/if}

<style>
	.named {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
	}
	.create {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
		margin-bottom: 1rem;
	}
	.create input {
		flex: 1 1 16rem;
	}
	a {
		color: var(--text);
		text-decoration: none;
	}
	a:hover {
		text-decoration: underline;
	}
</style>
