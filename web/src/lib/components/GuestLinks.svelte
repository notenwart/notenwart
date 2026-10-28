<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
	Guest links of an ensemble: a substitute gets chosen parts without an account until a date. Parts are picked one by
	one, or all of an instrument (and part number) in a programme at once.
-->
<script lang="ts">
	import {
		api,
		message,
		type EnsembleDetail,
		type EnsemblePart,
		type GuestLink,
		type Instrument
	} from '#lib/api/index.js';
	import { formatDate } from '#lib/dates.js';
	import { i18n, plural, t } from '#lib/i18n/index.svelte.js';
	import { confirm, notify } from '#lib/dialog.svelte.js';
	import { instrumentOptions } from '#lib/instruments.js';
	import { today } from '#lib/loans.js';
	import Icon from './Icon.svelte';
	import Select from './Select.svelte';

	interface Props {
		ensemble: EnsembleDetail;
		instruments: Instrument[];
	}

	let { ensemble, instruments }: Props = $props();

	let links = $state<GuestLink[]>([]);
	let parts = $state<EnsemblePart[]>([]);
	let creating = $state(false);
	let name = $state('');
	let expiresOn = $state('');
	let chosen = $state<string[]>([]);
	let pick = $state('');
	let fromProgram = $state('');
	let fromInstrument = $state('');
	let fromVoice = $state<number | null>(null);
	let created = $state<GuestLink | null>(null);

	const partOptions = $derived(
		parts
			.filter((p) => !chosen.includes(p.id))
			.map((p) => ({ value: p.id, label: p.label, group: p.workTitle }))
	);
	const byId = $derived(new Map(parts.map((p) => [p.id, p])));

	async function load() {
		const [l, p] = await Promise.all([
			api.GET('/ensembles/{ensembleId}/guest-links', {
				params: { path: { ensembleId: ensemble.id } }
			}),
			api.GET('/ensembles/{ensembleId}/parts', { params: { path: { ensembleId: ensemble.id } } })
		]);
		links = l.data ?? [];
		parts = p.data ?? [];
	}

	$effect(() => {
		void ensemble.id;
		load();
	});

	/** A day after the next dated programme, else in two weeks. */
	function defaultExpiry() {
		const next = ensemble.programs
			.map((p) => p.date)
			.filter((d): d is string => !!d && d >= today())[0];
		const base = next ? Date.parse(`${next}T12:00:00`) : Date.now();
		return today(new Date(base + (next ? 1 : 14) * 24 * 3600 * 1000));
	}

	function start() {
		creating = true;
		created = null;
		name = '';
		chosen = [];
		expiresOn = defaultExpiry();
	}

	/** Adds all parts of an instrument (and number) in a programme's works. */
	function addFromProgram() {
		const program = ensemble.programs.find((p) => p.id === fromProgram);
		if (!program || !fromInstrument) return;
		const matching = parts.filter(
			(p) =>
				program.workIds.includes(p.workId) &&
				p.instrumentId === fromInstrument &&
				(!fromVoice || !p.voice || p.voice === fromVoice)
		);
		chosen = [...new Set([...chosen, ...matching.map((p) => p.id)])];
		if (!matching.length) notify(t('No such parts in this programme.'), 'error');
	}

	async function create(event: SubmitEvent) {
		event.preventDefault();
		const { data, error } = await api.POST('/ensembles/{ensembleId}/guest-links', {
			params: { path: { ensembleId: ensemble.id } },
			body: { name, partIds: chosen, expiresOn }
		});
		if (!data) return notify(message(error), 'error');
		created = data;
		creating = false;
		await load();
	}

	async function copy(url: string) {
		await navigator.clipboard.writeText(url);
		notify(t('Copied'));
	}

	async function revoke(link: GuestLink) {
		const ok = await confirm({
			title: t('Stop the guest link for {name}?', { name: link.name }),
			message: t('The link stops working at once.'),
			confirm: t('Stop link'),
			danger: true
		});
		if (!ok) return;
		const { error } = await api.DELETE('/guest-links/{linkId}', {
			params: { path: { linkId: link.id } }
		});
		if (error) notify(message(error), 'error');
		await load();
	}
</script>

<p class="muted">
	{t(
		'For substitutes without an account: a link to chosen parts that works until a date. Only you see the link, once – pass it on yourself.'
	)}
</p>

{#if created?.url}
	<div class="created">
		<p><strong>{t('Guest link for {name}', { name: created.name })}</strong></p>
		<div class="row">
			<input readonly value={created.url} aria-label={t('Guest link')} />
			<button onclick={() => created?.url && copy(created.url)}>{t('Copy')}</button>
		</div>
		<p class="muted small">{t('It is shown only now. Lost it? Stop it and make a new one.')}</p>
	</div>
{/if}

{#if links.length}
	<table class="links">
		<tbody>
			{#each links as link (link.id)}
				<tr class:off={link.revoked || link.expiresOn < today()}>
					<td><strong>{link.name}</strong></td>
					<td>{plural(link.partIds.length, '{n} part', '{n} parts')}</td>
					<td>
						{link.revoked
							? t('stopped')
							: link.expiresOn < today()
								? t('expired')
								: t('until {date}', { date: formatDate(link.expiresOn) })}
					</td>
					<td class="muted">
						{link.lastUsedAt
							? t('last opened {date}', {
									date: new Date(link.lastUsedAt).toLocaleDateString(i18n.locale)
								})
							: t('not opened yet')}
					</td>
					<td class="row-actions">
						{#if !link.revoked && link.expiresOn >= today()}
							<button class="link danger" onclick={() => revoke(link)}>{t('Stop link')}</button>
						{/if}
					</td>
				</tr>
			{/each}
		</tbody>
	</table>
{/if}

{#if creating}
	<form class="new" onsubmit={create}>
		<div class="row">
			<label class="grow">
				{t('For whom')}
				<input
					bind:value={name}
					required
					maxlength="120"
					placeholder={t('e.g. Max, trumpet substitute')}
				/>
			</label>
			<label>
				{t('Works until')}
				<input type="date" bind:value={expiresOn} min={today()} required />
			</label>
		</div>

		{#if ensemble.programs.length}
			<div class="row quick">
				<span class="muted">{t('All parts of')}</span>
				<Select
					bind:value={fromInstrument}
					label={t('Instrument')}
					placeholder={t('Instrument …')}
					options={instrumentOptions(instruments)}
				/>
				<input
					class="voice"
					type="number"
					min="1"
					max="99"
					bind:value={fromVoice}
					placeholder={t('No.')}
					aria-label={t('Part number')}
				/>
				<span class="muted">{t('in')}</span>
				<Select
					bind:value={fromProgram}
					label={t('Programme')}
					placeholder={t('Programme …')}
					options={ensemble.programs.map((p) => ({ value: p.id, label: p.name }))}
				/>
				<button type="button" disabled={!fromProgram || !fromInstrument} onclick={addFromProgram}
					>{t('Add')}</button
				>
			</div>
		{/if}

		<div class="row">
			<Select
				bind:value={pick}
				label={t('Part')}
				placeholder={t('Add a single part …')}
				options={partOptions}
				onchange={(v) => {
					if (v) chosen = [...chosen, v];
					pick = '';
				}}
			/>
		</div>

		{#if chosen.length}
			<ul class="chosen">
				{#each chosen as id (id)}
					<li>
						{byId.get(id)?.workTitle} – {byId.get(id)?.label}
						<button
							type="button"
							class="icon"
							aria-label={t('Remove')}
							onclick={() => (chosen = chosen.filter((c) => c !== id))}
							><Icon name="x" size={14} /></button
						>
					</li>
				{/each}
			</ul>
		{/if}

		<div class="row">
			<button class="primary" disabled={!name.trim() || !chosen.length || !expiresOn}>
				{plural(chosen.length, 'Make a link to {n} part', 'Make a link to {n} parts')}
			</button>
			<button type="button" onclick={() => (creating = false)}>{t('Cancel')}</button>
		</div>
	</form>
{:else}
	<button onclick={start}>{t('New guest link …')}</button>
{/if}

<style>
	.created {
		background: var(--selected);
		border-radius: var(--radius);
		padding: 0.6rem 0.9rem;
		margin-bottom: 0.75rem;
	}
	.created p {
		margin: 0.2rem 0;
	}
	.created input {
		flex: 1;
		font-family: ui-monospace, monospace;
		font-size: 0.85em;
	}
	.links {
		width: 100%;
		border-collapse: collapse;
		margin-bottom: 0.75rem;
	}
	.links td {
		padding: 0.4rem 0.5rem 0.4rem 0;
		border-bottom: 1px solid var(--line);
	}
	.links tr.off td {
		color: var(--muted);
	}
	.new {
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
		padding: 0.75rem;
		border: 1px solid var(--line);
		border-radius: var(--radius);
	}
	.row {
		display: flex;
		flex-wrap: wrap;
		align-items: end;
		gap: 0.5rem;
	}
	.quick {
		align-items: center;
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
	.grow {
		flex: 1 1 14rem;
	}
	.voice {
		width: 4.5rem;
	}
	.chosen {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-wrap: wrap;
		gap: 0.35rem;
	}
	.chosen li {
		display: inline-flex;
		align-items: center;
		gap: 0.2rem;
		padding: 0.15rem 0.2rem 0.15rem 0.6rem;
		border: 1px solid var(--line);
		border-radius: 999px;
		background: var(--surface);
		font-size: 0.9em;
	}
	.small {
		font-size: 0.85em;
	}
</style>
