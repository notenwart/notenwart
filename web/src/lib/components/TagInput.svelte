<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Tags as chips: Enter or comma adds what was typed, Backspace in the empty field removes the last one. -->
<script lang="ts">
	import { t } from '#lib/i18n/index.svelte.js';
	import { fold, suggest } from '#lib/search.js';
	import Icon from './Icon.svelte';
	import Suggestions from './Suggestions.svelte';

	interface Props {
		value: string[];
		suggestions: string[];
		placeholder?: string;
		label?: string;
	}

	let { value = $bindable(), suggestions, placeholder, label }: Props = $props();

	const id = `tags-${Math.random().toString(36).slice(2, 9)}`;
	let text = $state('');
	let open = $state(false);
	let active = $state(-1);
	let input: HTMLInputElement;
	const items = $derived(open ? suggest(suggestions, text, value) : []);

	function add(tag: string) {
		tag = tag.trim();
		if (tag && !value.some((v) => fold(v) === fold(tag))) value = [...value, tag];
		text = '';
		active = -1;
	}

	function remove(tag: string) {
		value = value.filter((v) => v !== tag);
		input.focus();
	}

	function onkeydown(event: KeyboardEvent) {
		if (event.key === 'ArrowDown' && items.length) active = (active + 1) % items.length;
		else if (event.key === 'ArrowUp' && items.length)
			active = active <= 0 ? items.length - 1 : active - 1;
		else if (event.key === 'Enter' || event.key === ',') {
			// Enter in an empty field still submits the form.
			if (event.key === 'Enter' && active < 0 && !text.trim()) return;
			add(active >= 0 ? items[active] : text);
		} else if (event.key === 'Backspace' && !text && value.length) value = value.slice(0, -1);
		else if (event.key === 'Escape') open = false;
		else return;
		event.preventDefault();
	}
</script>

<div class="tags">
	{#each value as tag (tag)}
		<span class="chip">
			{tag}
			<button
				type="button"
				class="icon"
				aria-label={t('Remove {name}', { name: tag })}
				onclick={() => remove(tag)}><Icon name="x" size={12} /></button
			>
		</span>
	{/each}
	<input
		bind:this={input}
		bind:value={text}
		placeholder={value.length ? '' : placeholder}
		aria-label={label}
		role="combobox"
		autocomplete="off"
		aria-autocomplete="list"
		aria-expanded={items.length > 0}
		aria-controls={id}
		aria-activedescendant={active >= 0 && items.length ? `${id}-${active}` : undefined}
		onfocus={() => (open = true)}
		onblur={() => {
			open = false;
			add(text);
		}}
		oninput={() => {
			open = true;
			active = -1;
		}}
		{onkeydown}
	/>
	{#if items.length}
		<Suggestions {id} {items} {active} onchoose={add} onhover={(i) => (active = i)} />
	{/if}
</div>

<style>
	.tags {
		position: relative;
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.3rem;
		padding: 0.25rem 0.4rem;
		border: 1px solid var(--line);
		border-radius: var(--radius);
		background: var(--bg);
		cursor: text;
	}
	.tags:focus-within {
		outline: 2px solid var(--focus);
		outline-offset: 1px;
	}
	.chip {
		display: inline-flex;
		align-items: center;
		gap: 0.15rem;
		padding: 0.1rem 0.15rem 0.1rem 0.55rem;
		border-radius: 999px;
		background: var(--surface);
		border: 1px solid var(--line);
		font-size: 0.9em;
	}
	.chip button {
		padding: 0.15rem;
		border: none;
		background: none;
	}
	input {
		flex: 1 1 8rem;
		min-width: 6rem;
		border: none;
		background: none;
		padding: 0.25rem 0.2rem;
	}
	input:focus-visible {
		outline: none;
	}
</style>
