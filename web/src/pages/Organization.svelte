<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- The organisation (admins): name, logo, and whether people may show a picture. -->
<script lang="ts">
	import { api, message, type Organization } from '#lib/api/index.js';
	import { loadSession, session } from '#lib/session.svelte.js';
	import { t } from '#lib/i18n/index.svelte.js';
	import { notify } from '#lib/dialog.svelte.js';
	import ImageField from '#lib/components/ImageField.svelte';

	let organization = $state<Organization | null>(null);
	let name = $state('');
	let pictures = $state(true);

	async function load() {
		const { data } = await api.GET('/organization');
		if (!data) return;
		organization = data;
		name = data.name;
		pictures = data.profilePictures;
	}

	$effect(() => {
		load();
	});

	async function save(event: SubmitEvent) {
		event.preventDefault();
		const { data, error } = await api.PUT('/organization', {
			body: { name, profilePictures: pictures }
		});
		if (!data) return notify(message(error), 'error');
		organization = data;
		notify(t('Saved.'));
		await loadSession();
	}
</script>

<div class="narrow">
	<h1>{t('Organisation')}</h1>
	{#if session.me?.role !== 'admin'}
		<p class="muted">{t('Only admins change the organisation.')}</p>
	{:else if organization}
		<h2>{t('Logo')}</h2>
		<p class="muted">{t('Shown in the header and on the sign-in page.')}</p>
		<ImageField
			name={organization.name}
			url={organization.logoUrl}
			endpoint="/api/organization/logo"
			square
			max={1024}
			onchange={async () => {
				await load();
				await loadSession();
			}}
		/>

		<form class="stack" onsubmit={save}>
			<label>
				{t('Name')}
				<input bind:value={name} required maxlength="120" />
			</label>
			<label class="check">
				<input type="checkbox" bind:checked={pictures} />
				{t('People may show a profile picture')}
			</label>
			<p class="muted small">
				{t(
					'Pupils are often minors. Without pictures everyone shows their initials; pictures already chosen stay hidden.'
				)}
			</p>
			<button class="primary">{t('Save')}</button>
		</form>
	{/if}
</div>

<style>
	h2 {
		font-size: 1.15rem;
		margin-top: 1.5rem;
	}
	form {
		margin-top: 1.5rem;
	}
	.check {
		flex-direction: row;
		align-items: center;
		gap: 0.5rem;
	}
	.small {
		font-size: 0.85em;
	}
</style>
