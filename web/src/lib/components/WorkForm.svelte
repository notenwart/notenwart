<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import { untrack } from 'svelte';
	import { api, message, type Work, type WorkFacets } from '#lib/api/index.js';
	import { t } from '#lib/i18n/index.svelte.js';
	import Autocomplete from '#lib/components/Autocomplete.svelte';
	import Select from '#lib/components/Select.svelte';
	import { formatDuration, parseDuration } from '#lib/durations.js';
	import { workInput } from '#lib/works.js';
	import TagInput from '#lib/components/TagInput.svelte';

	interface Props {
		work: Work;
		onsaved: (work: Work) => void;
	}

	let { work, onsaved }: Props = $props();

	// A local copy, so typing does not change the page until it is saved.
	let draft = $state(untrack(() => $state.snapshot(work)));
	let saving = $state(false);
	let duration = $state(untrack(() => formatDuration(work.durationSeconds)));
	let error = $state('');
	let saved = $state(false);
	let facets = $state<WorkFacets>({
		composers: [],
		arrangers: [],
		publishers: [],
		instrumentations: [],
		tags: []
	});

	$effect(() => {
		api.GET('/works/facets').then(({ data }) => data && (facets = data));
	});

	async function save(event: SubmitEvent) {
		event.preventDefault();
		const seconds = parseDuration(duration);
		if (Number.isNaN(seconds)) {
			error = t('Write the duration as minutes:seconds, e.g. 4:30.');
			return;
		}
		saving = true;
		error = '';
		const { data, error: failure } = await api.PUT('/works/{workId}', {
			params: { path: { workId: work.id } },
			body: workInput(draft, {
				difficulty: draft.difficulty || null,
				durationSeconds: seconds
			})
		});
		saving = false;
		if (data) {
			saved = true;
			setTimeout(() => (saved = false), 2000);
			onsaved(data);
		} else {
			error = message(failure);
		}
	}
</script>

<form onsubmit={save}>
	<label class="wide"
		>{t('Title')} <input bind:value={draft.title} required maxlength="300" /></label
	>
	<label
		>{t('Composer')}
		<Autocomplete bind:value={draft.composer} suggestions={facets.composers} /></label
	>
	<label
		>{t('Arranger')}
		<Autocomplete bind:value={draft.arranger} suggestions={facets.arrangers} /></label
	>
	<label
		>{t('Publisher')}
		<Autocomplete bind:value={draft.publisher} suggestions={facets.publishers} /></label
	>
	<label
		>{t('Instrumentation')}
		<Autocomplete
			bind:value={draft.instrumentation}
			suggestions={facets.instrumentations}
			placeholder={t('e.g. Concert band')}
		/></label
	>
	<label>
		{t('Difficulty')}
		<Select
			value={draft.difficulty ? String(draft.difficulty) : ''}
			label={t('Difficulty')}
			options={[
				{ value: '', label: '—' },
				...[1, 2, 3, 4, 5, 6].map((n) => ({ value: String(n), label: String(n) }))
			]}
			onchange={(v) => (draft.difficulty = v ? Number(v) : null)}
		/>
	</label>
	<label>{t('Lyricist')} <input bind:value={draft.lyricist} /></label>
	<label
		>{t('Place in the archive')}
		<input bind:value={draft.archiveLocation} placeholder={t('e.g. cabinet 3, shelf B')} /></label
	>
	<label
		>{t('Work number (GEMA)')}
		<input bind:value={draft.societyNumber} placeholder="1234567-001" /></label
	>
	<label
		>{t('Duration (min:sec)')}
		<input bind:value={duration} placeholder="4:30" inputmode="numeric" /></label
	>
	<label class="wide"
		>{t('Tags')}
		<TagInput
			bind:value={draft.tags}
			suggestions={facets.tags}
			placeholder={t('e.g. March, Christmas')}
			label={t('Tags')}
		/></label
	>
	<label class="wide">
		{t('Source / licence')}
		<input
			bind:value={draft.source}
			placeholder={t('Where the music comes from or under which licence it is used')}
		/>
	</label>
	<label class="wide">{t('Notes')} <textarea rows="2" bind:value={draft.notes}></textarea></label>
	<div class="wide actions">
		<button class="primary" disabled={saving}>{t('Save details')}</button>
		{#if saved}<span class="muted">{t('Saved.')}</span>{/if}
		{#if error}<span class="error">{error}</span>{/if}
	</div>
</form>

<style>
	form {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(14rem, 1fr));
		gap: 0.75rem;
	}
	label {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		font-size: 0.85em;
		color: var(--muted);
	}
	label > :global(*) {
		color: var(--text);
		font-size: 1rem;
	}
	.wide {
		grid-column: 1 / -1;
	}
	.actions {
		display: flex;
		align-items: center;
		gap: 0.75rem;
	}
</style>
