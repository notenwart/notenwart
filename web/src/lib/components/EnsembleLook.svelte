<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- How an ensemble appears: a logo (larger ensembles), or a symbol on a colour (two clicks for smaller ones). -->
<script lang="ts">
	import { api, message, type EnsembleDetail } from '#lib/api/index.js';
	import { COLORS, SYMBOLS } from '#lib/avatars.js';
	import { t } from '#lib/i18n/index.svelte.js';
	import { notify } from '#lib/dialog.svelte.js';
	import ImageField from './ImageField.svelte';

	interface Props {
		ensemble: EnsembleDetail;
		onchange: () => void;
	}

	let { ensemble, onchange }: Props = $props();

	async function setSymbol(symbol: string | null, color: string | null) {
		const { error } = await api.PUT('/ensembles/{ensembleId}/symbol', {
			params: { path: { ensembleId: ensemble.id } },
			body: { symbol, color }
		});
		if (error) notify(message(error), 'error');
		onchange();
	}
</script>

<div class="look">
	<div>
		<p class="label">{t('Logo')}</p>
		<ImageField
			name={ensemble.name}
			url={ensemble.logoUrl}
			symbol={ensemble.symbol}
			color={ensemble.color}
			endpoint="/api/ensembles/{ensemble.id}/logo"
			square
			max={1024}
			{onchange}
		/>
	</div>
	<div>
		<p class="label">{t('Or a symbol')}</p>
		<div class="symbols" role="group" aria-label={t('Symbol')}>
			<button
				class:on={!ensemble.symbol}
				onclick={() => setSymbol(null, ensemble.color ?? null)}
				title={t('Initials')}>Aa</button
			>
			{#each SYMBOLS as symbol (symbol)}
				<button
					class:on={ensemble.symbol === symbol}
					onclick={() => setSymbol(symbol, ensemble.color ?? null)}>{symbol}</button
				>
			{/each}
		</div>
		<div class="colors" role="group" aria-label={t('Colour')}>
			{#each COLORS as color (color)}
				<button
					class="swatch"
					class:on={ensemble.color === color}
					style:background={color}
					aria-label={color}
					onclick={() => setSymbol(ensemble.symbol ?? null, color)}
				></button>
			{/each}
		</div>
		{#if ensemble.logoUrl}
			<p class="muted small">{t('The logo is shown while there is one.')}</p>
		{/if}
	</div>
</div>

<style>
	.look {
		display: flex;
		flex-wrap: wrap;
		gap: 1rem 2.5rem;
	}
	.label {
		margin: 0 0 0.4rem;
		font-size: 0.85em;
		color: var(--muted);
	}
	.symbols,
	.colors {
		display: flex;
		flex-wrap: wrap;
		gap: 0.3rem;
		max-width: 26rem;
	}
	.colors {
		margin-top: 0.5rem;
	}
	.symbols button {
		width: 2.3rem;
		height: 2.3rem;
		padding: 0;
		font-size: 1.15rem;
	}
	.swatch {
		width: 1.6rem;
		height: 1.6rem;
		padding: 0;
		border-radius: 50%;
	}
	button.on {
		outline: 2px solid var(--accent);
		outline-offset: 1px;
	}
	.small {
		font-size: 0.85em;
	}
</style>
