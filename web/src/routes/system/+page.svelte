<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- For admins: setting notenwart up step by step, and what this server is and can do. -->
<script lang="ts">
	import { resolve } from '$app/paths';
	import { api, message, type System } from '#lib/api/index.js';
	import { session } from '#lib/session.svelte.js';
	import { i18n, t } from '#lib/i18n/index.svelte.js';
	import { notify } from '#lib/dialog.svelte.js';
	import { bytes, open } from '#lib/setup.js';
	import Icon from '#lib/components/Icon.svelte';

	let system = $state<System | null>(null);
	let sending = $state(false);

	async function load() {
		const { data } = await api.GET('/system');
		system = data ?? null;
	}

	$effect(() => {
		load();
	});

	async function tick(key: 'instruments' | 'backups' | 'testMail', done: boolean) {
		await api.PUT('/system/checklist/{key}', { params: { path: { key } }, body: { done } });
		await load();
	}

	async function testMail() {
		sending = true;
		const { data, error } = await api.POST('/system/test-mail');
		sending = false;
		if (!data) return notify(message(error), 'error');
		notify(
			data.sent
				? t('Sent to {address}. Check the inbox (and the spam folder).', { address: data.to })
				: t('The mail server did not take it – see the server log.'),
			data.sent ? 'info' : 'error'
		);
		await load();
	}

	const steps = $derived<Record<string, { title: string; text: string }>>({
		logo: {
			title: t('Name and logo of the organisation'),
			text: t('Shown in the header and on the sign-in page.')
		},
		instruments: {
			title: t('Check the instruments'),
			text: t(
				'Spellings your organisation uses, instruments that are missing, the order of families.'
			)
		},
		ensembles: {
			title: t('Create the ensembles'),
			text: t('Choirs, orchestras, classes, courses.')
		},
		people: {
			title: t('Add people'),
			text: t('One by one or from a spreadsheet, with their ensembles and instruments.')
		},
		music: {
			title: t('Bring in the music'),
			text: t('Import a folder (e.g. a Google Drive download) or upload works one by one.')
		},
		mail: {
			title: t('Email'),
			text: t(
				'SMTP_URL and MAIL_FROM in deploy/.env – for sign-in links, invitations and notifications.'
			)
		},
		testMail: { title: t('Send a test email'), text: t('To yourself, to see that email arrives.') },
		sso: {
			title: t('Sign-in with school accounts'),
			text: t('Optional: NOTENWART_SSO_* in deploy/.env (docs/operations.md).')
		},
		backups: {
			title: t('Back up regularly'),
			text: t(
				'deploy/backup.sh every night, with a copy away from the server (docs/operations.md).'
			)
		}
	});
	const links: Record<string, string> = {
		logo: resolve('/organization'),
		instruments: resolve('/instruments'),
		ensembles: resolve('/ensembles'),
		people: resolve('/people'),
		music: resolve('/works/import')
	};
</script>

<h1>{t('System')}</h1>

{#if session.me?.role !== 'admin'}
	<p class="muted">{t('Only admins see the system.')}</p>
{:else if system}
	{@const left = open(system.checklist)}
	<section>
		<h2>{t('Setting up')}</h2>
		<p class="muted">
			{left.length
				? t('{done} of {total} steps done.', {
						done: system.checklist.filter((s) => s.done && !s.optional).length,
						total: system.checklist.filter((s) => !s.optional).length
					})
				: t('Everything is set up.')}
		</p>
		<ol class="steps">
			{#each system.checklist as step (step.key)}
				<li class:done={step.done}>
					<span class="mark" aria-hidden="true">
						{#if step.done}<Icon name="check" size={16} />{/if}
					</span>
					<div class="what">
						<strong>
							{#if links[step.key]}
								<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- resolved above -->
								<a href={links[step.key]}>{steps[step.key].title}</a>
							{:else}
								{steps[step.key].title}
							{/if}
							{#if step.optional}<span class="muted optional">({t('optional')})</span>{/if}
						</strong>
						<span class="muted">{steps[step.key].text}</span>
					</div>
					<span class="actions">
						{#if step.key === 'testMail'}
							<button disabled={sending || !system.mailFrom} onclick={testMail}
								>{t('Send test email')}</button
							>
						{:else if step.key === 'instruments' || step.key === 'backups'}
							<button onclick={() => tick(step.key as 'instruments' | 'backups', !step.done)}>
								{step.done ? t('Not done') : t('Done')}
							</button>
						{/if}
					</span>
				</li>
			{/each}
		</ol>
	</section>

	<section>
		<h2>{t('This server')}</h2>
		<dl>
			<dt>{t('Version')}</dt>
			<dd>notenwart {system.version}</dd>
			<dt>{t('Address')}</dt>
			<dd>{system.publicUrl}</dd>
			<dt>{t('Language of emails')}</dt>
			<dd>{system.language === 'de' ? 'Deutsch' : 'English'}</dd>
			<dt>{t('Email')}</dt>
			<dd>{system.mailFrom ?? t('none – links only appear in the log')}</dd>
			<dt>{t('Files')}</dt>
			<dd>
				{system.storage === 's3' ? t('S3 storage') : t('folder on the server')} ·
				{bytes(system.counts.storedBytes, i18n.locale)} ·
				{t('up to {size} per file', { size: bytes(system.maxUploadBytes, i18n.locale) })}
			</dd>
			<dt>{t('Reading sheet music')}</dt>
			<dd>
				{[
					system.recognition.text && t('text in PDFs'),
					system.recognition.ocr && t('scans (OCR)'),
					system.ai && t('AI: {name}', { name: system.ai })
				]
					.filter(Boolean)
					.join(' · ') || t('off')}
			</dd>
			<dt>{t('Single sign-on')}</dt>
			<dd>{system.sso ?? t('off')}</dd>
			<dt>{t('Contents')}</dt>
			<dd>
				{t(
					'{people} people, {ensembles} ensembles, {works} works, {parts} parts, {recordings} recordings',
					{
						...system.counts
					}
				)}
			</dd>
		</dl>
	</section>
{/if}

<style>
	h2 {
		font-size: 1.2rem;
		margin-top: 1.5rem;
	}
	.steps {
		list-style: none;
		padding: 0;
		margin: 0;
		border-top: 1px solid var(--line);
	}
	.steps li {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 0.6rem 0;
		border-bottom: 1px solid var(--line);
	}
	.mark {
		flex: none;
		width: 1.5rem;
		height: 1.5rem;
		border-radius: 50%;
		border: 2px solid var(--line-strong);
		display: inline-flex;
		align-items: center;
		justify-content: center;
	}
	.done .mark {
		background: var(--success);
		border-color: var(--success);
		color: #fff;
	}
	.what {
		flex: 1;
		display: flex;
		flex-direction: column;
	}
	.done .what strong {
		color: var(--muted);
	}
	.optional {
		font-weight: normal;
	}
	dl {
		display: grid;
		grid-template-columns: max-content 1fr;
		gap: 0.35rem 1.25rem;
	}
	dt {
		color: var(--muted);
	}
	dd {
		margin: 0;
		word-break: break-word;
	}
</style>
