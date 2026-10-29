<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- A programme's running order for printing: start times, works with composer and arranger, breaks, durations. -->
<script lang="ts">
	import { page, resolve } from '#lib/router.svelte.js';
	import { api, message, type EnsembleDetail } from '#lib/api/index.js';
	import { formatDate } from '#lib/dates.js';
	import { clockAfter, formatDuration } from '#lib/durations.js';
	import { t } from '#lib/i18n/index.svelte.js';

	let ensemble = $state<EnsembleDetail | null>(null);
	let error = $state('');
	const ensembleId = $derived(page.params.id ?? '');
	const program = $derived(ensemble?.programs.find((p) => p.id === page.params.programId));
	const works = $derived(new Map((ensemble?.works ?? []).map((w) => [w.id, w])));

	$effect(() => {
		api
			.GET('/ensembles/{ensembleId}', { params: { path: { ensembleId } } })
			.then(({ data, error: failure }) => {
				if (data) ensemble = data;
				else error = message(failure);
			});
	});

	const rows = $derived.by(() => {
		if (!program) return [];
		let at = 0;
		let number = 0;
		return program.items.map((item) => {
			const work = item.kind === 'work' ? works.get(item.workId) : undefined;
			const seconds =
				item.kind === 'work' ? (work?.durationSeconds ?? null) : (item.durationSeconds ?? null);
			const row = {
				number: item.kind === 'work' ? ++number : null,
				start: at,
				title: item.kind === 'work' ? (work?.title ?? '?') : item.label,
				by: work
					? [work.composer, work.arranger && t('arr. {name}', { name: work.arranger })]
							.filter(Boolean)
							.join(' · ')
					: '',
				seconds,
				note: item.note ?? '',
				isBreak: item.kind === 'break'
			};
			at += seconds ?? 0;
			return row;
		});
	});
	const total = $derived(rows.reduce((sum, r) => sum + (r.seconds ?? 0), 0));
</script>

<p class="no-print">
	<a href={resolve('/ensembles/[id]', { id: ensembleId })}>← {ensemble?.name ?? t('Ensemble')}</a>
</p>

{#if error}<p class="error">{error}</p>{/if}

{#if program && ensemble}
	<article class="sheet">
		<header>
			<p class="ensemble">{ensemble.name}</p>
			<h1>{program.name}</h1>
			<p class="muted">
				{[
					program.date && formatDate(program.date),
					program.startTime && t('at {time}', { time: program.startTime }),
					program.venue
				]
					.filter(Boolean)
					.join(' · ')}
			</p>
			<button class="primary no-print" onclick={() => window.print()}>{t('Print')}</button>
		</header>
		<table>
			<thead>
				<tr>
					<th>{t('No.')}</th>
					{#if program.startTime}<th>{t('Time')}</th>{/if}
					<th>{t('Work')}</th>
					<th class="right">{t('Duration')}</th>
				</tr>
			</thead>
			<tbody>
				{#each rows as row, i (i)}
					<tr class:break={row.isBreak}>
						<td>{row.number ?? ''}</td>
						{#if program.startTime}<td class="clock">{clockAfter(program.startTime, row.start)}</td
							>{/if}
						<td>
							<strong>{row.title}</strong>
							{#if row.by}<span class="by">{row.by}</span>{/if}
							{#if row.note}<span class="note">{row.note}</span>{/if}
						</td>
						<td class="right">{formatDuration(row.seconds) || '–'}</td>
					</tr>
				{/each}
			</tbody>
			<tfoot>
				<tr>
					<td colspan={program.startTime ? 3 : 2}>
						{program.startTime
							? `${t('Total')} · ${t('ends about {time}', { time: clockAfter(program.startTime, total) })}`
							: t('Total')}
					</td>
					<td class="right">{formatDuration(total)}</td>
				</tr>
			</tfoot>
		</table>
		{#if program.notes}<p class="notes">{program.notes}</p>{/if}
	</article>
{:else if !error && ensemble}
	<p class="error">{t('not found')}</p>
{:else if !error}
	<p class="muted">{t('Loading…')}</p>
{/if}

<style>
	.sheet {
		max-width: 48rem;
	}
	.sheet header {
		margin-bottom: 1rem;
	}
	.ensemble {
		margin: 0;
		color: var(--muted);
		text-transform: uppercase;
		letter-spacing: 0.05em;
		font-size: 0.85em;
	}
	h1 {
		margin: 0.1rem 0;
	}
	table {
		width: 100%;
		border-collapse: collapse;
	}
	th,
	td {
		text-align: left;
		padding: 0.45rem 0.5rem;
		border-bottom: 1px solid var(--line);
		vertical-align: top;
	}
	th {
		font-size: 0.8em;
		color: var(--muted);
	}
	.right {
		text-align: right;
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}
	.clock {
		font-variant-numeric: tabular-nums;
		width: 4.5rem;
	}
	td:first-child,
	th:first-child {
		width: 2.5rem;
	}
	.by,
	.note {
		display: block;
		font-size: 0.9em;
		color: var(--muted);
	}
	.note {
		font-style: italic;
	}
	tr.break td {
		background: var(--surface);
	}
	tfoot td {
		font-weight: 600;
		border-bottom: none;
	}
	.notes {
		white-space: pre-line;
		color: var(--muted);
	}
	@media print {
		tr.break td {
			background: #eee;
		}
		.sheet {
			max-width: none;
		}
	}
</style>
