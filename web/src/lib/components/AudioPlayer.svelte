<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
	Playing a work's recordings while reading the part: choose a recording, play, seek, slower or faster (the pitch
	stays), five seconds back or ahead, and repeat a passage from A to B.
-->
<script lang="ts">
	import type { Recording } from '#lib/api/index.js';
	import { formatDuration } from '#lib/durations.js';
	import { t } from '#lib/i18n/index.svelte.js';
	import Select from './Select.svelte';

	interface Props {
		recordings: Recording[];
		/** Dark bar inside the viewer. */
		dark?: boolean;
	}

	let { recordings, dark = false }: Props = $props();

	let chosen = $state('');
	let audio = $state<HTMLAudioElement>();
	let playing = $state(false);
	let time = $state(0);
	let duration = $state(0);
	let rate = $state(1);
	let loopA = $state<number | null>(null);
	let loopB = $state<number | null>(null);

	const recording = $derived(recordings.find((r) => r.id === chosen) ?? recordings[0]);
	const rates = [0.5, 0.6, 0.75, 0.9, 1, 1.1, 1.25];

	$effect(() => {
		// A new recording starts from the beginning, without a loop.
		void recording?.id;
		loopA = loopB = null;
		playing = false;
	});

	$effect(() => {
		if (audio) {
			audio.playbackRate = rate;
			audio.preservesPitch = true;
		}
	});

	function toggle() {
		if (!audio) return;
		if (audio.paused) audio.play().catch(() => (playing = false));
		else audio.pause();
	}

	function seek(to: number) {
		if (audio) audio.currentTime = Math.max(0, Math.min(to, duration || to));
	}

	function ontimeupdate() {
		if (!audio) return;
		time = audio.currentTime;
		if (loopA !== null && loopB !== null && time >= loopB) audio.currentTime = loopA;
	}

	/** First press sets A, second B, third clears the loop. */
	function markLoop() {
		if (loopA === null) loopA = time;
		else if (loopB === null) {
			if (time > loopA) loopB = time;
			else [loopA, loopB] = [time, loopA];
			seek(loopA ?? 0);
		} else loopA = loopB = null;
	}
</script>

{#if recording}
	<div class="player" class:dark>
		{#if recordings.length > 1}
			<Select
				value={recording.id}
				label={t('Recording')}
				options={recordings.map((r) => ({
					value: r.id,
					label: r.title,
					hint: r.kind === 'full' ? t('full recording') : t('practice')
				}))}
				onchange={(v) => (chosen = v)}
			/>
		{:else}
			<span class="title">{recording.title}</span>
		{/if}
		<button onclick={() => seek(time - 5)} aria-label={t('5 seconds back')}>−5</button>
		<button class="play" onclick={toggle} aria-label={playing ? t('Pause') : t('Play')}>
			{playing ? '❚❚' : '▶'}
		</button>
		<button onclick={() => seek(time + 5)} aria-label={t('5 seconds ahead')}>+5</button>
		<input
			class="seek"
			type="range"
			min="0"
			max={duration || 0}
			step="0.1"
			value={time}
			oninput={(e) => seek(Number(e.currentTarget.value))}
			aria-label={t('Position')}
		/>
		<span class="time">{formatDuration(time)} / {formatDuration(duration)}</span>
		<select bind:value={rate} aria-label={t('Speed')}>
			{#each rates as r (r)}<option value={r}>{r === 1 ? '1×' : `${r}×`}</option>{/each}
		</select>
		<button
			class:on={loopA !== null}
			onclick={markLoop}
			title={t('Repeat a passage: set start, then end; once more to stop.')}
		>
			{loopA === null
				? t('A–B')
				: loopB === null
					? t('A {time} – set B', { time: formatDuration(loopA) })
					: `${formatDuration(loopA)}–${formatDuration(loopB)} ↺`}
		</button>
		<audio
			bind:this={audio}
			src="/api/recordings/{recording.id}/file"
			preload="metadata"
			onplay={() => (playing = true)}
			onpause={() => (playing = false)}
			onended={() => (playing = false)}
			onloadedmetadata={() => (duration = audio?.duration ?? 0)}
			{ontimeupdate}
		></audio>
	</div>
{/if}

<style>
	.player {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.4rem 0.5rem;
		padding: 0.4rem 0.6rem;
		border-radius: var(--radius);
		background: var(--surface);
		border: 1px solid var(--line);
	}
	.player.dark {
		background: rgb(20 24 28 / 0.9);
		border-color: transparent;
		color: #fff;
	}
	.player.dark button,
	.player.dark select {
		background: rgb(255 255 255 / 0.12);
		border-color: transparent;
		color: #fff;
	}
	.player.dark :global(.trigger) {
		background: rgb(255 255 255 / 0.12);
		border-color: transparent;
		color: #fff;
	}
	.player.dark :global(.trigger .hint) {
		color: rgb(255 255 255 / 0.7);
	}
	.title {
		font-weight: 600;
		max-width: 14rem;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.play {
		min-width: 2.6rem;
	}
	.seek {
		flex: 1 1 8rem;
		min-width: 6rem;
		accent-color: var(--accent);
	}
	.time {
		font-variant-numeric: tabular-nums;
		font-size: 0.9em;
		white-space: nowrap;
	}
	button.on {
		outline: 2px solid var(--accent);
	}
	button,
	select {
		padding: 0.3rem 0.55rem;
	}
</style>
