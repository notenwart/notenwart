<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- The list under a text field with suggestions; the field keeps the focus and drives it with the keyboard. -->
<script lang="ts">
	interface Props {
		id: string;
		items: string[];
		active: number;
		onchoose: (value: string) => void;
		onhover: (index: number) => void;
	}

	let { id, items, active, onchoose, onhover }: Props = $props();
</script>

<ul {id} role="listbox" class="suggestions">
	{#each items as item, i (item)}
		<li
			id="{id}-{i}"
			role="option"
			aria-selected={i === active}
			class:active={i === active}
			onpointerenter={() => onhover(i)}
			onpointerdown={(e) => {
				// Keep the focus in the field.
				e.preventDefault();
				onchoose(item);
			}}
		>
			{item}
		</li>
	{/each}
</ul>

<style>
	ul {
		position: absolute;
		top: calc(100% + 4px);
		left: 0;
		right: 0;
		z-index: 50;
		list-style: none;
		margin: 0;
		padding: 0.25rem 0;
		max-height: 16rem;
		overflow-y: auto;
		background: var(--raised);
		border: 1px solid var(--line);
		border-radius: var(--radius);
		box-shadow: var(--shadow);
		font-size: 1rem;
		color: var(--text);
	}
	li {
		padding: 0.4rem 0.75rem;
	}
	li.active {
		background: var(--hover);
	}
</style>
