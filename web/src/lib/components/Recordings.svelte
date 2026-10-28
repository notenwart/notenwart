<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- A work's recordings: full and practice recordings, recorded or generated; added by staff and leaders. -->
<script lang="ts">
	import {
		api,
		message,
		type Instrument,
		type Recording,
		type WorkDetail
	} from '#lib/api/index.js';
	import { formatDuration } from '#lib/durations.js';
	import { plural, t } from '#lib/i18n/index.svelte.js';
	import { confirm, notify } from '#lib/dialog.svelte.js';
	import { instrumentOptions } from '#lib/instruments.js';
	import { AUDIO_ACCEPT, measure, titleFromFile, uploadRecording } from '#lib/recordings.js';
	import AudioPlayer from './AudioPlayer.svelte';
	import Icon from './Icon.svelte';
	import Select from './Select.svelte';

	interface Props {
		work: WorkDetail;
		instruments: Instrument[];
		/** Staff and leaders may add; the server decides for leaders (only for their ensembles' works). */
		canManage: boolean;
	}

	let { work, instruments, canManage }: Props = $props();

	let recordings = $state<Recording[]>([]);
	let uploads = $state<{ name: string; progress: number; error: string }[]>([]);
	// The form for the next files.
	let kind = $state<'full' | 'practice'>('practice');
	let origin = $state<'recorded' | 'generated'>('recorded');
	let partId = $state('');
	let instrumentId = $state('');
	let editing = $state<Recording | null>(null);
	let title = $state('');

	const parts = $derived(work.parts.map((p) => ({ value: p.id, label: p.label })));
	const partNames = $derived(new Map(work.parts.map((p) => [p.id, p.label])));
	const instrumentNames = $derived(new Map(instruments.map((i) => [i.id, i.name])));

	async function load() {
		const { data } = await api.GET('/works/{workId}/recordings', {
			params: { path: { workId: work.id } }
		});
		recordings = data ?? [];
	}

	$effect(() => {
		void work.id;
		load();
	});

	async function add(files: FileList) {
		for (const file of files) {
			const entry = { name: file.name, progress: 0, error: '' };
			uploads = [...uploads, entry];
			const current = uploads[uploads.length - 1];
			try {
				const durationSeconds = await measure(file);
				await uploadRecording(
					work.id,
					file,
					{
						kind,
						origin,
						title: titleFromFile(file.name) || file.name,
						partId: kind === 'practice' ? partId || null : null,
						instrumentId: kind === 'practice' && !partId ? instrumentId || null : null,
						durationSeconds
					},
					(p) => (current.progress = p)
				);
				uploads = uploads.filter((u) => u !== current);
			} catch (e) {
				current.error = e instanceof Error ? t(e.message) : String(e);
			}
		}
		await load();
	}

	function startEditing(recording: Recording) {
		editing = recording;
		title = recording.title;
	}

	async function save() {
		if (!editing) return;
		const { error } = await api.PUT('/recordings/{recordingId}', {
			params: { path: { recordingId: editing.id } },
			body: {
				kind: editing.kind,
				origin: editing.origin,
				title,
				partId: editing.partId,
				instrumentId: editing.instrumentId,
				voice: editing.voice
			}
		});
		if (error) return notify(message(error), 'error');
		editing = null;
		await load();
	}

	async function remove(recording: Recording) {
		const ok = await confirm({
			title: t('Delete the recording “{name}”?', { name: recording.title }),
			confirm: t('Delete'),
			danger: true
		});
		if (!ok) return;
		const { error } = await api.DELETE('/recordings/{recordingId}', {
			params: { path: { recordingId: recording.id } }
		});
		if (error) notify(message(error), 'error');
		await load();
	}

	function belongsTo(r: Recording) {
		if (r.kind === 'full') return t('whole work');
		if (r.partId) return partNames.get(r.partId) ?? '';
		if (r.instrumentId) return instrumentNames.get(r.instrumentId) ?? '';
		return t('practice');
	}
</script>

{#if recordings.length}
	<AudioPlayer {recordings} />
	<table class="recordings">
		<tbody>
			{#each recordings as r (r.id)}
				<tr>
					<td>
						{#if editing?.id === r.id}
							<form
								class="row"
								onsubmit={(e) => {
									e.preventDefault();
									save();
								}}
							>
								<input bind:value={title} maxlength="120" required aria-label={t('Title')} />
								<button class="primary">{t('Save')}</button>
								<button type="button" onclick={() => (editing = null)}>{t('Cancel')}</button>
							</form>
						{:else}
							<strong>{r.title}</strong>
						{/if}
					</td>
					<td>{belongsTo(r)}</td>
					<td class="muted">{r.origin === 'generated' ? t('generated') : t('recorded')}</td>
					<td class="muted">{r.durationSeconds ? formatDuration(r.durationSeconds) : ''}</td>
					<td class="row-actions">
						<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- served by the API, not a route -->
						<a class="button icon" href="/api/recordings/{r.id}/file" download title={t('Download')}
							><Icon name="download" label={t('Download')} /></a
						>
						{#if canManage}
							<button class="icon" title={t('Rename')} onclick={() => startEditing(r)}
								><Icon name="edit" label={t('Rename')} /></button
							>
							<button class="icon danger" title={t('Delete')} onclick={() => remove(r)}
								><Icon name="trash" label={t('Delete')} /></button
							>
						{/if}
					</td>
				</tr>
			{/each}
		</tbody>
	</table>
{:else}
	<p class="muted">{t('No recordings yet.')}</p>
{/if}

{#if canManage}
	<div class="add">
		<div class="row">
			<Select
				bind:value={kind}
				label={t('Kind')}
				options={[
					{ value: 'practice', label: t('Practice recording of a part') },
					{ value: 'full', label: t('Full recording of the work') }
				]}
			/>
			<Select
				bind:value={origin}
				label={t('Origin')}
				options={[
					{ value: 'recorded', label: t('recorded') },
					{ value: 'generated', label: t('generated (e.g. from MIDI)') }
				]}
			/>
			{#if kind === 'practice'}
				{#if parts.length}
					<Select
						bind:value={partId}
						label={t('Part')}
						placeholder={t('Part …')}
						options={[{ value: '', label: t('— by instrument or voice —') }, ...parts]}
					/>
				{/if}
				{#if !partId}
					<Select
						bind:value={instrumentId}
						label={t('Instrument')}
						placeholder={t('Instrument or voice …')}
						options={instrumentOptions(instruments)}
					/>
				{/if}
			{/if}
		</div>
		<label class="button">
			{t('Add audio files …')}
			<input
				type="file"
				accept={AUDIO_ACCEPT}
				multiple
				onchange={(e) => {
					const input = e.currentTarget;
					if (input.files?.length) add(input.files).then(() => (input.value = ''));
				}}
			/>
		</label>
		<p class="muted small">
			{t('MP3, M4A (also voice memos), WAV, FLAC, Ogg/Opus, WebM, AIFF – no videos.')}
		</p>
		{#each uploads as u, i (i)}
			<div class="upload">
				<span>{u.name}</span>
				{#if u.error}<span class="error">{u.error}</span>{:else}<progress max="1" value={u.progress}
					></progress>{/if}
			</div>
		{/each}
		{#if uploads.some((u) => u.error)}
			<button class="link" onclick={() => (uploads = uploads.filter((u) => !u.error))}
				>{plural(
					uploads.filter((u) => u.error).length,
					'Dismiss {n} error',
					'Dismiss {n} errors'
				)}</button
			>
		{/if}
	</div>
{/if}

<style>
	.recordings {
		width: 100%;
		border-collapse: collapse;
		margin: 0.6rem 0;
	}
	.recordings td {
		padding: 0.35rem 0.5rem 0.35rem 0;
		border-bottom: 1px solid var(--line);
	}
	.row {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
		align-items: center;
	}
	.add {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 0.5rem;
		margin-top: 0.5rem;
	}
	label.button input {
		display: none;
	}
	.upload {
		display: flex;
		gap: 0.75rem;
		align-items: center;
	}
	.small {
		font-size: 0.85em;
		margin: 0;
	}
	.row-actions {
		white-space: nowrap;
		text-align: right;
	}
</style>
