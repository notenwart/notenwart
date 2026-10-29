<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import { goto, resolve } from '#lib/router.svelte.js';
	import { api, type Notification } from '#lib/api/index.js';
	import { describe, notifications } from '#lib/notifications.svelte.js';
	import { i18n, t } from '#lib/i18n/index.svelte.js';

	let items = $state<Notification[] | null>(null);

	async function load() {
		const { data } = await api.GET('/notifications');
		items = data?.items ?? [];
		notifications.unread = data?.unread ?? 0;
	}

	$effect(() => {
		load();
	});

	async function markAll() {
		await api.POST('/notifications/read', { body: {} });
		await load();
	}

	async function open(item: Notification) {
		if (!item.read) await api.POST('/notifications/read', { body: { ids: [item.id] } });
		notifications.unread = Math.max(0, notifications.unread - (item.read ? 0 : 1));
		// Links are paths of this app, set by the server.
		if (!item.link || item.link === '/') await goto(resolve('/'));
		else location.assign(item.link);
	}
</script>

<header class="title">
	<h1>{t('Notifications')}</h1>
	{#if items?.some((i) => !i.read)}
		<button onclick={markAll}>{t('Mark all as read')}</button>
	{/if}
</header>
<p class="muted">
	{t('Which ones you get, and how:')} <a href={resolve('/account')}>{t('Your account')}</a>
</p>

{#if items === null}
	<p class="muted">{t('Loading…')}</p>
{:else if items.length === 0}
	<p class="muted">{t('Nothing new.')}</p>
{:else}
	<ul class="items">
		{#each items as item (item.id)}
			<li class:unread={!item.read}>
				<button class="link" onclick={() => open(item)}>{describe(item)}</button>
				<span class="muted">{new Date(item.createdAt).toLocaleString(i18n.locale)}</span>
			</li>
		{/each}
	</ul>
{/if}

<style>
	.title {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem;
	}
	.items {
		list-style: none;
		padding: 0;
		margin: 0;
		border-top: 1px solid var(--line);
	}
	.items li {
		display: flex;
		flex-wrap: wrap;
		justify-content: space-between;
		gap: 0.25rem 1rem;
		padding: 0.6rem 0.25rem;
		border-bottom: 1px solid var(--line);
	}
	.items li button {
		color: var(--text);
		text-align: left;
	}
	.items li.unread button {
		font-weight: 600;
	}
	.items li.unread::before {
		content: '';
		width: 0.5rem;
		height: 0.5rem;
		border-radius: 50%;
		background: var(--accent);
		align-self: center;
		margin-right: -0.5rem;
	}
</style>
