<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- The paper archive's loans: who has which printed parts, overdue ones first in view. -->
<script lang="ts">
	import { resolve } from '#lib/router.svelte.js';
	import { api, message, type Loan } from '#lib/api/index.js';
	import { isStaff, session } from '#lib/session.svelte.js';
	import { i18n, t } from '#lib/i18n/index.svelte.js';
	import { notify } from '#lib/dialog.svelte.js';
	import { overdue, returnLoan } from '#lib/loans.js';
	import { formatDate } from '#lib/dates.js';
	import DataTable, { type Column } from '#lib/components/DataTable.svelte';

	let loans = $state<Loan[]>([]);
	let all = $state(false);
	let error = $state('');
	let selected = $state<string[]>([]);

	const columns = $derived<Column<Loan>[]>([
		{ id: 'work', label: t('Work'), value: (l) => l.workTitle },
		{ id: 'item', label: t('What'), value: (l) => l.item },
		{ id: 'quantity', label: t('Copies'), value: (l) => l.quantity, align: 'right' },
		{ id: 'borrower', label: t('Borrower'), value: (l) => l.borrowerName },
		{ id: 'lent', label: t('Lent on'), value: (l) => l.lentOn, wide: true },
		{ id: 'due', label: t('Until'), value: (l) => l.dueOn },
		{ id: 'returned', label: t('Returned'), value: (l) => l.returnedOn, hidden: !all },
		{ id: 'note', label: t('Note'), value: (l) => l.note, hidden: true, wide: true },
		{ id: 'actions', label: '', value: () => '', sortable: false, align: 'right' }
	]);

	async function load() {
		const { data, error: failure } = await api.GET('/loans', {
			params: { query: { show: all ? 'all' : 'open' } }
		});
		if (data) loans = data;
		else error = message(failure);
		selected = selected.filter((id) => loans.some((l) => l.id === id));
	}

	$effect(() => {
		void all;
		load();
	});

	async function giveBack(list: Loan[]) {
		for (const loan of list) {
			const { error: failure } = await returnLoan(loan);
			if (failure) return notify(message(failure), 'error');
		}
		notify(t('Marked as returned.'));
		await load();
	}

	function date(iso: string | null | undefined) {
		return iso ? formatDate(iso) : '';
	}
</script>

<p class="back"><a href={resolve('/works')}>← {t('Library')}</a></p>
<h1>{t('Loans')}</h1>

{#if !isStaff(session.me)}
	<p class="muted">{t('Only admins and librarians keep the loans.')}</p>
{:else}
	<p class="muted">
		{t('Printed parts lent out. Lend them from a work’s page, under “Paper archive”.')}
	</p>
	{#if error}<p class="error">{error}</p>{/if}
	<DataTable
		id="loans"
		rows={loans}
		key={(l) => l.id}
		{columns}
		bind:selected
		empty={all ? t('No loans yet.') : t('Nothing is lent out.')}
		filterPlaceholder={t('Filter by work, part, person …')}
	>
		{#snippet actions()}
			<label class="toggle"><input type="checkbox" bind:checked={all} /> {t('Show returned')}</label
			>
		{/snippet}
		{#snippet cell(loan, column)}
			{#if column.id === 'work'}
				<a href={resolve('/works/[id]', { id: loan.workId })}>{loan.workTitle}</a>
			{:else if column.id === 'due'}
				<span class:late={overdue(loan)}>{date(loan.dueOn)}</span>
			{:else if column.id === 'lent' || column.id === 'returned'}
				{date(column.id === 'lent' ? loan.lentOn : loan.returnedOn)}
			{:else if column.id === 'actions'}
				{#if !loan.returnedOn}
					<button onclick={() => giveBack([loan])}>{t('Returned')}</button>
				{/if}
			{:else}
				{column.value(loan) ?? ''}
			{/if}
		{/snippet}
		{#snippet bulk()}
			<button
				onclick={() => giveBack(loans.filter((l) => selected.includes(l.id) && !l.returnedOn))}
				>{t('Mark as returned')}</button
			>
		{/snippet}
	</DataTable>
	<p class="muted small">
		{t('Overdue: {n}', { n: loans.filter((l) => overdue(l)).length })} ·
		{new Date().toLocaleDateString(i18n.locale)}
	</p>
{/if}

<style>
	.back {
		margin: 0;
	}
	.late {
		color: var(--danger);
		font-weight: 600;
	}
	.toggle {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		white-space: nowrap;
	}
	.small {
		font-size: 0.85em;
	}
</style>
