<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import { untrack } from 'svelte';
	import { page } from '$app/state';
	import { api, message } from '#lib/api/index.js';
	import { i18n, locales, setLocale, t, type Locale } from '#lib/i18n/index.svelte.js';
	import { loadSession, roleNames, session, signedIn } from '#lib/session.svelte.js';
	import { setTheme, theme, type Theme } from '#lib/theme.svelte.js';
	import { download } from '#lib/download.js';
	import ImageField from '#lib/components/ImageField.svelte';
	import { kindNames } from '#lib/notifications.svelte.js';
	import { notify } from '#lib/dialog.svelte.js';
	import type { NotificationSetting } from '#lib/api/index.js';

	const welcome = $derived(page.url.searchParams.has('welcome'));
	const themes = $derived<[Theme, string][]>([
		['system', t('As on this device')],
		['light', t('Light')],
		['dark', t('Dark')]
	]);
	let name = $state(untrack(() => session.me?.name ?? ''));
	let currentPassword = $state('');
	let password = $state('');
	let error = $state('');
	let saved = $state(false);
	let busy = $state(false);

	let channels = $state<NotificationSetting[]>([]);
	$effect(() => {
		api.GET('/notifications/settings').then(({ data }) => (channels = data ?? []));
	});
	async function saveChannels() {
		const { data, error: failure } = await api.PUT('/notifications/settings', { body: channels });
		if (data) {
			channels = data;
			notify(t('Saved.'));
		} else notify(message(failure), 'error');
	}

	async function exportData() {
		const { data } = await api.GET('/me/export');
		if (data) download('notenwart-data.json', JSON.stringify(data, null, 2), 'application/json');
	}

	async function save(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		error = '';
		saved = false;
		const { data, error: failure } = await api.PUT('/me', {
			body: { name, password: password || null, currentPassword: currentPassword || null }
		});
		busy = false;
		if (data) {
			signedIn(data);
			password = currentPassword = '';
			saved = true;
		} else error = message(failure);
	}
</script>

{#if session.me}
	<div class="narrow">
		<h1>{t('Your account')}</h1>
		{#if welcome && !session.me.hasPassword}
			<p>
				<strong>{t('Welcome!')}</strong>
				{t('Choose a password so you can sign in again later.')}
			</p>
		{/if}
		<p class="muted">
			{session.me.email} · {t('{role} in {organization}', {
				role: t(roleNames[session.me.role]),
				organization: session.me.organization.name
			})}
		</p>
		<form class="stack" onsubmit={save}>
			<label>
				{t('Name')}
				<input bind:value={name} required maxlength="120" autocomplete="name" />
			</label>
			{#if session.me.hasPassword}
				<label>
					{t('Current password')}
					<input type="password" bind:value={currentPassword} autocomplete="current-password" />
				</label>
			{/if}
			<label>
				{session.me.hasPassword
					? t('New password (at least 10 characters)')
					: t('Password (at least 10 characters)')}
				<input type="password" bind:value={password} minlength="10" autocomplete="new-password" />
			</label>
			{#if error}<p class="error">{error}</p>{/if}
			{#if saved}<p class="muted">{t('Saved.')}</p>{/if}
			<button class="primary" disabled={busy}>{t('Save')}</button>
		</form>

		{#if session.me.organization.profilePictures}
			<h2>{t('Picture')}</h2>
			<ImageField
				name={session.me.name}
				url={session.me.avatarUrl}
				endpoint="/api/me/avatar"
				onchange={() => loadSession()}
			/>
			<p class="muted small">
				{t('Only you, your leaders and the administration see it; otherwise your initials show.')}
			</p>
		{/if}

		<h2>{t('Colours')}</h2>
		<div class="languages" role="group" aria-label={t('Colours')}>
			{#each themes as [value, label] (value)}
				<button aria-pressed={theme.value === value} onclick={() => setTheme(value)}>{label}</button
				>
			{/each}
		</div>

		<h2>{t('Notifications')}</h2>
		<table class="channels">
			<thead>
				<tr>
					<th></th>
					<th>{t('In the app')}</th>
					<th>{t('By email')}</th>
					<th>{t('Push')}</th>
				</tr>
			</thead>
			<tbody>
				{#each channels as setting (setting.kind)}
					<tr>
						<td>{t(kindNames[setting.kind])}</td>
						<td>
							<input
								type="checkbox"
								bind:checked={setting.inApp}
								onchange={saveChannels}
								aria-label={`${t(kindNames[setting.kind])}: ${t('In the app')}`}
							/>
						</td>
						<td>
							<input
								type="checkbox"
								bind:checked={setting.email}
								onchange={saveChannels}
								aria-label={`${t(kindNames[setting.kind])}: ${t('By email')}`}
							/>
						</td>
						<td>
							<input
								type="checkbox"
								checked={false}
								disabled
								title={t('Coming soon')}
								aria-label={`${t(kindNames[setting.kind])}: ${t('Push')}`}
							/>
						</td>
					</tr>
				{/each}
			</tbody>
		</table>
		<p class="muted small">
			{session.status?.mailEnabled
				? t('Emails are collected for a few minutes and come as one message.')
				: t('This server sends no email yet.')}
			{t('Push messages on phones are coming soon.')}
		</p>

		<h2>{t('Your data')}</h2>
		<p class="muted">
			{t(
				'Everything notenwart stores about you, as a file. To have your account deleted, ask an admin.'
			)}
		</p>
		<button onclick={exportData}>{t('Download my data')}</button>

		<h2>{t('Language')}</h2>
		<div class="languages" role="group" aria-label={t('Language')}>
			{#each Object.entries(locales) as [code, label] (code)}
				<button aria-pressed={i18n.locale === code} onclick={() => setLocale(code as Locale)}>
					{label}
				</button>
			{/each}
		</div>
	</div>
{/if}

<style>
	h2 {
		margin-top: 2rem;
		font-size: 1.2rem;
	}
	.channels {
		border-collapse: collapse;
	}
	.channels th,
	.channels td {
		padding: 0.35rem 0.75rem 0.35rem 0;
		text-align: left;
	}
	.channels th {
		font-size: 0.8em;
		color: var(--muted);
	}
	.channels td:not(:first-child),
	.channels th:not(:first-child) {
		text-align: center;
	}
	.small {
		font-size: 0.85em;
	}
	.languages {
		display: flex;
		gap: 0.5rem;
	}
	.languages button[aria-pressed='true'] {
		background: var(--accent);
		border-color: var(--accent);
		color: var(--accent-text);
	}
</style>
