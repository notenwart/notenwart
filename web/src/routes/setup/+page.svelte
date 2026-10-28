<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import { t } from '#lib/i18n/index.svelte.js';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { api, message } from '#lib/api/index.js';
	import { session, signedIn } from '#lib/session.svelte.js';

	let organizationName = $state('');
	let name = $state('');
	let email = $state('');
	let password = $state('');
	let error = $state('');
	let busy = $state(false);
	let done = false;

	// Someone opening /setup on a server that is set up already.
	$effect(() => {
		if (!done && session.status && !session.status.setupRequired) goto(resolve('/'));
	});

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		const { data, error: failure } = await api.POST('/auth/setup', {
			body: { organizationName, name, email, password }
		});
		busy = false;
		if (data) {
			done = true;
			signedIn(data);
			await goto(resolve('/ensembles'));
		} else error = message(failure);
	}
</script>

<div class="narrow">
	<h1>{t('Welcome to notenwart')}</h1>
	<p>{t('Set up this server: name your organisation and create the first admin account.')}</p>
	<form class="stack" onsubmit={submit}>
		<label>
			{t('Organisation')}
			<input
				bind:value={organizationName}
				required
				maxlength="120"
				placeholder={t('e.g. Music school …')}
			/>
		</label>
		<label
			>{t('Your name')}
			<input bind:value={name} required maxlength="120" autocomplete="name" /></label
		>
		<label
			>{t('Email')} <input type="email" bind:value={email} required autocomplete="email" /></label
		>
		<label>
			{t('Password (at least 10 characters)')}
			<input
				type="password"
				bind:value={password}
				required
				minlength="10"
				autocomplete="new-password"
			/>
		</label>
		{#if error}<p class="error">{error}</p>{/if}
		<button class="primary" disabled={busy}>{t('Create admin')}</button>
	</form>
</div>
