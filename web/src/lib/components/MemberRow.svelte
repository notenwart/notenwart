<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import Avatar from '#lib/components/Avatar.svelte';
	import type { EnsembleMember, Instrument, PlayedPart } from '#lib/api/index.js';
	import { instrumentOptions } from '#lib/instruments.js';
	import { t } from '#lib/i18n/index.svelte.js';
	import Select from '#lib/components/Select.svelte';

	interface Props {
		member: EnsembleMember;
		instruments: Instrument[];
		/** May change who leads (admins and librarians). */
		staff: boolean;
		onsave: (leader: boolean, plays: PlayedPart[]) => Promise<void>;
		onremove: () => void;
	}

	let { member, instruments, staff, onsave, onremove }: Props = $props();

	let instrumentId = $state('');
	let voice = $state<number | null>(null);

	function label(part: PlayedPart) {
		const name = t(instruments.find((i) => i.id === part.instrumentId)?.name ?? '?');
		return part.voice
			? `${name} ${part.voice}`
			: t('{instrument} (all parts)', { instrument: name });
	}

	async function add(event: SubmitEvent) {
		event.preventDefault();
		if (!instrumentId) return;
		await onsave(member.leader, [...member.plays, { instrumentId, voice: voice || null }]);
		instrumentId = '';
		voice = null;
	}
</script>

<tr>
	<td class="who">
		<Avatar name={member.name} url={member.avatarUrl} size={26} />
		{member.name}
		{#if member.leader && !staff}<span class="badge">{t('Leader')}</span>{/if}
	</td>
	<td>
		{#if staff}
			<label class="leader">
				<input
					type="checkbox"
					checked={member.leader}
					onchange={(e) => onsave(e.currentTarget.checked, member.plays)}
				/>
				{t('Leads')}
			</label>
		{/if}
	</td>
	<td>
		<div class="plays">
			{#each member.plays as part, i (i)}
				<span class="chip">
					{label(part)}
					<button
						class="link"
						aria-label={t('Remove {item}', { item: label(part) })}
						onclick={() =>
							onsave(
								member.leader,
								member.plays.filter((_, j) => j !== i)
							)}>×</button
					>
				</span>
			{/each}
			<form onsubmit={add}>
				<Select
					bind:value={instrumentId}
					label={t('Instrument for {name}', { name: member.name })}
					placeholder={t('+ instrument')}
					options={instrumentOptions(instruments)}
				/>
				{#if instrumentId}
					<input
						type="number"
						min="1"
						max="99"
						placeholder={t('all')}
						bind:value={voice}
						aria-label={t('Part number')}
					/>
					<button>{t('Add')}</button>
				{/if}
			</form>
		</div>
	</td>
	<td class="actions">
		{#if staff || !member.leader}
			<button class="danger" onclick={onremove}>{t('Remove')}</button>
		{/if}
	</td>
</tr>

<style>
	.who {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}
	td {
		padding: 0.5rem 0.5rem 0.5rem 0;
		border-bottom: 1px solid var(--line);
		vertical-align: top;
	}
	.badge {
		font-size: 0.75rem;
		background: var(--selected);
		border-radius: 999px;
		padding: 0.1rem 0.5rem;
		margin-left: 0.5rem;
	}
	.leader {
		white-space: nowrap;
	}
	.plays {
		display: flex;
		flex-wrap: wrap;
		gap: 0.4rem;
		align-items: center;
	}
	.chip {
		background: var(--surface);
		border: 1px solid var(--line);
		border-radius: 999px;
		padding: 0.2rem 0.6rem;
		white-space: nowrap;
	}
	form {
		display: flex;
		gap: 0.4rem;
	}
	form input {
		width: 4.5rem;
	}
	.actions {
		text-align: right;
	}
</style>
