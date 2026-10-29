<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import { t } from '#lib/i18n/index.svelte.js';
	import { goto, page, resolve } from '#lib/router.svelte.js';
	import { api } from '#lib/api/index.js';
	import { signedIn } from '#lib/session.svelte.js';

	let error = $state('');

	// Landing page of sign-in and invitation links: trade the token for a session.
	$effect(() => {
		const token = page.url.searchParams.get('token');
		if (!token) {
			error = t('This link is incomplete.');
			return;
		}
		// Keep the token out of the history and of referrers.
		history.replaceState(history.state, '', '/auth');
		api.POST('/auth/token', { body: { token } }).then(({ data }) => {
			if (!data) {
				error = t('This link has expired or was used already. Ask for a new one.');
				return;
			}
			signedIn(data);
			goto(data.hasPassword ? resolve('/') : resolve('/account?welcome'));
		});
	});
</script>

<div class="narrow">
	{#if error}
		<h1>{t('Sign-in link')}</h1>
		<p class="error">{error}</p>
		<p><a href={resolve('/login')}>{t('Go to sign-in')}</a></p>
	{:else}
		<p class="muted">{t('Signing you in…')}</p>
	{/if}
</div>
