<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import { resolve } from '#lib/router.svelte.js';
	import { api, message, type Loan, type MyEnsemble, type Program } from '#lib/api/index.js';
	import { formatDate } from '#lib/dates.js';
	import {
		getMeta,
		listParts,
		offlineEnsembles,
		setEnsembleOffline,
		sync,
		type SyncProgress
	} from '#lib/offline.js';
	import { isStaff, session } from '#lib/session.svelte.js';
	import { plural, t } from '#lib/i18n/index.svelte.js';
	import { open as openSteps } from '#lib/setup.js';
	import Avatar from '#lib/components/Avatar.svelte';

	let music = $state<MyEnsemble[] | null>(null);
	let offline = $state(false);
	let error = $state('');
	let enabled = $state<string[]>([]);
	let stored = $state<string[]>([]);
	let progress = $state<SyncProgress | null>(null);
	// Admins: how far setting up has come.
	let setupLeft = $state(0);
	$effect(() => {
		if (session.me?.role !== 'admin') return;
		api
			.GET('/system')
			.then(({ data }) => (setupLeft = data ? openSteps(data.checklist).length : 0));
	});
	// Printed parts the user has borrowed (not when offline).
	let loans = $state<Loan[]>([]);
	$effect(() => {
		api.GET('/me/loans').then(({ data }) => (loans = data ?? []));
	});

	async function refreshStored() {
		enabled = await offlineEnsembles();
		stored = (await listParts()).map((p) => p.id);
	}

	async function load() {
		try {
			const { data, error: failure } = await api.GET('/my/music');
			if (!data) {
				error = message(failure);
				return;
			}
			music = data;
			offline = false;
			await runSync(data);
		} catch {
			// No network: show what this device saved last time.
			music = (await getMeta<MyEnsemble[]>('music')) ?? [];
			offline = true;
			await refreshStored();
		}
	}

	async function runSync(data: MyEnsemble[]) {
		try {
			await sync(data, (p) => (progress = p.total ? p : null));
		} catch (e) {
			error = t('Saving for offline use failed: {reason}', {
				reason: e instanceof Error ? e.message : String(e)
			});
		} finally {
			progress = null;
			await refreshStored();
		}
	}

	$effect(() => {
		load();
	});

	async function toggleOffline(ensemble: MyEnsemble, on: boolean) {
		await setEnsembleOffline(ensemble.id, on);
		if (music) await runSync(music);
	}

	/** The programme's works the user has parts in, in playing order. */
	function programWorks(ensemble: MyEnsemble, program: Program) {
		return program.workIds
			.map((id) => ensemble.works.find((w) => w.id === id))
			.filter((w): w is MyEnsemble['works'][number] => !!w);
	}

	function partCount(ensemble: MyEnsemble) {
		return ensemble.works.reduce((n, w) => n + w.parts.length, 0);
	}

	function storedCount(ensemble: MyEnsemble) {
		return ensemble.works.reduce(
			(n, w) => n + w.parts.filter((p) => stored.includes(p.id)).length,
			0
		);
	}
</script>

<h1>{t('My music')}</h1>
{#if offline}
	<p class="banner">{t('You are offline. Showing the music saved on this device.')}</p>
{/if}
{#if error}<p class="error">{error}</p>{/if}
{#if progress}
	<p class="muted">
		{t('Saving for offline use … {done} of {total}', {
			done: progress.done,
			total: progress.total
		})}
		<progress max={progress.total} value={progress.done}></progress>
	</p>
{/if}

{#if setupLeft}
	<p class="setup">
		{plural(
			setupLeft,
			'{n} step of setting up is still open.',
			'{n} steps of setting up are still open.'
		)}
		<a href={resolve('/system')}>{t('Continue setting up')}</a>
	</p>
{/if}

{#if loans.length}
	<section class="loans">
		<h2>{t('Borrowed from the archive')}</h2>
		<ul>
			{#each loans as loan (loan.id)}
				<li>
					<strong>{loan.workTitle}</strong> – {loan.item}{loan.quantity > 1
						? ` × ${loan.quantity}`
						: ''}
					{#if loan.dueOn}<span class="muted"
							>· {t('please return by {date}', { date: formatDate(loan.dueOn) })}</span
						>{/if}
				</li>
			{/each}
		</ul>
	</section>
{/if}

{#if music === null}
	<p class="muted">{t('Loading…')}</p>
{:else if music.length === 0}
	<p class="muted">
		{offline ? t('Nothing is saved on this device yet.') : t('You are not in an ensemble yet.')}
	</p>
	{#if isStaff(session.me) && !offline}
		<p>
			<a href={resolve('/ensembles')}>{t('Create an ensemble')}</a>
			{t('and add yourself and your players.')}
		</p>
	{/if}
{:else}
	{#each music as ensemble (ensemble.id)}
		{@const isOffline = enabled.includes(ensemble.id)}
		<section>
			<header>
				<h2>
					<Avatar
						name={ensemble.name}
						url={offline ? null : ensemble.logoUrl}
						symbol={ensemble.symbol}
						color={ensemble.color}
						size={30}
						square
					/>
					{#if offline}
						{ensemble.name}
					{:else}
						<a href={resolve('/ensembles/[id]', { id: ensemble.id })}>{ensemble.name}</a>
					{/if}
					{#if ensemble.leader}<span class="badge">{t('You lead')}</span>{/if}
				</h2>
				{#if !offline && partCount(ensemble)}
					<label class="offline-toggle">
						<input
							type="checkbox"
							checked={isOffline}
							onchange={(e) => toggleOffline(ensemble, e.currentTarget.checked)}
						/>
						{t('Available offline')}
						{#if isOffline}<span class="muted">({storedCount(ensemble)}/{partCount(ensemble)})</span
							>{/if}
					</label>
				{/if}
			</header>
			{#if ensemble.works.length === 0}
				<p class="muted">{t('No music for you here yet.')}</p>
			{:else}
				{#each ensemble.programs as program (program.id)}
					{@const works = programWorks(ensemble, program)}
					{#if works.length}
						<h3 class="program">
							{program.name}
							{#if program.date}<span class="muted">· {formatDate(program.date)}</span>{/if}
						</h3>
						{@render workList(ensemble, works, program.id)}
					{/if}
				{/each}
				{#if ensemble.programs.some((p) => programWorks(ensemble, p).length)}
					<h3 class="program">{t('All works')}</h3>
				{/if}
				{@render workList(ensemble, ensemble.works, null)}
			{/if}
		</section>
	{/each}
{/if}

{#snippet workList(ensemble: MyEnsemble, works: MyEnsemble['works'], program: string | null)}
	<ul class="works">
		{#each works as work (work.id)}
			<li>
				<div>
					<strong>{work.title}</strong>
					{#if work.composer}<span class="muted"> · {work.composer}</span>{/if}
				</div>
				<div class="parts">
					{#each work.parts as part (part.id)}
						{@const available = !offline || stored.includes(part.id)}
						{#if available}
							<!-- eslint-disable svelte/no-navigation-without-resolve -- resolved path plus a query -->
							<a
								class="part"
								href={resolve('/view/[id]', { id: part.id }) +
									`?ensemble=${ensemble.id}` +
									(program ? `&program=${program}` : '')}
							>
								{part.label}
								<span class="muted">{t('{n} p.', { n: part.pageCount })}</span>
								{#if stored.includes(part.id)}<span class="saved" title={t('Saved on this device')}
										>●</span
									>{/if}
							</a>
							<!-- eslint-enable svelte/no-navigation-without-resolve -->
						{:else}
							<span class="part unavailable" title={t('Not saved on this device')}
								>{part.label}</span
							>
						{/if}
					{/each}
				</div>
			</li>
		{/each}
	</ul>
{/snippet}

<style>
	.setup {
		background: var(--selected);
		border-radius: var(--radius);
		padding: 0.6rem 0.9rem;
	}
	.setup a {
		margin-left: 0.5rem;
		font-weight: 600;
	}
	.loans ul {
		margin: 0.25rem 0 1rem;
		padding-left: 1.25rem;
	}
	header {
		display: flex;
		flex-wrap: wrap;
		justify-content: space-between;
		align-items: center;
		gap: 0.5rem 1rem;
		margin-top: 2rem;
	}
	h2 {
		font-size: 1.2rem;
		margin: 0;
		display: flex;
		align-items: center;
		gap: 0.75rem;
	}
	h3.program {
		font-family: var(--serif);
		font-size: 1rem;
		margin: 1.25rem 0 0.25rem;
	}
	h2 a {
		color: var(--text);
		text-decoration: none;
	}
	.badge {
		font-size: 0.75rem;
		font-weight: 600;
		background: var(--selected);
		border-radius: 999px;
		padding: 0.15rem 0.6rem;
	}
	.banner {
		background: var(--selected);
		border-radius: var(--radius);
		padding: 0.6rem 0.9rem;
	}
	.offline-toggle {
		display: flex;
		align-items: center;
		gap: 0.4rem;
	}
	.works {
		list-style: none;
		padding: 0;
		margin: 0.5rem 0 0;
		border-top: 1px solid var(--line);
	}
	.works li {
		display: flex;
		flex-wrap: wrap;
		justify-content: space-between;
		gap: 0.5rem 1rem;
		padding: 0.75rem 0;
		border-bottom: 1px solid var(--line);
	}
	.parts {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
	}
	.part {
		border: 1px solid var(--line);
		border-radius: 6px;
		padding: 0.5rem 0.8rem;
		text-decoration: none;
		background: var(--surface);
	}
	.unavailable {
		opacity: 0.5;
	}
	.saved {
		color: #2e7d32;
		font-size: 0.7em;
		vertical-align: middle;
	}
</style>
