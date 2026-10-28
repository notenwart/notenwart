<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import {
		api,
		message,
		type BulkInput,
		type Deletion,
		type Ensemble,
		type Instrument,
		type Invitation,
		type Role,
		type User
	} from '#lib/api/index.js';
	import { roleNames, session } from '#lib/session.svelte.js';
	import { i18n, plural, t } from '#lib/i18n/index.svelte.js';
	import { confirm, notify } from '#lib/dialog.svelte.js';
	import { toCsv } from '#lib/csv.js';
	import { download } from '#lib/download.js';
	import { instrumentOptions } from '#lib/instruments.js';
	import DataTable, { type Column } from '#lib/components/DataTable.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import Avatar from '#lib/components/Avatar.svelte';
	import ImportPeople from '#lib/components/ImportPeople.svelte';
	import Select from '#lib/components/Select.svelte';

	let users = $state<User[]>([]);
	let ensembles = $state<Ensemble[]>([]);
	let instruments = $state<Instrument[]>([]);
	let error = $state('');
	let invitation = $state<Invitation | null>(null);
	let selected = $state<string[]>([]);

	let name = $state('');
	let email = $state('');
	let role = $state<Role>('member');
	let busy = $state(false);

	// Bulk: which action is being set up, and its choices.
	let action = $state<'' | 'addToEnsemble' | 'removeFromEnsemble' | 'setRole'>('');
	let bulkEnsemble = $state('');
	let bulkInstrument = $state('');
	let bulkVoice = $state<number | null>(null);
	let bulkRole = $state<Role>('member');

	const roles = Object.keys(roleNames) as Role[];
	const roleOptions = $derived(roles.map((r) => ({ value: r, label: t(roleNames[r]) })));
	const ensembleOptions = $derived(ensembles.map((e) => ({ value: e.id, label: e.name })));

	function status(user: User) {
		if (user.lastLoginAt)
			return t('Last seen {date}', {
				date: new Date(user.lastLoginAt).toLocaleDateString(i18n.locale)
			});
		return user.hasPassword ? t('Has a password') : t('Invited');
	}

	const columns = $derived<Column<User>[]>([
		{ id: 'name', label: t('Name'), value: (u) => u.name },
		{ id: 'email', label: t('Email'), value: (u) => u.email, wide: true },
		{ id: 'role', label: t('Role'), value: (u) => t(roleNames[u.role]) },
		{
			id: 'ensembles',
			label: t('Ensembles'),
			value: (u) => (u.ensembles ?? []).map((e) => e.name).join(', '),
			wide: true
		},
		{
			id: 'status',
			label: t('Status'),
			value: (u) => (u.lastLoginAt ? `1 ${u.lastLoginAt}` : u.hasPassword ? '2' : '3'),
			wide: true
		},
		{ id: 'actions', label: '', value: () => '', sortable: false, align: 'right' }
	]);

	async function load() {
		const [u, e, i] = await Promise.all([
			api.GET('/users'),
			api.GET('/ensembles'),
			api.GET('/instruments')
		]);
		if (u.data) users = u.data;
		else error = message(u.error);
		ensembles = e.data ?? [];
		instruments = i.data ?? [];
		selected = selected.filter((id) => users.some((u) => u.id === id));
	}

	$effect(() => {
		load();
	});

	async function create(event: SubmitEvent) {
		event.preventDefault();
		busy = true;
		error = '';
		const { data, error: failure } = await api.POST('/users', { body: { name, email, role } });
		busy = false;
		if (data) {
			invitation = data;
			name = email = '';
			role = 'member';
			await load();
		} else error = message(failure);
	}

	async function changeRole(user: User, newRole: Role) {
		const { error: failure } = await api.PUT('/users/{userId}', {
			params: { path: { userId: user.id } },
			body: { name: user.name, email: user.email ?? '', role: newRole }
		});
		if (failure) notify(message(failure), 'error');
		else notify(t('Saved.'));
		await load();
	}

	async function invite(user: User) {
		const { data, error: failure } = await api.POST('/users/{userId}/invitation', {
			params: { path: { userId: user.id } }
		});
		if (data) invitation = data;
		else notify(message(failure), 'error');
	}

	async function remove(user: User) {
		const ok = await confirm({
			title: t('Delete {name}?', { name: user.name }),
			message: t('They lose access right away. This cannot be undone.'),
			confirm: t('Delete'),
			danger: true
		});
		if (!ok) return;
		const { error: failure } = await api.DELETE('/users/{userId}', {
			params: { path: { userId: user.id } }
		});
		if (failure) notify(message(failure), 'error');
		await load();
	}

	async function runBulk(body: Omit<BulkInput, 'userIds'>) {
		busy = true;
		const { data, error: failure } = await api.POST('/users/bulk', {
			body: { userIds: selected, ...body }
		});
		busy = false;
		if (!data) {
			notify(message(failure), 'error');
			return null;
		}
		const skipped = data.skipped.length
			? ` ${plural(data.skipped.length, '{n} skipped.', '{n} skipped.')}`
			: '';
		notify(plural(data.done, 'Done for {n} person.', 'Done for {n} people.') + skipped);
		action = '';
		await load();
		return data;
	}

	async function applyBulk() {
		if (action === 'setRole') await runBulk({ action, role: bulkRole });
		else if (action === 'addToEnsemble')
			await runBulk({
				action,
				ensembleId: bulkEnsemble,
				instrumentId: bulkInstrument || null,
				voice: bulkInstrument ? bulkVoice || null : null
			});
		else if (action === 'removeFromEnsemble') await runBulk({ action, ensembleId: bulkEnsemble });
	}

	async function bulkInvite() {
		const data = await runBulk({ action: 'invite' });
		if (!data?.invitations.length) return;
		const rows = data.invitations.map((i) => [i.name, i.email, i.url]);
		download(
			i18n.locale === 'de' ? 'einladungen.csv' : 'invitations.csv',
			toCsv([[t('Name'), t('Email'), t('Invitation link')], ...rows])
		);
	}

	async function bulkDelete() {
		const ok = await confirm({
			title: plural(selected.length, 'Delete {n} person?', 'Delete {n} people?'),
			message: t('They lose access right away. This cannot be undone.'),
			confirm: t('Delete'),
			danger: true
		});
		if (ok) {
			await runBulk({ action: 'delete' });
			selected = [];
		}
	}

	async function exportUser(user: User) {
		const { data, error: failure } = await api.GET('/users/{userId}/export', {
			params: { path: { userId: user.id } }
		});
		if (!data) return notify(message(failure), 'error');
		const name = user.name.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, '-');
		download(`notenwart-data-${name}.json`, JSON.stringify(data, null, 2), 'application/json');
	}

	/**
	 * People who have left: in no ensemble and not signed in for a year (or never, invited long ago). Admins and the
	 * signed-in user are never picked.
	 */
	function selectFormer() {
		const yearAgo = Date.now() - 365 * 24 * 3600 * 1000;
		selected = users
			.filter(
				(u) =>
					u.role !== 'admin' &&
					u.id !== session.me?.id &&
					!(u.ensembles ?? []).length &&
					(!u.lastLoginAt || new Date(u.lastLoginAt).getTime() < yearAgo)
			)
			.map((u) => u.id);
		notify(
			selected.length
				? plural(selected.length, '{n} person selected.', '{n} people selected.')
				: t('Nobody has left: everyone is in an ensemble or was here this year.')
		);
	}

	let deletions = $state<Deletion[] | null>(null);
	const recordNames: Record<string, string> = {
		ensembles: 'Ensembles',
		annotations: 'Annotations',
		singleParts: 'Single parts',
		loans: 'Loans',
		sessions: 'Sign-ins'
	};
	async function loadDeletions() {
		const { data } = await api.GET('/deletions');
		deletions = data ?? [];
	}

	async function copy() {
		if (!invitation) return;
		await navigator.clipboard.writeText(invitation.url);
		notify(t('Copied'));
	}
</script>

<h1>{t('People')}</h1>

{#if session.me?.role !== 'admin'}
	<p class="muted">{t('Only admins manage people.')}</p>
{:else}
	<form class="add" onsubmit={create}>
		<input placeholder={t('Name')} bind:value={name} required maxlength="120" />
		<input type="email" placeholder={t('Email')} bind:value={email} required />
		<Select bind:value={role} label={t('Role')} options={roleOptions} />
		<button class="primary" disabled={busy}>{t('Add and invite')}</button>
	</form>

	{#if invitation}
		<div class="invitation">
			<p>
				<strong>{t('Invitation for {name}', { name: invitation.user.name })}</strong> –
				{invitation.emailed
					? t('sent by email. You can also hand over this link:')
					: t('this server sends no email, so hand over this link yourself:')}
			</p>
			<div class="link">
				<input readonly value={invitation.url} aria-label={t('Invitation link')} />
				<button onclick={copy}>{t('Copy')}</button>
			</div>
			<p class="muted">
				{t('Works once, until {date}.', {
					date: new Date(invitation.expiresAt).toLocaleString(i18n.locale)
				})}
				<button class="link" onclick={() => (invitation = null)}>{t('Close')}</button>
			</p>
		</div>
	{/if}

	{#if error}<p class="error">{error}</p>{/if}

	<DataTable
		id="people"
		rows={users}
		key={(u) => u.id}
		{columns}
		bind:selected
		empty={t('No people yet.')}
		filterPlaceholder={t('Filter by name, email, ensemble …')}
	>
		{#snippet actions()}
			<button
				onclick={selectFormer}
				title={t('In no ensemble and not seen for a year – for tidying up after the school year.')}
				>{t('Select former members')}</button
			>
		{/snippet}
		{#snippet cell(user, column)}
			{#if column.id === 'name'}
				<span class="named"
					><Avatar name={user.name} url={user.avatarUrl} size={26} />{user.name}</span
				>
			{:else if column.id === 'role'}
				<Select
					value={user.role}
					disabled={user.id === session.me?.id}
					label={t('Role of {name}', { name: user.name })}
					options={roleOptions}
					onchange={(v) => changeRole(user, v as Role)}
				/>
			{:else if column.id === 'status'}
				<span class="muted">{status(user)}</span>
			{:else if column.id === 'actions'}
				{#if user.id !== session.me?.id}
					<span class="row-actions">
						<button class="icon" title={t('Export data')} onclick={() => exportUser(user)}>
							<Icon name="download" label={t('Export data')} />
						</button>
						<button class="icon" title={t('New sign-in link')} onclick={() => invite(user)}>
							<Icon name="link" label={t('New sign-in link')} />
						</button>
						<button class="icon danger" title={t('Delete')} onclick={() => remove(user)}>
							<Icon name="trash" label={t('Delete')} />
						</button>
					</span>
				{/if}
			{:else}
				{column.value(user) ?? ''}
			{/if}
		{/snippet}
		{#snippet bulk()}
			{#if action === ''}
				<button onclick={() => (action = 'addToEnsemble')}>{t('Add to ensemble …')}</button>
				<button onclick={() => (action = 'removeFromEnsemble')}
					>{t('Remove from ensemble …')}</button
				>
				<button onclick={() => (action = 'setRole')}>{t('Change role …')}</button>
				<button onclick={bulkInvite} disabled={busy}>{t('New invitation links')}</button>
				<button class="danger" onclick={bulkDelete} disabled={busy}>{t('Delete')}</button>
			{:else}
				{#if action === 'setRole'}
					<Select bind:value={bulkRole} label={t('Role')} options={roleOptions} />
				{:else}
					<Select
						bind:value={bulkEnsemble}
						label={t('Ensemble')}
						placeholder={t('Ensemble …')}
						options={ensembleOptions}
					/>
					{#if action === 'addToEnsemble'}
						<Select
							bind:value={bulkInstrument}
							label={t('Instrument')}
							options={[
								{ value: '', label: t('No instrument') },
								...instrumentOptions(instruments)
							]}
						/>
						{#if bulkInstrument}
							<input
								class="voice"
								type="number"
								min="1"
								max="99"
								placeholder={t('all')}
								bind:value={bulkVoice}
								aria-label={t('Part number')}
							/>
						{/if}
					{/if}
				{/if}
				<button
					class="primary"
					disabled={busy || (action !== 'setRole' && !bulkEnsemble)}
					onclick={applyBulk}>{t('Apply')}</button
				>
				<button onclick={() => (action = '')}>{t('Cancel')}</button>
			{/if}
		{/snippet}
	</DataTable>

	<ImportPeople onimported={load} />

	<details
		class="deletions"
		ontoggle={(e) => (e.currentTarget as HTMLDetailsElement).open && !deletions && loadDeletions()}
	>
		<summary>{t('Deletion record')}</summary>
		<p class="muted">
			{t(
				'Proof that people were deleted: when, by whom and how many records went – without their data.'
			)}
		</p>
		{#if deletions === null}
			<p class="muted">{t('Loading…')}</p>
		{:else if deletions.length === 0}
			<p class="muted">{t('Nobody has been deleted yet.')}</p>
		{:else}
			<table>
				<tbody>
					{#each deletions as d (d.userId + d.deletedAt)}
						<tr>
							<td>{new Date(d.deletedAt).toLocaleString(i18n.locale)}</td>
							<td>{d.deletedBy ?? '–'}</td>
							<td class="muted">
								{Object.entries(d.records)
									.filter(([, n]) => n)
									.map(([kind, n]) => `${t(recordNames[kind] ?? kind)}: ${n}`)
									.join(' · ') || t('no records')}
							</td>
							<td class="muted"><code>{d.userId.slice(0, 8)}</code></td>
						</tr>
					{/each}
				</tbody>
			</table>
		{/if}
	</details>
{/if}

<style>
	.add {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
		margin-bottom: 1rem;
	}
	.add input {
		flex: 1 1 12rem;
	}
	.invitation {
		background: var(--selected);
		border-radius: var(--radius);
		padding: 0.75rem 1rem;
		margin-bottom: 1rem;
	}
	.invitation p {
		margin: 0.25rem 0;
	}
	.link {
		display: flex;
		gap: 0.5rem;
	}
	.link input {
		flex: 1;
		font-family: ui-monospace, monospace;
		font-size: 0.85em;
	}
	.named {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
	}
	.row-actions {
		display: inline-flex;
		gap: 0.4rem;
		white-space: nowrap;
	}
	.voice {
		width: 5rem;
	}
	.deletions {
		margin-top: 2rem;
	}
	.deletions summary {
		font-weight: 600;
	}
</style>
