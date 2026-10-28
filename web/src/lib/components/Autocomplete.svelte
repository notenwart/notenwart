<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- A text field that suggests values used before (composers, publishers, …); anything else can be typed too. -->
<script lang="ts">
	import { suggest } from '#lib/search.js';
	import Suggestions from './Suggestions.svelte';

	interface Props {
		value: string | null | undefined;
		suggestions: string[];
		placeholder?: string;
		maxlength?: number;
		required?: boolean;
		label?: string;
	}

	let {
		value = $bindable(),
		suggestions,
		placeholder,
		maxlength,
		required = false,
		label
	}: Props = $props();

	const id = `suggest-${Math.random().toString(36).slice(2, 9)}`;
	let open = $state(false);
	let active = $state(-1);
	const items = $derived(open ? suggest(suggestions, value ?? '') : []);

	function choose(item: string) {
		value = item;
		open = false;
	}

	function onkeydown(event: KeyboardEvent) {
		if (!items.length) return;
		if (event.key === 'ArrowDown') active = (active + 1) % items.length;
		else if (event.key === 'ArrowUp') active = active <= 0 ? items.length - 1 : active - 1;
		else if (event.key === 'Enter' && active >= 0) choose(items[active]);
		else if (event.key === 'Escape') open = false;
		else return;
		event.preventDefault();
	}
</script>

<div class="autocomplete">
	<input
		bind:value
		{placeholder}
		{maxlength}
		{required}
		aria-label={label}
		role="combobox"
		autocomplete="off"
		aria-autocomplete="list"
		aria-expanded={items.length > 0}
		aria-controls={id}
		aria-activedescendant={active >= 0 && items.length ? `${id}-${active}` : undefined}
		onfocus={() => (open = true)}
		onblur={() => (open = false)}
		oninput={() => {
			open = true;
			active = -1;
		}}
		{onkeydown}
	/>
	{#if items.length}
		<Suggestions {id} {items} {active} onchoose={choose} onhover={(i) => (active = i)} />
	{/if}
</div>

<style>
	.autocomplete {
		position: relative;
		display: flex;
	}
	input {
		flex: 1;
		min-width: 0;
	}
</style>
