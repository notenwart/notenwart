<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- A picture or logo; without one a symbol, else initials – on a colour. Round for people, rounded squares else. -->
<script lang="ts">
	import { colorFor, initials } from '#lib/avatars.js';

	interface Props {
		name: string;
		url?: string | null;
		symbol?: string | null;
		color?: string | null;
		size?: number;
		square?: boolean;
	}

	let {
		name,
		url = null,
		symbol = null,
		color = null,
		size = 32,
		square = false
	}: Props = $props();
	let broken = $state(false);
</script>

<span
	class="avatar"
	class:square
	style:width="{size}px"
	style:height="{size}px"
	style:font-size="{Math.round(size * (symbol ? 0.55 : 0.4))}px"
	style:background={url && !broken ? 'transparent' : (color ?? colorFor(name))}
	aria-hidden="true"
>
	{#if url && !broken}
		<img src={url} alt="" onerror={() => (broken = true)} />
	{:else}
		{symbol ?? initials(name)}
	{/if}
</span>

<style>
	.avatar {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		flex: none;
		border-radius: 50%;
		color: #fff;
		font-weight: 600;
		line-height: 1;
		overflow: hidden;
		user-select: none;
	}
	.avatar.square {
		border-radius: 22%;
	}
	img {
		width: 100%;
		height: 100%;
		object-fit: cover;
	}
</style>
