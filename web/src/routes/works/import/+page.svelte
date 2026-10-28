<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
	Import a whole folder of sheet music – a Google Drive download (ZIP), a ZIP or a folder on the computer. Folders
	become works, PDFs parts; everything can be corrected before the upload starts.
-->
<script lang="ts">
	import { resolve } from '$app/paths';
	import {
		AlreadyUploaded,
		api,
		message,
		uploadSource,
		type Instrument,
		type Recognition,
		type WorkFacets
	} from '#lib/api/index.js';
	import { isStaff, session } from '#lib/session.svelte.js';
	import { i18n, plural, t } from '#lib/i18n/index.svelte.js';
	import { confirm } from '#lib/dialog.svelte.js';
	import { instrumentOptions } from '#lib/instruments.js';
	import {
		applyRecognition,
		chosenFiles,
		planImport,
		readDropped,
		readFiles,
		type Entry,
		type Mode,
		type Plan,
		type PlannedFile,
		type PlannedWork
	} from '#lib/bulk-import.js';
	import Autocomplete from '#lib/components/Autocomplete.svelte';
	import Select from '#lib/components/Select.svelte';
	import TagInput from '#lib/components/TagInput.svelte';

	let instruments = $state<Instrument[]>([]);
	let existing = $state<{ id: string; title: string }[]>([]);
	let facets = $state<WorkFacets>({
		composers: [],
		arrangers: [],
		publishers: [],
		instrumentations: [],
		tags: []
	});
	let plan = $state<Plan | null>(null);
	let reading = $state(false);
	let dragging = $state(false);
	let error = $state('');

	// Reading the first pages: two at a time, can be stopped.
	let recognizing = $state(false);
	let recognized = $state(0);
	let toRecognize = $state(0);
	let stopRecognizing = false;
	const abilities = $derived(session.status?.recognition);
	const canRecognize = $derived(!!abilities && (abilities.text || abilities.ocr || abilities.ai));

	async function recognizeOne(file: PlannedFile): Promise<Recognition | null> {
		const form = new FormData();
		form.append('file', new File([file.file], file.name, { type: 'application/pdf' }));
		const response = await fetch('/api/recognize', { method: 'POST', body: form });
		return response.ok ? ((await response.json()) as Recognition) : null;
	}

	async function recognizeAll() {
		if (!plan) return;
		const queue = plan.works.flatMap((work) =>
			work.files.filter((f) => f.mode !== 'skip' && !f.found).map((file) => ({ work, file }))
		);
		recognizing = true;
		stopRecognizing = false;
		recognized = 0;
		toRecognize = queue.length;
		const worker = async () => {
			for (let next = queue.shift(); next && !stopRecognizing; next = queue.shift()) {
				const found = await recognizeOne(next.file).catch(() => null);
				if (found) applyRecognition(next.work, next.file, found, instruments, existing);
				recognized++;
			}
		};
		await Promise.all([worker(), worker()]);
		recognizing = false;
	}

	/** "2. Trompete in B · B♭ major · treble" – what the page said, under the file name. */
	function foundSummary(found: Recognition) {
		return [
			found.part,
			found.key,
			found.clef,
			found.timeSignature,
			found.method === 'ocr' ? t('scan') : null
		]
			.filter(Boolean)
			.join(' · ');
	}

	// The run: one step per file, results per work.
	let running = $state(false);
	let finished = $state(false);
	let done = $state(0);
	let current = $state('');
	let failures = $state<{ path: string; message: string }[]>([]);
	let toSplit = $state<{ id: string; title: string; count: number }[]>([]);
	let created = $state(0);
	let uploaded = $state(0);
	let present = $state(0);

	const maxBytes = $derived(session.status?.maxUploadBytes ?? 50 * 1024 * 1024);
	const options = $derived(instrumentOptions(instruments));
	const modes = $derived<{ value: Mode; label: string }[]>([
		{ value: 'part', label: t('Part for an instrument') },
		{ value: 'score', label: t('Part without instrument (score)') },
		{ value: 'source', label: t('Upload only, split later') },
		{ value: 'skip', label: t('Leave out') }
	]);
	const files = $derived(plan ? plan.works.flatMap(chosenFiles) : []);
	const undecided = $derived(
		plan ? plan.works.filter((w) => w.existingId && w.decision === 'open').length : 0
	);
	const decisions = $derived([
		{ value: 'open', label: t('Not decided – left out') },
		{ value: 'add', label: t('Add the files to the existing work') },
		{ value: 'new', label: t('Create a separate work') }
	]);
	const newWorks = $derived(
		plan
			? plan.works.filter((w) => (!w.existingId || w.decision === 'new') && chosenFiles(w).length)
					.length
			: 0
	);
	const incomplete = $derived(files.filter((f) => f.mode === 'part' && !f.instrumentId).length);

	$effect(() => {
		api.GET('/instruments').then(({ data }) => (instruments = data ?? []));
		api.GET('/works').then(({ data }) => (existing = data ?? []));
		api.GET('/works/facets').then(({ data }) => data && (facets = data));
	});

	// Leaving the page during an import would stop it halfway.
	$effect(() => {
		if (!running) return;
		const stay = (event: BeforeUnloadEvent) => event.preventDefault();
		window.addEventListener('beforeunload', stay);
		return () => window.removeEventListener('beforeunload', stay);
	});

	async function load(read: () => Promise<Entry[]>) {
		reading = true;
		error = '';
		finished = false;
		try {
			const entries = await read();
			plan = planImport(entries, instruments, existing, maxBytes);
			if (!plan.works.length) error = t('No PDF files found.');
		} catch (e) {
			error = t('The files could not be read: {reason}', { reason: String(e) });
			plan = null;
		}
		reading = false;
	}

	function ondrop(event: DragEvent) {
		event.preventDefault();
		dragging = false;
		const items = event.dataTransfer?.items;
		if (items) load(() => readDropped(items));
	}

	function onpick(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		const picked = input.files;
		if (picked?.length) load(() => readFiles(picked));
		input.value = '';
	}

	function setMode(file: PlannedFile, mode: Mode) {
		file.mode = mode;
		if (mode === 'score' && !file.label) file.label = t('Full score');
	}

	function setInstrument(file: PlannedFile, id: string) {
		file.instrumentId = id;
		const instrument = instruments.find((i) => i.id === id);
		if (instrument) file.label = file.voice ? `${instrument.name} ${file.voice}` : instrument.name;
	}

	function skipWork(work: PlannedWork) {
		for (const f of work.files) f.mode = 'skip';
	}

	function size(bytes: number) {
		return bytes < 1024 * 1024
			? `${Math.max(1, Math.round(bytes / 1024))} KB`
			: `${(bytes / 1024 / 1024).toLocaleString(i18n.locale, { maximumFractionDigits: 1 })} MB`;
	}

	async function run() {
		if (!plan) return;
		if (
			incomplete &&
			!(await confirm({
				title: plural(incomplete, '{n} part has no instrument', '{n} parts have no instrument'),
				message: t('They are uploaded and can be split on the work page later.'),
				confirm: t('Import anyway')
			}))
		)
			return;
		running = true;
		done = created = uploaded = present = 0;
		failures = [];
		toSplit = [];
		for (const work of plan.works) {
			const chosen = chosenFiles(work);
			if (!chosen.length) continue;
			let workId = work.decision === 'add' ? work.existingId : null;
			if (!workId) {
				const { data, error: failure } = await api.POST('/works', {
					body: {
						title: work.title.trim(),
						tags: work.tags,
						composer: work.composer.trim() || null,
						arranger: work.arranger.trim() || null,
						lyricist: work.lyricist.trim() || null,
						publisher: work.publisher.trim() || null
					}
				});
				if (!data) {
					for (const f of chosen) failures.push({ path: f.path, message: message(failure) });
					done += chosen.length;
					continue;
				}
				workId = data.id;
				created++;
			}
			let unsplit = 0;
			for (const file of chosen) {
				current = file.path;
				try {
					const source = await uploadSource(
						workId,
						new File([file.file], file.name, { type: 'application/pdf' }),
						() => {}
					);
					uploaded++;
					const part = file.mode === 'part' && file.instrumentId;
					if (part || file.mode === 'score') {
						const { error: failure } = await api.POST('/works/{workId}/parts', {
							params: { path: { workId } },
							body: {
								label: file.label.trim() || file.name,
								instrumentId: part ? file.instrumentId : null,
								voice: part ? file.voice || null : null,
								sourceId: source.id,
								pages: Array.from({ length: source.pageCount }, (_, i) => i + 1)
							}
						});
						if (failure) failures.push({ path: file.path, message: message(failure) });
					} else unsplit++;
				} catch (e) {
					if (e instanceof AlreadyUploaded) present++;
					else
						failures.push({ path: file.path, message: e instanceof Error ? e.message : String(e) });
				}
				done++;
			}
			if (unsplit) toSplit.push({ id: workId, title: work.title, count: unsplit });
		}
		running = false;
		finished = true;
		current = '';
		const { data } = await api.GET('/works');
		existing = data ?? existing;
	}

	function reset() {
		plan = null;
		finished = false;
		failures = [];
	}
</script>

<p class="back"><a href={resolve('/works')}>← {t('Library')}</a></p>
<h1>{t('Import a folder')}</h1>

{#if !isStaff(session.me)}
	<p class="muted">{t('Only admins and librarians add music.')}</p>
{:else if finished}
	<section class="result">
		<p>
			<strong>{t('Import finished.')}</strong>
			{plural(created, '{n} new work', '{n} new works')},
			{plural(uploaded, '{n} file uploaded', '{n} files uploaded')}.
			{#if present}
				{plural(
					present,
					'{n} file was in the library already and was left out.',
					'{n} files were in the library already and were left out.'
				)}
			{/if}
		</p>
		{#if failures.length}
			<h2>{plural(failures.length, '{n} file failed', '{n} files failed')}</h2>
			<ul class="failures">
				{#each failures as failure (failure.path)}
					<li><code>{failure.path}</code> – {failure.message}</li>
				{/each}
			</ul>
		{/if}
		{#if toSplit.length}
			<h2>{t('Still to split')}</h2>
			<p class="muted">{t('These files contain several parts; split them on the work page.')}</p>
			<ul>
				{#each toSplit as work (work.id)}
					<li>
						<a href={resolve('/works/[id]', { id: work.id })}>{work.title}</a>
						<span class="muted">· {plural(work.count, '{n} file', '{n} files')}</span>
					</li>
				{/each}
			</ul>
		{/if}
		<p class="buttons">
			<a class="button" href={resolve('/works')}>{t('To the library')}</a>
			<button onclick={reset}>{t('Import more')}</button>
		</p>
	</section>
{:else if running}
	<section class="progress" aria-live="polite">
		<progress max={files.length} value={done}></progress>
		<p>
			{t('{done} of {total} files', { done, total: files.length })}
			<span class="muted">· {current}</span>
		</p>
		<p class="muted">{t('Keep this page open until the import is finished.')}</p>
	</section>
{:else}
	{#if !plan}
		<p class="muted">
			{t(
				'Each folder becomes a work, the folders above it become tags, and each PDF becomes a part when its name says which instrument it is for ("Trompete 2.pdf"). You check everything before anything is uploaded.'
			)}
		</p>
		<div
			class="drop"
			class:dragging
			role="region"
			aria-label={t('Drop zone')}
			ondragover={(e) => {
				e.preventDefault();
				dragging = true;
			}}
			ondragleave={() => (dragging = false)}
			{ondrop}
		>
			{#if reading}
				<p>{t('Reading files …')}</p>
			{:else}
				<p><strong>{t('Drop a folder or a ZIP file here')}</strong></p>
				<p class="muted">
					{t('Google Drive: right-click the folder → Download gives a ZIP file.')}
				</p>
				<div class="buttons">
					<label class="button">
						{t('Choose ZIP or PDF files …')}
						<input
							type="file"
							accept=".zip,.pdf,application/zip,application/pdf"
							multiple
							onchange={onpick}
						/>
					</label>
					<label class="button">
						{t('Choose a folder …')}
						<input type="file" webkitdirectory multiple onchange={onpick} />
					</label>
				</div>
			{/if}
		</div>
	{/if}

	{#if error}<p class="error">{error}</p>{/if}

	{#if plan && plan.works.length}
		<div class="summary">
			<p>
				<strong>{plural(plan.works.length, '{n} work', '{n} works')}</strong>
				({plural(newWorks, '{n} new', '{n} new')}) ·
				{plural(files.length, '{n} file', '{n} files')}
				{#if plan.skipped.length}· {plural(plan.skipped.length, '{n} skipped', '{n} skipped')}{/if}
				{#if undecided}· <span class="error"
						>{plural(
							undecided,
							'{n} work exists already – decide below',
							'{n} works exist already – decide below'
						)}</span
					>{/if}
				{#if incomplete}· <span class="error"
						>{plural(
							incomplete,
							'{n} part without instrument',
							'{n} parts without instrument'
						)}</span
					>{/if}
			</p>
			<div class="buttons">
				<button onclick={reset}>{t('Choose other files')}</button>
				{#if canRecognize}
					{#if recognizing}
						<span class="muted"
							>{t('Reading pages: {done} of {total}', {
								done: recognized,
								total: toRecognize
							})}</span
						>
						<button onclick={() => (stopRecognizing = true)}>{t('Stop')}</button>
					{:else}
						<button
							onclick={recognizeAll}
							title={t(
								'Reads title, composer, arranger, publisher and part from the first page of each PDF.'
							)}>{t('Recognise from contents')}</button
						>
					{/if}
				{/if}
				<button class="primary" disabled={!files.length} onclick={run}>
					{plural(files.length, 'Import {n} file', 'Import {n} files')}
				</button>
			</div>
		</div>

		{#each plan.works as work (work.id)}
			{@const active = chosenFiles(work)}
			{@const adding = !!work.existingId && work.decision === 'add'}
			<details
				class="work"
				class:muted={!active.length}
				open={(!!work.existingId && work.decision === 'open') ||
					active.some((f) => f.mode !== 'part')}
			>
				<summary>
					<span class="title">{work.title}</span>
					{#if work.existingId}
						<span class="badge" class:open={work.decision === 'open'}>
							{work.decision === 'add'
								? t('adds to the existing work')
								: work.decision === 'new'
									? t('separate work with the same title')
									: t('exists already – please decide')}
						</span>
					{/if}
					{#each work.tags as tag (tag)}<span class="tag">{tag}</span>{/each}
					<span class="muted">{plural(active.length, '{n} file', '{n} files')}</span>
				</summary>
				{#if work.existingId}
					<div class="namesake">
						<p>
							{t('The library has a work with this title already:')}
							<a href={resolve('/works/[id]', { id: work.existingId })} target="_blank"
								>{existing.find((w) => w.id === work.existingId)?.title ?? work.title}</a
							>
						</p>
						<Select
							value={work.decision}
							label={t('What to do with {name}', { name: work.title })}
							options={decisions}
							onchange={(v) => (work.decision = v as PlannedWork['decision'])}
						/>
					</div>
				{/if}
				{#if work.suggestedTitle && !adding}
					<p class="suggestion">
						{t('The first page says: “{title}”', { title: work.suggestedTitle })}
						<button
							class="link"
							onclick={() => {
								work.title = work.suggestedTitle ?? work.title;
								work.suggestedTitle = null;
							}}>{t('Use this title')}</button
						>
						<button class="link" onclick={() => (work.suggestedTitle = null)}
							>{t('Keep the folder name')}</button
						>
					</p>
				{/if}
				<div class="work-fields">
					<label>
						{t('Title')}
						<input bind:value={work.title} required maxlength="300" disabled={adding} />
					</label>
					{#if !adding}
						<label>
							{t('Composer')}
							<Autocomplete bind:value={work.composer} suggestions={facets.composers} />
						</label>
						<label>
							{t('Arranger')}
							<Autocomplete bind:value={work.arranger} suggestions={facets.arrangers} />
						</label>
						<label>
							{t('Lyricist')}
							<input bind:value={work.lyricist} />
						</label>
						<label>
							{t('Publisher')}
							<Autocomplete bind:value={work.publisher} suggestions={facets.publishers} />
						</label>
						<label class="grow">
							{t('Tags')}
							<TagInput bind:value={work.tags} suggestions={facets.tags} label={t('Tags')} />
						</label>
					{/if}
					<button class="link" onclick={() => skipWork(work)}>{t('Leave out this work')}</button>
				</div>
				<table>
					<thead>
						<tr>
							<th>{t('File')}</th>
							<th>{t('Import as')}</th>
							<th>{t('Instrument')}</th>
							<th>{t('No.')}</th>
							<th>{t('Label')}</th>
						</tr>
					</thead>
					<tbody>
						{#each work.files as file (file.id)}
							<tr class:skipped={file.mode === 'skip'}>
								<td class="file">
									{file.name} <span class="muted">{size(file.size)}</span>
									{#if file.found}
										<span class="found">{foundSummary(file.found) || t('nothing found')}</span>
									{/if}
								</td>
								<td>
									<Select
										value={file.mode}
										label={t('Import {name} as', { name: file.name })}
										options={modes}
										onchange={(v) => setMode(file, v as Mode)}
									/>
								</td>
								<td>
									{#if file.mode === 'part'}
										<Select
											value={file.instrumentId}
											placeholder={t('Instrument …')}
											label={t('Instrument')}
											{options}
											onchange={(v) => setInstrument(file, v)}
										/>
									{/if}
								</td>
								<td>
									{#if file.mode === 'part'}
										<input
											class="voice"
											type="number"
											min="1"
											max="99"
											bind:value={file.voice}
											aria-label={t('Part number')}
										/>
									{/if}
								</td>
								<td>
									{#if file.mode === 'part' || file.mode === 'score'}
										<input bind:value={file.label} maxlength="120" aria-label={t('Label')} />
									{/if}
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</details>
		{/each}

		{#if plan.skipped.length}
			<details class="skipped-files">
				<summary
					>{plural(plan.skipped.length, '{n} file is skipped', '{n} files are skipped')}</summary
				>
				<ul>
					{#each plan.skipped as skipped (skipped.path)}
						<li>
							<code>{skipped.path}</code> –
							{skipped.reason === 'too large'
								? t('larger than this server allows')
								: t('not a PDF')}
						</li>
					{/each}
				</ul>
			</details>
		{/if}
	{/if}
{/if}

<style>
	.back {
		margin: 0;
	}
	.back a {
		color: var(--muted);
		text-decoration: none;
	}
	h2 {
		font-size: 1.1rem;
		margin: 1.5rem 0 0.5rem;
	}
	.drop {
		border: 2px dashed var(--line-strong);
		border-radius: 12px;
		padding: 2.5rem 1rem;
		text-align: center;
		background: var(--surface);
	}
	.drop.dragging {
		border-color: var(--accent);
		background: var(--selected);
	}
	.drop p {
		margin: 0.25rem 0;
	}
	.buttons {
		display: flex;
		flex-wrap: wrap;
		justify-content: center;
		gap: 0.5rem;
		margin-top: 1rem;
	}
	label.button input {
		display: none;
	}
	.summary {
		position: sticky;
		top: 0;
		z-index: 5;
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem 1rem;
		padding: 0.6rem 0;
		background: var(--bg);
		border-bottom: 1px solid var(--line);
		margin-bottom: 0.75rem;
	}
	.summary p {
		margin: 0;
	}
	.summary .buttons {
		margin: 0;
	}
	.work {
		border: 1px solid var(--line);
		border-radius: var(--radius);
		margin-bottom: 0.5rem;
		background: var(--raised);
	}
	.work > summary {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.25rem 0.6rem;
		padding: 0.6rem 0.9rem;
	}
	.work .title {
		font-weight: 600;
	}
	.badge,
	.tag {
		font-size: 0.8em;
		border-radius: 999px;
		padding: 0.1rem 0.5rem;
		border: 1px solid var(--line);
		background: var(--surface);
	}
	.badge {
		border-color: var(--accent);
	}
	.badge.open {
		border-color: var(--danger);
		color: var(--danger);
		background: var(--danger-bg);
	}
	.namesake {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.5rem 1rem;
		margin: 0 0.9rem 0.75rem;
		padding: 0.5rem 0.75rem;
		border-radius: var(--radius);
		background: var(--danger-bg);
	}
	.namesake p {
		margin: 0;
	}
	.work-fields {
		display: flex;
		flex-wrap: wrap;
		align-items: end;
		gap: 0.75rem;
		padding: 0 0.9rem 0.75rem;
	}
	.work-fields label {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		font-size: 0.85em;
		color: var(--muted);
	}
	.work-fields label > :global(*) {
		color: var(--text);
		font-size: 1rem;
	}
	.work-fields .grow {
		flex: 1 1 16rem;
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.95em;
	}
	th,
	td {
		text-align: left;
		padding: 0.35rem 0.5rem;
		border-top: 1px solid var(--line);
		vertical-align: middle;
	}
	th {
		font-size: 0.8em;
		color: var(--muted);
		font-weight: 600;
	}
	th:first-child,
	td:first-child {
		padding-left: 0.9rem;
	}
	.file {
		word-break: break-word;
	}
	.found {
		display: block;
		font-size: 0.85em;
		color: var(--accent);
	}
	.suggestion {
		margin: 0 0.9rem 0.6rem;
		font-size: 0.95em;
	}
	.suggestion button {
		margin-left: 0.5rem;
	}
	.voice {
		width: 4.5rem;
	}
	tr.skipped td:not(:nth-child(2)) {
		opacity: 0.45;
	}
	.progress progress {
		width: 100%;
		height: 1rem;
	}
	.failures code,
	.skipped-files code {
		font-size: 0.85em;
	}
	.result .buttons {
		justify-content: flex-start;
	}
	@media (max-width: 760px) {
		table,
		thead,
		tbody,
		tr,
		td {
			display: block;
		}
		thead {
			display: none;
		}
		tr {
			border-top: 1px solid var(--line);
			padding: 0.4rem 0.9rem;
		}
		td,
		td:first-child {
			border: none;
			padding: 0.2rem 0;
		}
	}
</style>
