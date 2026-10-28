<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
	A select with search: type to filter, arrow keys and Enter to choose, Escape to close. Shows at most `limit`
	matches at a time (catalogues of 2000+ parts stay fast) and never grows beyond the max height.
-->

<script lang="ts">
	import { t } from '#lib/i18n/index.svelte.js';
	import { filterOptions, type Option } from '#lib/search.js';
	import Icon from './Icon.svelte';

	interface Props {
		options: Option[];
		value?: string;
		placeholder?: string;
		label?: string;
		disabled?: boolean;
		/** How many matches are listed at most. */
		limit?: number;
		onchange?: (value: string) => void;
	}

	let {
		options,
		value = $bindable(''),
		placeholder = '',
		label,
		disabled = false,
		limit = 150,
		onchange
	}: Props = $props();

	const id = `select-${Math.random().toString(36).slice(2, 9)}`;
	let open = $state(false);
	let query = $state('');
	let active = $state(0);
	let root: HTMLDivElement;
	let list = $state<HTMLUListElement>();
	let search = $state<HTMLInputElement>();

	const selected = $derived(options.find((o) => o.value === value));
	const matches = $derived(filterOptions(options, query));
	const shown = $derived(matches.slice(0, limit));

	function toggle() {
		if (disabled) return;
		if (open) close();
		else show();
	}

	function show() {
		open = true;
		query = '';
		active = Math.max(
			0,
			shown.findIndex((o) => o.value === value)
		);
		queueMicrotask(() => {
			search?.focus();
			scrollToActive();
		});
	}

	function close(refocus = false) {
		open = false;
		if (refocus) root.querySelector<HTMLButtonElement>('.trigger')?.focus();
	}

	function choose(option: Option) {
		value = option.value;
		onchange?.(option.value);
		close(true);
	}

	function scrollToActive() {
		list?.querySelector(`#${id}-${active}`)?.scrollIntoView({ block: 'nearest' });
	}

	function onkeydown(event: KeyboardEvent) {
		if (event.key === 'ArrowDown') active = Math.min(active + 1, shown.length - 1);
		else if (event.key === 'ArrowUp') active = Math.max(active - 1, 0);
		else if (event.key === 'Home') active = 0;
		else if (event.key === 'End') active = shown.length - 1;
		else if (event.key === 'Enter') {
			if (shown[active]) choose(shown[active]);
		} else if (event.key === 'Escape') close(true);
		else if (event.key === 'Tab') close();
		else return;
		event.preventDefault();
		queueMicrotask(scrollToActive);
	}

	function onTriggerKey(event: KeyboardEvent) {
		if (['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(event.key)) {
			event.preventDefault();
			show();
		}
	}

	// Close when clicking elsewhere.
	$effect(() => {
		if (!open) return;
		const outside = (event: PointerEvent) => {
			if (!root.contains(event.target as Node)) close();
		};
		window.addEventListener('pointerdown', outside);
		return () => window.removeEventListener('pointerdown', outside);
	});

	$effect(() => {
		void query;
		active = 0;
	});
</script>

<div class="select" bind:this={root}>
	<button
		type="button"
		class="trigger"
		class:placeholder={!selected}
		{disabled}
		aria-haspopup="listbox"
		aria-expanded={open}
		aria-label={label}
		onclick={toggle}
		onkeydown={onTriggerKey}
	>
		<span class="value">{selected ? selected.label : placeholder}</span>
		{#if selected?.hint}<span class="hint">{selected.hint}</span>{/if}
		<Icon name="chevronDown" size={16} />
	</button>
	{#if open}
		<div class="popover">
			<div class="search">
				<Icon name="search" size={16} />
				<input
					bind:this={search}
					bind:value={query}
					{onkeydown}
					placeholder={t('Search …')}
					role="combobox"
					aria-expanded="true"
					aria-controls="{id}-list"
					aria-activedescendant={shown[active] ? `${id}-${active}` : undefined}
					aria-autocomplete="list"
					aria-label={label ?? placeholder}
				/>
			</div>
			<ul id="{id}-list" role="listbox" bind:this={list} aria-label={label ?? placeholder}>
				{#each shown as option, i (option.value)}
					{#if option.group && option.group !== shown[i - 1]?.group}
						<li class="group" role="presentation">{option.group}</li>
					{/if}
					<li
						id="{id}-{i}"
						role="option"
						aria-selected={option.value === value}
						class:active={i === active}
						onpointerenter={() => (active = i)}
						onpointerdown={(e) => {
							e.preventDefault();
							choose(option);
						}}
					>
						<span>{option.label}</span>
						{#if option.hint}<span class="hint">{option.hint}</span>{/if}
						{#if option.value === value}<Icon name="check" size={14} />{/if}
					</li>
				{:else}
					<li class="empty" role="presentation">{t('No matches')}</li>
				{/each}
				{#if matches.length > shown.length}
					<li class="more" role="presentation">
						{t('{n} more – type to narrow down', { n: matches.length - shown.length })}
					</li>
				{/if}
			</ul>
		</div>
	{/if}
</div>

<style>
	.select {
		position: relative;
		display: inline-flex;
		min-width: 10rem;
		max-width: 100%;
	}
	.trigger {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		width: 100%;
		text-align: left;
		background: var(--bg);
		padding: 0.45rem 0.6rem;
	}
	.trigger .value {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.trigger.placeholder .value {
		color: var(--muted);
	}
	.hint {
		color: var(--muted);
		font-size: 0.85em;
		white-space: nowrap;
	}
	.popover {
		position: absolute;
		top: calc(100% + 4px);
		left: 0;
		z-index: 50;
		min-width: 100%;
		width: max-content;
		max-width: min(32rem, 90vw);
		background: var(--raised);
		border: 1px solid var(--line);
		border-radius: var(--radius);
		box-shadow: var(--shadow);
		overflow: hidden;
	}
	.search {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		padding: 0.4rem 0.6rem;
		border-bottom: 1px solid var(--line);
		color: var(--muted);
	}
	.search input {
		flex: 1;
		border: none;
		background: none;
		padding: 0.2rem 0;
		color: var(--text);
	}
	.search input:focus-visible {
		outline: none;
	}
	ul {
		list-style: none;
		margin: 0;
		padding: 0.25rem 0;
		max-height: 18rem;
		overflow-y: auto;
		overscroll-behavior: contain;
	}
	li[role='option'] {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.4rem 0.75rem;
	}
	li[role='option'] > span:first-child {
		flex: 1;
	}
	li.active {
		background: var(--hover);
	}
	li[aria-selected='true'] {
		font-weight: 600;
	}
	.group {
		padding: 0.5rem 0.75rem 0.2rem;
		font-size: 0.75rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		color: var(--muted);
	}
	.empty,
	.more {
		padding: 0.5rem 0.75rem;
		color: var(--muted);
		font-size: 0.9em;
	}
</style>
