<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import { dialogs } from '#lib/dialog.svelte.js';
	import { t } from '#lib/i18n/index.svelte.js';

	let element = $state<HTMLDialogElement>();

	$effect(() => {
		if (dialogs.confirm && element && !element.open) element.showModal();
		if (!dialogs.confirm && element?.open) element.close();
	});

	function answer(ok: boolean) {
		const request = dialogs.confirm;
		dialogs.confirm = null;
		request?.resolve(ok);
	}
</script>

<dialog bind:this={element} oncancel={() => answer(false)} aria-labelledby="confirm-title">
	{#if dialogs.confirm}
		<h2 id="confirm-title">{dialogs.confirm.title}</h2>
		{#if dialogs.confirm.message}<p>{dialogs.confirm.message}</p>{/if}
		<div class="buttons">
			<button onclick={() => answer(false)}>{t('Cancel')}</button>
			<button
				class={dialogs.confirm.danger ? 'danger solid' : 'primary'}
				onclick={() => answer(true)}
			>
				{dialogs.confirm.confirm}
			</button>
		</div>
	{/if}
</dialog>

<div class="notices" role="status" aria-live="polite">
	{#each dialogs.notices as notice (notice.id)}
		<p class:error={notice.kind === 'error'}>{notice.text}</p>
	{/each}
</div>

<style>
	dialog {
		border: 1px solid var(--line);
		border-radius: 12px;
		background: var(--raised);
		color: var(--text);
		box-shadow: var(--shadow);
		padding: 1.25rem 1.5rem;
		width: min(28rem, calc(100vw - 2rem));
	}
	dialog::backdrop {
		background: rgb(10 15 25 / 0.45);
	}
	h2 {
		margin: 0 0 0.5rem;
		font-size: 1.2rem;
	}
	p {
		margin: 0 0 1rem;
		color: var(--muted);
	}
	.buttons {
		display: flex;
		justify-content: flex-end;
		gap: 0.5rem;
	}
	.solid {
		background: var(--danger);
		border-color: var(--danger);
		color: #fff;
	}
	.solid:hover:not(:disabled) {
		background: var(--danger);
		filter: brightness(1.1);
		color: #fff;
	}
	.notices {
		position: fixed;
		left: 50%;
		bottom: 1.5rem;
		transform: translateX(-50%);
		z-index: 100;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		pointer-events: none;
	}
	.notices p {
		margin: 0;
		padding: 0.6rem 1rem;
		border-radius: 999px;
		background: var(--accent);
		color: var(--accent-text);
		box-shadow: var(--shadow);
	}
	.notices p.error {
		background: var(--danger);
		color: #fff;
	}
</style>
