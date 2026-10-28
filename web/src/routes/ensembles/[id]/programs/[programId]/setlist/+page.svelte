<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
	The set list a performing rights society asks for after a concert (in Germany the GEMA "Musikfolge"): the event and,
	per work, title, composer, lyricist, arranger, publisher, work number and duration. Printable and as CSV; it is not
	the society's own form – the details are typed or pasted into its portal.
-->
<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { api, message, type EnsembleDetail } from '#lib/api/index.js';
	import { formatDate } from '#lib/dates.js';
	import { formatDuration } from '#lib/durations.js';
	import { plural, t } from '#lib/i18n/index.svelte.js';
	import { session } from '#lib/session.svelte.js';
	import { toCsv } from '#lib/csv.js';
	import { download } from '#lib/download.js';

	let ensemble = $state<EnsembleDetail | null>(null);
	let error = $state('');
	const ensembleId = $derived(page.params.id ?? '');
	const program = $derived(ensemble?.programs.find((p) => p.id === page.params.programId));
	const works = $derived.by(() => {
		const byId = new Map((ensemble?.works ?? []).map((w) => [w.id, w]));
		return (program?.workIds ?? []).map((id) => byId.get(id)).filter((w) => w !== undefined);
	});
	/** Works lacking what the list should say. */
	const incomplete = $derived(
		works.filter((w) => !w.composer || w.durationSeconds == null || (w.arranger && !w.publisher))
	);

	$effect(() => {
		api
			.GET('/ensembles/{ensembleId}', { params: { path: { ensembleId } } })
			.then(({ data, error: failure }) => {
				if (data) ensemble = data;
				else error = message(failure);
			});
	});

	function csv() {
		if (!program || !ensemble) return;
		const event = [
			[t('Event'), program.name],
			[t('Date'), program.date ?? ''],
			[t('Starts at'), program.startTime ?? ''],
			[t('Venue'), program.venue ?? ''],
			[t('Organiser'), session.me?.organization.name ?? ''],
			[t('Ensemble'), ensemble.name],
			[]
		];
		const header = [
			t('No.'),
			t('Title'),
			t('Composer'),
			t('Lyricist'),
			t('Arranger'),
			t('Publisher'),
			t('Work number (GEMA)'),
			t('Duration')
		];
		const rows = works.map((w, i) => [
			String(i + 1),
			w.title,
			w.composer ?? '',
			w.lyricist ?? '',
			w.arranger ?? '',
			w.publisher ?? '',
			w.societyNumber ?? '',
			formatDuration(w.durationSeconds)
		]);
		const name = `${t('set-list')}-${program.date ?? ''}-${program.name}`
			.toLowerCase()
			.replace(/[^\p{L}\p{N}]+/gu, '-')
			.replace(/^-|-$/g, '');
		download(`${name}.csv`, toCsv([...event, header, ...rows]));
	}
</script>

<p class="no-print">
	<a href={resolve('/ensembles/[id]', { id: ensembleId })}>← {ensemble?.name ?? t('Ensemble')}</a>
</p>

{#if error}<p class="error">{error}</p>{/if}

{#if program && ensemble}
	<article class="sheet">
		<header>
			<p class="kind">{t('Set list')}</p>
			<h1>{program.name}</h1>
			<dl>
				<dt>{t('Date')}</dt>
				<dd>{program.date ? formatDate(program.date) : '–'}</dd>
				<dt>{t('Starts at')}</dt>
				<dd>{program.startTime ?? '–'}</dd>
				<dt>{t('Venue')}</dt>
				<dd>{program.venue ?? '–'}</dd>
				<dt>{t('Organiser')}</dt>
				<dd>{session.me?.organization.name ?? '–'}</dd>
				<dt>{t('Ensemble')}</dt>
				<dd>{ensemble.name}</dd>
			</dl>
			<div class="actions no-print">
				<button class="primary" onclick={() => window.print()}>{t('Print')}</button>
				<button onclick={csv}>{t('Download as CSV')}</button>
			</div>
			<p class="muted no-print">
				{t(
					'For the performing rights society (e.g. GEMA): print it or transfer the details into its portal. This is not the society’s own form.'
				)}
			</p>
			{#if incomplete.length}
				<p class="warning no-print">
					{plural(
						incomplete.length,
						'{n} work lacks composer, duration or publisher:',
						'{n} works lack composer, duration or publisher:'
					)}
					{#each incomplete as work, i (work.id)}{i ? ', ' : ' '}<a
							href={resolve('/works/[id]', { id: work.id })}>{work.title}</a
						>{/each}
				</p>
			{/if}
		</header>
		<table>
			<thead>
				<tr>
					<th>{t('No.')}</th>
					<th>{t('Title')}</th>
					<th>{t('Composer')}</th>
					<th>{t('Lyricist')}</th>
					<th>{t('Arranger')}</th>
					<th>{t('Publisher')}</th>
					<th>{t('Work number (GEMA)')}</th>
					<th class="right">{t('Duration')}</th>
				</tr>
			</thead>
			<tbody>
				{#each works as work, i (work.id)}
					<tr>
						<td>{i + 1}</td>
						<td><strong>{work.title}</strong></td>
						<td>{work.composer ?? ''}</td>
						<td>{work.lyricist ?? ''}</td>
						<td>{work.arranger ?? ''}</td>
						<td>{work.publisher ?? ''}</td>
						<td>{work.societyNumber ?? ''}</td>
						<td class="right">{formatDuration(work.durationSeconds)}</td>
					</tr>
				{:else}
					<tr><td colspan="8" class="muted">{t('No works in this programme yet.')}</td></tr>
				{/each}
			</tbody>
		</table>
	</article>
{:else if !error && ensemble}
	<p class="error">{t('not found')}</p>
{:else if !error}
	<p class="muted">{t('Loading…')}</p>
{/if}

<style>
	.kind {
		margin: 0;
		color: var(--muted);
		text-transform: uppercase;
		letter-spacing: 0.05em;
		font-size: 0.85em;
	}
	h1 {
		margin: 0.1rem 0 0.75rem;
	}
	dl {
		display: grid;
		grid-template-columns: max-content 1fr;
		gap: 0.2rem 1rem;
		margin: 0 0 1rem;
	}
	dt {
		color: var(--muted);
	}
	dd {
		margin: 0;
	}
	.actions {
		display: flex;
		gap: 0.5rem;
		margin-bottom: 0.5rem;
	}
	.warning {
		background: var(--danger-bg);
		padding: 0.5rem 0.75rem;
		border-radius: var(--radius);
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.95em;
	}
	th,
	td {
		text-align: left;
		padding: 0.4rem 0.45rem;
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
	}
	@media print {
		table {
			font-size: 0.85em;
		}
	}
</style>
