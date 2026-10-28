<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Choosing, replacing or removing a picture or logo; it is scaled down in the browser before it is uploaded. -->
<script lang="ts">
	import { putImage, shrink } from '#lib/avatars.js';
	import { message } from '#lib/api/index.js';
	import { t } from '#lib/i18n/index.svelte.js';
	import { notify } from '#lib/dialog.svelte.js';
	import Avatar from './Avatar.svelte';

	interface Props {
		name: string;
		url: string | null | undefined;
		/** Where the image is PUT and DELETEd. */
		endpoint: string;
		square?: boolean;
		symbol?: string | null;
		color?: string | null;
		/** Longest side after scaling. */
		max?: number;
		onchange: () => void;
	}

	let {
		name,
		url,
		endpoint,
		square = false,
		symbol = null,
		color = null,
		max = 512,
		onchange
	}: Props = $props();
	let busy = $state(false);

	async function choose(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		input.value = '';
		if (!file) return;
		busy = true;
		try {
			const response = await putImage(endpoint, await shrink(file, max));
			if (!response.ok) notify(message(await response.json().catch(() => null)), 'error');
			else onchange();
		} catch {
			notify(t('This image cannot be read.'), 'error');
		}
		busy = false;
	}

	async function remove() {
		busy = true;
		await fetch(endpoint, { method: 'DELETE' });
		busy = false;
		onchange();
	}
</script>

<div class="image-field">
	<Avatar {name} {url} {symbol} {color} size={64} {square} />
	<label class="button" class:disabled={busy}>
		{url ? t('Replace …') : t('Choose an image …')}
		<input
			type="file"
			accept="image/png,image/jpeg,image/webp,image/heic,image/*"
			onchange={choose}
		/>
	</label>
	{#if url}<button class="link danger" disabled={busy} onclick={remove}>{t('Remove')}</button>{/if}
</div>

<style>
	.image-field {
		display: flex;
		align-items: center;
		gap: 0.75rem;
	}
	label.button input {
		display: none;
	}
	label.disabled {
		opacity: 0.5;
		pointer-events: none;
	}
</style>
