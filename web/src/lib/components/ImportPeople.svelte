<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import { api, message, type ImportResult } from '#lib/api/index.js';
	import { decode, parse, toCsv, toPeople, type PersonRow } from '#lib/csv.js';
	import { download } from '#lib/download.js';
	import { i18n, plural, t } from '#lib/i18n/index.svelte.js';

	type Result = ImportResult;

	interface Props {
		/** Called after a successful import, to reload the list of people. */
		onimported: () => void;
	}

	let { onimported }: Props = $props();

	let fileName = $state('');
	let rows = $state<PersonRow[]>([]);
	let problem = $state('');
	let check = $state<Result | null>(null);
	let done = $state<Result | null>(null);
	let busy = $state(false);
	let error = $state('');

	async function choose(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		input.value = '';
		if (!file) return;
		reset();
		fileName = file.name;
		const table = toPeople(parse(decode(new Uint8Array(await file.arrayBuffer()))));
		if (table.problem) {
			problem = t(table.problem);
			return;
		}
		rows = table.rows;
		check = await send(true);
	}

	async function send(dryRun: boolean): Promise<Result | null> {
		busy = true;
		error = '';
		const {
			data,
			error: failure,
			response
		} = await api.POST('/users/import', {
			body: { rows, dryRun }
		});
		busy = false;
		// 422 carries the per-row results, like a dry run.
		const result = data ?? (response.status === 422 ? (failure as unknown as Result) : null);
		if (!result) error = message(failure);
		return result;
	}

	async function run() {
		const result = await send(false);
		if (result?.applied) {
			done = result;
			check = null;
			onimported();
		} else if (result) check = result;
	}

	function reset() {
		fileName = '';
		rows = [];
		problem = '';
		check = null;
		done = null;
		error = '';
	}

	function template() {
		const header =
			i18n.locale === 'de'
				? ['Name', 'E-Mail', 'Rolle', 'Ensemble', 'Instrument', 'Stimme']
				: ['Name', 'Email', 'Role', 'Ensemble', 'Instrument', 'Part'];
		const example =
			i18n.locale === 'de'
				? ['Anna Beispiel', 'anna@example.org', 'Mitglied', 'Blasorchester', 'Trompete', '2']
				: ['Anna Example', 'anna@example.org', 'member', 'Concert band', 'Trumpet', '2'];
		download(
			i18n.locale === 'de' ? 'personen-vorlage.csv' : 'people-template.csv',
			toCsv([header, example])
		);
	}

	function invitations() {
		if (!done) return;
		const lines = done.rows
			.filter((r) => r.invitationUrl)
			.map((r) => {
				const row = rows[r.line - 1];
				return [row.name, row.email, r.invitationUrl ?? ''];
			});
		const header = [t('Name'), t('Email'), t('Invitation link')];
		download(
			i18n.locale === 'de' ? 'einladungen.csv' : 'invitations.csv',
			toCsv([header, ...lines])
		);
	}

	const errorRows = $derived(check?.rows.filter((r) => r.outcome === 'error') ?? []);
	const existing = $derived(check?.rows.filter((r) => r.outcome === 'existing').length ?? 0);
</script>

<section class="import">
	<h2>{t('Import from a table')}</h2>
	<p class="muted">
		{t(
			'A CSV file from Excel, LibreOffice or Numbers with the columns Name and Email, optionally Role, Ensemble, Instrument and Part. One row per person and ensemble.'
		)}
		<button class="link" onclick={template}>{t('Download a template')}</button>
	</p>
	<label class="file">
		<input type="file" accept=".csv,text/csv,text/plain" onchange={choose} />
		<span class="button">{t('Choose CSV file …')}</span>
		{#if fileName}<span class="muted">{fileName}</span>{/if}
	</label>

	{#if problem}<p class="error">{problem}</p>{/if}
	{#if error}<p class="error">{error}</p>{/if}
	{#if busy}<p class="muted">{t('Checking …')}</p>{/if}

	{#if check && !busy}
		<p>
			{plural(rows.length, '{n} row', '{n} rows')} ·
			{plural(check.newPeople, '{n} new person', '{n} new people')} ·
			{plural(existing, '{n} row for people already here', '{n} rows for people already here')}
			{#if check.ensemblesCreated.length}
				· {t('new ensembles: {names}', { names: check.ensemblesCreated.join(', ') })}
			{/if}
		</p>
		{#if errorRows.length}
			<p class="error">
				{plural(
					errorRows.length,
					'{n} row has a problem. Fix the file and choose it again – nothing was imported.',
					'{n} rows have a problem. Fix the file and choose it again – nothing was imported.'
				)}
			</p>
			<table>
				<thead>
					<tr
						><th>{t('Line')}</th><th>{t('Name')}</th><th>{t('Email')}</th><th>{t('Problem')}</th
						></tr
					>
				</thead>
				<tbody>
					{#each errorRows as row (row.line)}
						<tr>
							<td>{row.line + 1}</td>
							<td>{rows[row.line - 1]?.name}</td>
							<td>{rows[row.line - 1]?.email}</td>
							<td class="error">{t(row.message ?? '')}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{:else}
			<div class="actions">
				<button class="primary" onclick={run}>
					{plural(rows.length, 'Import {n} row', 'Import {n} rows')}
				</button>
				<button onclick={reset}>{t('Cancel')}</button>
			</div>
		{/if}
	{/if}

	{#if done}
		<p>
			{t('Imported.')}
			{plural(done.newPeople, '{n} new person invited.', '{n} new people invited.')}
		</p>
		{#if done.newPeople}
			<div class="actions">
				<button class="primary" onclick={invitations}>{t('Download invitation links (CSV)')}</button
				>
				<button onclick={reset}>{t('Close')}</button>
			</div>
			<p class="muted">
				{t(
					'Each link works once for 7 days. Hand them over or send them yourself if this server sends no email.'
				)}
			</p>
		{/if}
	{/if}
</section>

<style>
	.import {
		margin-top: 2.5rem;
		padding-top: 1rem;
		border-top: 1px solid var(--line);
	}
	h2 {
		font-size: 1.2rem;
	}
	.file input {
		display: none;
	}
	.file {
		display: inline-flex;
		align-items: center;
		gap: 0.75rem;
		cursor: pointer;
	}
	.button {
		border: 1px solid var(--line);
		background: var(--surface);
		border-radius: 6px;
		padding: 0.45rem 0.9rem;
	}
	.actions {
		display: flex;
		gap: 0.5rem;
		margin: 0.75rem 0;
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.9em;
	}
	th,
	td {
		text-align: left;
		padding: 0.35rem 0.5rem 0.35rem 0;
		border-bottom: 1px solid var(--line);
	}
</style>
