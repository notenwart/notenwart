<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import { t } from '#lib/i18n/index.svelte.js';
	import { page } from '$app/state';
	import { api, message } from '#lib/api/index.js';
	import { session, signedIn } from '#lib/session.svelte.js';
	import Avatar from '#lib/components/Avatar.svelte';

	let email = $state('');
	let password = $state('');
	let error = $state('');
	let sent = $state(false);
	let busy = $state(false);

	/** Only same-site paths, so the link cannot send people elsewhere. */
	function next() {
		const target = page.url.searchParams.get('next') ?? '/';
		return target.startsWith('/') && !target.startsWith('//') ? target : '/';
	}

	$effect(() => {
		if (session.me) location.replace(next());
	});

	// Back from the identity provider without success.
	const ssoError = $derived.by(() => {
		const outcome = page.url.searchParams.get('sso');
		if (outcome === 'unknown')
			return t(
				'Your account is not known here yet. Ask an admin to add you with this email address.'
			);
		if (outcome === 'failed')
			return t('Signing in with {provider} did not work. Please try again.', {
				provider: session.status?.sso ?? ''
			});
		return '';
	});

	async function login(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		error = '';
		const {
			data,
			error: failure,
			response
		} = await api.POST('/auth/login', { body: { email, password } });
		busy = false;
		if (data) {
			signedIn(data);
			location.replace(next());
		} else {
			error = response.status === 401 ? t('Email or password is wrong.') : message(failure);
		}
	}

	async function magicLink() {
		if (!email) {
			error = t('Enter your email address first.');
			return;
		}
		busy = true;
		error = '';
		const { error: failure } = await api.POST('/auth/magic-link', { body: { email } });
		busy = false;
		if (failure) error = message(failure);
		else sent = true;
	}
</script>

<div class="narrow">
	{#if session.status?.organization}
		<p class="organization">
			<Avatar
				name={session.status.organization.name}
				url={session.status.organization.logoUrl}
				size={48}
				square
			/>
			<span>{session.status.organization.name}</span>
		</p>
	{/if}
	<h1>{t('Sign in')}</h1>
	{#if !session.me}
		{#if session.status?.sso}
			<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- served by the API, not a route -->
			<a class="button primary sso" href="/api/auth/sso/start?next={encodeURIComponent(next())}"
				>{t('Sign in with {provider}', { provider: session.status.sso })}</a
			>
			{#if ssoError}<p class="error">{ssoError}</p>{/if}
			<p class="or muted">{t('or with email and password')}</p>
		{/if}
		<form class="stack" onsubmit={login}>
			<label
				>{t('Email')}
				<input type="email" bind:value={email} required autocomplete="username" /></label
			>
			<label>
				{t('Password')}
				<input type="password" bind:value={password} autocomplete="current-password" />
			</label>
			{#if error}<p class="error">{error}</p>{/if}
			<button class="primary" disabled={busy || !password}>{t('Sign in')}</button>
		</form>
		{#if session.status?.mailEnabled}
			<p>
				{t('No password yet, or forgotten it?')}
				<button class="link" onclick={magicLink} disabled={busy}
					>{t('Email me a sign-in link')}</button
				>
			</p>
			{#if sent}<p class="muted">
					{t('If the address is known, a link is on its way. It works for 20 minutes.')}
				</p>{/if}
		{:else}
			<p class="muted">{t('No password yet? Ask your admin for an invitation link.')}</p>
		{/if}
	{/if}
</div>

<style>
	.organization {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		font-weight: 600;
		margin: 0 0 0.5rem;
	}
	.sso {
		display: block;
		text-align: center;
		padding: 0.6rem 1rem;
		margin-bottom: 0.5rem;
	}
	.or {
		text-align: center;
		margin: 0.75rem 0;
	}
</style>
