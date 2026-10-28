<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import {
		api,
		message,
		type EnsembleDetail,
		type EnsemblePart,
		type Grant
	} from '#lib/api/index.js';
	import { t } from '#lib/i18n/index.svelte.js';
	import Select from '#lib/components/Select.svelte';

	interface Props {
		ensemble: EnsembleDetail;
	}

	let { ensemble }: Props = $props();

	let grants = $state<Grant[]>([]);
	let parts = $state<EnsemblePart[]>([]);
	let userId = $state('');
	let partId = $state('');
	let error = $state('');

	async function load() {
		const path = { params: { path: { ensembleId: ensemble.id } } };
		const [g, p] = await Promise.all([
			api.GET('/ensembles/{ensembleId}/grants', path),
			api.GET('/ensembles/{ensembleId}/parts', path)
		]);
		grants = g.data ?? [];
		parts = p.data ?? [];
	}

	$effect(() => {
		// Reload when the ensemble's works change.
		void ensemble.works.length;
		load();
	});

	async function grant(event: SubmitEvent) {
		event.preventDefault();
		error = '';
		const { error: failure } = await api.PUT('/ensembles/{ensembleId}/grants/{partId}/{userId}', {
			params: { path: { ensembleId: ensemble.id, partId, userId } }
		});
		if (failure) error = message(failure);
		else partId = '';
		await load();
	}

	async function revoke(item: Grant) {
		const { error: failure } = await api.DELETE(
			'/ensembles/{ensembleId}/grants/{partId}/{userId}',
			{
				params: { path: { ensembleId: ensemble.id, partId: item.partId, userId: item.userId } }
			}
		);
		if (failure) error = message(failure);
		await load();
	}
</script>

<section>
	<h2>{t('Single parts')}</h2>
	<p class="muted">
		{t(
			'Give one member one more part – for example the full score for an accompanist, or a second part to practise.'
		)}
	</p>
	{#if grants.length}
		<ul class="grants">
			{#each grants as item (item.partId + item.userId)}
				<li>
					<span><strong>{item.userName}</strong> – {item.workTitle}: {item.label}</span>
					<button class="link danger" onclick={() => revoke(item)}>{t('Take back')}</button>
				</li>
			{/each}
		</ul>
	{/if}
	{#if parts.length}
		<form class="row" onsubmit={grant}>
			<Select
				bind:value={userId}
				label={t('Person')}
				placeholder={t('Person …')}
				options={ensemble.members.map((m) => ({ value: m.userId, label: m.name }))}
			/>
			<Select
				bind:value={partId}
				label={t('Part')}
				placeholder={t('Part …')}
				options={parts.map((p) => ({ value: p.id, label: p.label, group: p.workTitle }))}
			/>
			<button disabled={!userId || !partId}>{t('Grant')}</button>
		</form>
	{:else}
		<p class="muted">{t('Give the ensemble a work first.')}</p>
	{/if}
	{#if error}<p class="error">{error}</p>{/if}
</section>

<style>
	h2 {
		margin-top: 2rem;
		font-size: 1.2rem;
	}
	.grants {
		list-style: none;
		padding: 0;
		margin: 0 0 0.75rem;
	}
	.grants li {
		display: flex;
		justify-content: space-between;
		gap: 1rem;
		padding: 0.35rem 0;
		border-bottom: 1px solid var(--line);
	}
	.row {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
	}
</style>
