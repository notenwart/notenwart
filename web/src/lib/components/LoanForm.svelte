<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Lending printed parts of a work: what, how many, to whom (a person of the organisation or anyone by name), until. -->
<script lang="ts">
	import { api, message, type Borrower } from '#lib/api/index.js';
	import { t } from '#lib/i18n/index.svelte.js';
	import { notify } from '#lib/dialog.svelte.js';
	import Autocomplete from './Autocomplete.svelte';
	import Select from './Select.svelte';

	interface Props {
		workId: string;
		/** Suggestions for what is lent, e.g. the work's part labels. */
		items: string[];
		borrowers: Borrower[];
		onlent: () => void;
	}

	let { workId, items, borrowers, onlent }: Props = $props();

	const OTHER = 'other';
	let item = $state('');
	let quantity = $state(1);
	let borrower = $state('');
	let borrowerName = $state('');
	let dueOn = $state('');
	let busy = $state(false);

	const options = $derived([
		...borrowers.map((b) => ({ value: b.id, label: b.name })),
		{ value: OTHER, label: t('Someone else …') }
	]);

	async function lend(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		const { error } = await api.POST('/works/{workId}/loans', {
			params: { path: { workId } },
			body: {
				item,
				quantity,
				borrowerId: borrower && borrower !== OTHER ? borrower : null,
				borrowerName: borrower === OTHER ? borrowerName : null,
				dueOn: dueOn || null
			}
		});
		busy = false;
		if (error) return notify(message(error), 'error');
		item = '';
		quantity = 1;
		borrower = borrowerName = dueOn = '';
		onlent();
	}
</script>

<form class="lend" onsubmit={lend}>
	<label class="item">
		{t('What')}
		<Autocomplete bind:value={item} suggestions={items} required maxlength={120} />
	</label>
	<label class="quantity">
		{t('Copies')}
		<input type="number" min="1" max="999" bind:value={quantity} />
	</label>
	<label>
		{t('To')}
		<Select bind:value={borrower} label={t('Borrower')} placeholder={t('Person …')} {options} />
	</label>
	{#if borrower === OTHER}
		<label>
			{t('Name')}
			<input bind:value={borrowerName} required maxlength="120" />
		</label>
	{/if}
	<label>
		{t('Until')}
		<input type="date" bind:value={dueOn} />
	</label>
	<button class="primary" disabled={busy || !item.trim() || !borrower}>{t('Lend')}</button>
</form>

<style>
	.lend {
		display: flex;
		flex-wrap: wrap;
		align-items: end;
		gap: 0.6rem;
	}
	label {
		display: flex;
		flex-direction: column;
		gap: 0.2rem;
		font-size: 0.85em;
		color: var(--muted);
	}
	label > :global(*) {
		color: var(--text);
		font-size: 1rem;
	}
	.item {
		flex: 1 1 12rem;
	}
	.quantity input {
		width: 5rem;
	}
</style>
