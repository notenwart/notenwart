<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
	A table with sorting, column choice, a quick filter, pages and row selection. Sorting and filtering happen in the
	browser (lists of a few thousand rows). Column choice, sort and page size are remembered per table on the device.
-->
<script lang="ts" module>
	export interface Column<T> {
		id: string;
		label: string;
		/** Value for sorting and filtering, and the cell's text unless the `cell` snippet renders it. */
		value: (row: T) => string | number | null | undefined;
		sortable?: boolean;
		/** Hidden until the user shows it. */
		hidden?: boolean;
		align?: 'left' | 'right';
		/** Narrow screens hide this column (still sortable through the column menu). */
		wide?: boolean;
	}
</script>

<script lang="ts" generics="T">
	import { untrack, type Snippet } from 'svelte';
	import { plural, t } from '#lib/i18n/index.svelte.js';
	import { matches } from '#lib/search.js';
	import Icon from './Icon.svelte';
	import Select from './Select.svelte';

	interface Props {
		/** Key for the remembered settings. */
		id: string;
		rows: T[];
		key: (row: T) => string;
		columns: Column<T>[];
		/** Custom cell content by column id; falls back to the column's value. */
		cell?: Snippet<[T, Column<T>]>;
		/** Ids of selected rows; selection checkboxes appear when this is bound. */
		selected?: string[];
		selectable?: (row: T) => boolean;
		/** Shown above the table while rows are selected. */
		bulk?: Snippet<[string[]]>;
		/** Shown at the right of the toolbar. */
		actions?: Snippet;
		empty?: string;
		filterPlaceholder?: string;
	}

	let {
		id,
		rows,
		key,
		columns,
		cell,
		selected = $bindable(),
		selectable = () => true,
		bulk,
		actions,
		empty = '',
		filterPlaceholder
	}: Props = $props();

	interface Settings {
		hidden: string[];
		shown: string[];
		sort: string | null;
		desc: boolean;
		size: number;
	}
	// The table's id names where its settings are kept; it does not change.
	const STORE = untrack(() => `notenwart.table.${id}`);
	function load(): Settings {
		try {
			const saved = JSON.parse(localStorage.getItem(STORE) ?? 'null');
			if (saved) return saved;
		} catch {
			// Nothing remembered.
		}
		return { hidden: [], shown: [], sort: null, desc: false, size: 25 };
	}
	let settings = $state<Settings>(load());
	$effect(() => {
		const snapshot = JSON.stringify(settings);
		try {
			localStorage.setItem(STORE, snapshot);
		} catch {
			// Private mode.
		}
	});

	let filter = $state('');
	let page = $state(0);
	let menu = $state(false);

	const visible = $derived(
		columns.filter((c) =>
			c.hidden ? settings.shown.includes(c.id) : !settings.hidden.includes(c.id)
		)
	);
	const filtered = $derived(
		filter.trim()
			? rows.filter((row) => matches(visible.map((c) => c.value(row) ?? '').join(' '), filter))
			: rows
	);
	const sorted = $derived.by(() => {
		const column = columns.find((c) => c.id === settings.sort);
		if (!column) return filtered;
		const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });
		const direction = settings.desc ? -1 : 1;
		return [...filtered].sort((a, b) => {
			const x = column.value(a);
			const y = column.value(b);
			if (x === y) return 0;
			if (x === null || x === undefined || x === '') return 1;
			if (y === null || y === undefined || y === '') return -1;
			if (typeof x === 'number' && typeof y === 'number') return (x - y) * direction;
			return collator.compare(String(x), String(y)) * direction;
		});
	});
	const pages = $derived(Math.max(1, Math.ceil(sorted.length / settings.size)));
	const shownRows = $derived(sorted.slice(page * settings.size, (page + 1) * settings.size));

	// Back to the first page when the filter or sort changes; stay in range when rows go away.
	$effect(() => {
		void filter;
		void settings.sort;
		void settings.desc;
		page = 0;
	});
	$effect(() => {
		if (page > pages - 1) page = pages - 1;
	});

	function sortBy(column: Column<T>) {
		if (column.sortable === false) return;
		if (settings.sort !== column.id) settings = { ...settings, sort: column.id, desc: false };
		else if (!settings.desc) settings = { ...settings, desc: true };
		else settings = { ...settings, sort: null, desc: false };
	}

	function toggleColumn(column: Column<T>) {
		const on = visible.includes(column);
		if (column.hidden) {
			settings = {
				...settings,
				shown: on ? settings.shown.filter((c) => c !== column.id) : [...settings.shown, column.id]
			};
		} else {
			settings = {
				...settings,
				hidden: on
					? [...settings.hidden, column.id]
					: settings.hidden.filter((c) => c !== column.id)
			};
		}
	}

	// Selection
	const selectableIds = $derived(sorted.filter(selectable).map(key));
	const pageIds = $derived(shownRows.filter(selectable).map(key));
	const allOnPage = $derived(pageIds.length > 0 && pageIds.every((i) => selected?.includes(i)));
	function toggleRow(rowId: string) {
		if (!selected) return;
		selected = selected.includes(rowId)
			? selected.filter((i) => i !== rowId)
			: [...selected, rowId];
	}
	function togglePage() {
		if (!selected) return;
		selected = allOnPage
			? selected.filter((i) => !pageIds.includes(i))
			: [...new Set([...selected, ...pageIds])];
	}

	$effect(() => {
		if (!menu) return;
		const close = (event: PointerEvent) => {
			if (!(event.target as Element).closest('.columns')) menu = false;
		};
		window.addEventListener('pointerdown', close);
		return () => window.removeEventListener('pointerdown', close);
	});
</script>

<div class="table">
	<div class="toolbar">
		<label class="filter">
			<Icon name="search" size={16} />
			<input
				type="search"
				bind:value={filter}
				placeholder={filterPlaceholder ?? t('Filter …')}
				aria-label={filterPlaceholder ?? t('Filter …')}
			/>
		</label>
		<div class="columns">
			<button class="icon" onclick={() => (menu = !menu)} aria-expanded={menu} title={t('Columns')}>
				<Icon name="columns" label={t('Columns')} />
			</button>
			{#if menu}
				<div class="menu" role="group" aria-label={t('Columns')}>
					{#each columns as column (column.id)}
						<label>
							<input
								type="checkbox"
								checked={visible.includes(column)}
								onchange={() => toggleColumn(column)}
							/>
							{column.label}
						</label>
					{/each}
				</div>
			{/if}
		</div>
		{#if actions}<div class="actions">{@render actions()}</div>{/if}
	</div>

	{#if selected && selected.length && bulk}
		<div class="bulk" role="region" aria-label={t('Selected rows')}>
			<strong>{plural(selected.length, '{n} selected', '{n} selected')}</strong>
			{#if selected.length < selectableIds.length}
				<button class="link" onclick={() => (selected = selectableIds)}>
					{plural(selectableIds.length, 'Select all {n}', 'Select all {n}')}
				</button>
			{/if}
			<button class="link" onclick={() => (selected = [])}>{t('Clear selection')}</button>
			<div class="bulk-actions">{@render bulk(selected)}</div>
		</div>
	{/if}

	<div class="scroll">
		<table>
			<thead>
				<tr>
					{#if selected}
						<th class="check">
							<input
								type="checkbox"
								checked={allOnPage}
								onchange={togglePage}
								aria-label={t('Select all on this page')}
							/>
						</th>
					{/if}
					{#each visible as column (column.id)}
						<th
							class:right={column.align === 'right'}
							class:wide={column.wide}
							aria-sort={settings.sort === column.id
								? settings.desc
									? 'descending'
									: 'ascending'
								: undefined}
						>
							{#if column.sortable === false}
								{column.label}
							{:else}
								<button class="sort" onclick={() => sortBy(column)}>
									{column.label}
									<Icon
										name={settings.sort === column.id
											? settings.desc
												? 'sortDown'
												: 'sortUp'
											: 'sort'}
										size={14}
									/>
								</button>
							{/if}
						</th>
					{/each}
				</tr>
			</thead>
			<tbody>
				{#each shownRows as row (key(row))}
					{@const rowId = key(row)}
					<tr class:selected={selected?.includes(rowId)}>
						{#if selected}
							<td class="check">
								{#if selectable(row)}
									<input
										type="checkbox"
										checked={selected.includes(rowId)}
										onchange={() => toggleRow(rowId)}
										aria-label={t('Select row')}
									/>
								{/if}
							</td>
						{/if}
						{#each visible as column (column.id)}
							<td class:right={column.align === 'right'} class:wide={column.wide}>
								{#if cell}{@render cell(row, column)}{:else}{column.value(row) ?? ''}{/if}
							</td>
						{/each}
					</tr>
				{:else}
					<tr>
						<td class="empty" colspan={visible.length + (selected ? 1 : 0)}>
							{filter ? t('No matches') : empty}
						</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>

	<div class="pager">
		<span class="muted">
			{sorted.length
				? t('{from}–{to} of {total}', {
						from: page * settings.size + 1,
						to: Math.min((page + 1) * settings.size, sorted.length),
						total: sorted.length
					})
				: ''}
		</span>
		<Select
			label={t('Rows per page')}
			value={String(settings.size)}
			options={[25, 50, 100].map((n) => ({ value: String(n), label: t('{n} per page', { n }) }))}
			onchange={(v) => {
				settings = { ...settings, size: Number(v) };
				page = 0;
			}}
		/>
		<button
			class="icon"
			disabled={page === 0}
			onclick={() => page--}
			aria-label={t('Previous page')}
		>
			<Icon name="chevronLeft" />
		</button>
		<span class="muted">{t('Page {page} of {pages}', { page: page + 1, pages })}</span>
		<button
			class="icon"
			disabled={page >= pages - 1}
			onclick={() => page++}
			aria-label={t('Next page')}
		>
			<Icon name="chevronRight" />
		</button>
	</div>
</div>

<style>
	.toolbar {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.5rem;
		margin-bottom: 0.5rem;
	}
	.filter {
		flex: 1 1 14rem;
		display: flex;
		align-items: center;
		gap: 0.4rem;
		border: 1px solid var(--line);
		border-radius: 6px;
		padding: 0 0.6rem;
		color: var(--muted);
		background: var(--bg);
	}
	.filter input {
		flex: 1;
		border: none;
		background: none;
		padding: 0.45rem 0;
		color: var(--text);
	}
	.filter:focus-within {
		border-color: var(--focus);
	}
	.filter input:focus-visible {
		outline: none;
	}
	.columns {
		position: relative;
	}
	.menu {
		position: absolute;
		right: 0;
		top: calc(100% + 4px);
		z-index: 40;
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		min-width: 12rem;
		padding: 0.5rem 0.75rem;
		background: var(--raised);
		border: 1px solid var(--line);
		border-radius: var(--radius);
		box-shadow: var(--shadow);
	}
	.menu label {
		display: flex;
		gap: 0.5rem;
		align-items: center;
		white-space: nowrap;
	}
	.actions {
		display: flex;
		gap: 0.5rem;
		margin-left: auto;
	}
	.bulk {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.5rem 1rem;
		padding: 0.6rem 0.75rem;
		margin-bottom: 0.5rem;
		background: var(--selected);
		border-radius: var(--radius);
	}
	.bulk-actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
		align-items: center;
		margin-left: auto;
	}
	.scroll {
		overflow-x: auto;
	}
	table {
		width: 100%;
		border-collapse: collapse;
	}
	th,
	td {
		text-align: left;
		padding: 0.5rem 0.75rem 0.5rem 0;
		border-bottom: 1px solid var(--line);
		vertical-align: middle;
	}
	th {
		font-size: 0.85em;
		color: var(--muted);
		font-weight: 600;
		white-space: nowrap;
	}
	.right {
		text-align: right;
	}
	.check {
		width: 1.5rem;
		padding-right: 0.5rem;
	}
	.sort {
		display: inline-flex;
		align-items: center;
		gap: 0.25rem;
		padding: 0;
		background: none;
		border: none;
		color: inherit;
		font-weight: inherit;
	}
	.sort:hover:not(:disabled) {
		background: none;
		color: var(--text);
	}
	tbody tr:hover {
		background: var(--hover);
	}
	tr.selected {
		background: var(--selected);
	}
	.empty {
		color: var(--muted);
		padding: 1.5rem 0;
		text-align: center;
	}
	.pager {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: flex-end;
		gap: 0.5rem;
		margin-top: 0.75rem;
		font-size: 0.9em;
	}
	.pager > span:first-child {
		margin-right: auto;
	}
	@media (max-width: 40rem) {
		.wide {
			display: none;
		}
	}
</style>
