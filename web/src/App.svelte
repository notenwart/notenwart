<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
	import { i18n, t } from '#lib/i18n/index.svelte.js';
	import { goto, page, resolve, start, view } from '#lib/router.svelte.js';
	import { isStaff, loadSession, readsLibrary, session, signOut } from '#lib/session.svelte.js';
	import { nextTheme, setTheme, theme } from '#lib/theme.svelte.js';
	import Icon from '#lib/components/Icon.svelte';
	import Dialogs from '#lib/components/Dialogs.svelte';
	import Avatar from '#lib/components/Avatar.svelte';
	import { notifications, watchUnread } from '#lib/notifications.svelte.js';
	import { routes } from './routes.js';

	start(routes);

	const publicPaths = ['/login', '/setup', '/auth'];
	// Guest links work without an account.
	const isPublic = $derived(
		publicPaths.includes(page.url.pathname) || page.url.pathname.startsWith('/guest/')
	);

	$effect(() => {
		loadSession();
	});

	// The bell's count, while someone is signed in and online.
	$effect(() => {
		if (session.me && !session.offline) return watchUnread();
	});

	// The page's language follows the chosen locale (screen readers, hyphenation).
	$effect(() => {
		document.documentElement.lang = i18n.locale;
	});

	// Route guard: setup first, then sign-in.
	$effect(() => {
		const status = session.status;
		if (!status) return;
		const path = page.url.pathname;
		if (status.setupRequired) {
			if (path !== '/setup') goto(resolve('/setup'));
		} else if (!session.me && !isPublic) {
			goto(resolve(`/login?next=${encodeURIComponent(path + page.url.search)}`));
		}
	});

	const themeLabels = $derived({
		system: t('Colours: as on this device'),
		light: t('Colours: light'),
		dark: t('Colours: dark')
	});

	// The menu on narrow screens closes after each navigation.
	let menu = $state(false);
	$effect(() => {
		void page.url.pathname;
		menu = false;
	});

	function active(prefix: string) {
		const path = page.url.pathname;
		return (prefix === '/' ? path === '/' : path.startsWith(prefix)) ? 'page' : undefined;
	}
</script>

<svelte:head>
	<title>notenwart</title>
</svelte:head>

<header class="site">
	<a class="brand" href={resolve('/')}>
		<!-- Both marks are loaded; the colour scheme (system or chosen) shows one. -->
		<img class="mark-light" src="/notenwart-mark-on-light.svg" alt="" width="28" height="28" />
		<img class="mark-dark" src="/notenwart-mark-on-dark.svg" alt="" width="28" height="28" />
		notenwart
	</a>
	{#if session.me?.organization.logoUrl}
		<span class="organization" title={session.me.organization.name}>
			<Avatar
				name={session.me.organization.name}
				url={session.me.organization.logoUrl}
				size={28}
				square
			/>
		</span>
	{/if}
	{#if session.me}
		<button
			class="icon menu"
			aria-expanded={menu}
			aria-controls="site-menu"
			aria-label={t('Menu')}
			onclick={() => (menu = !menu)}
		>
			<Icon name={menu ? 'x' : 'menu'} />
		</button>
		<div id="site-menu" class="site-menu" class:open={menu}>
			<nav>
				<a href={resolve('/')} aria-current={active('/')}>{t('My music')}</a>
				{#if readsLibrary(session.me)}
					<a href={resolve('/works')} aria-current={active('/works')}>{t('Library')}</a>
				{/if}
				<a href={resolve('/ensembles')} aria-current={active('/ensembles')}>{t('Ensembles')}</a>
				{#if isStaff(session.me)}
					<a href={resolve('/instruments')} aria-current={active('/instruments')}
						>{t('Instruments')}</a
					>
				{/if}
				{#if session.me.role === 'admin'}
					<a href={resolve('/people')} aria-current={active('/people')}>{t('People')}</a>
					<a href={resolve('/system')} aria-current={active('/system') ?? active('/organization')}
						>{t('System')}</a
					>
				{/if}
			</nav>
			<div class="account">
				<a
					class="bell"
					href={resolve('/notifications')}
					aria-current={active('/notifications')}
					title={t('Notifications')}
				>
					<Icon name="bell" label={t('Notifications')} />
					{#if notifications.unread}<span class="badge"
							>{notifications.unread > 99 ? '99+' : notifications.unread}</span
						>{/if}
				</a>
				<a class="me" href={resolve('/account')} aria-current={active('/account')}
					><Avatar name={session.me.name} url={session.me.avatarUrl} size={26} />
					<span>{session.me.name}</span></a
				>
				<button class="link" onclick={signOut}>{t('Sign out')}</button>
			</div>
		</div>
	{/if}
	<button
		class="icon theme"
		onclick={() => setTheme(nextTheme(theme.value))}
		title={themeLabels[theme.value]}
		aria-label={themeLabels[theme.value]}
	>
		<Icon name={theme.value === 'light' ? 'sun' : theme.value === 'dark' ? 'moon' : 'auto'} />
	</button>
</header>

<Dialogs />

<main>
	{#if session.status && (session.me || isPublic)}
		{#if view.component}
			<view.component />
		{:else if view.missing}
			<h1>{t('Page not found')}</h1>
			<p><a href={resolve('/')}>{t('My music')}</a></p>
		{/if}
	{:else if !session.status}
		<p class="muted">{t('Loading…')}</p>
	{/if}
</main>

<style>
	header {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.5rem 1.5rem;
		padding: 0.75rem 1rem;
		border-bottom: 1px solid var(--line);
	}
	.brand {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		font-family: var(--serif);
		font-size: 1.15rem;
		font-weight: 600;
		color: var(--text);
		text-decoration: none;
	}
	.mark-dark,
	:global(:root[data-theme='dark']) .mark-light {
		display: none;
	}
	:global(:root[data-theme='dark']) .mark-dark {
		display: block;
	}
	@media (prefers-color-scheme: dark) {
		:global(:root:not([data-theme='light'])) .mark-light {
			display: none;
		}
		:global(:root:not([data-theme='light'])) .mark-dark {
			display: block;
		}
	}
	.site-menu {
		display: contents;
	}
	.menu {
		display: none;
	}
	nav {
		display: flex;
		flex-wrap: wrap;
		gap: 0.25rem 1rem;
	}
	nav a,
	.account a {
		color: var(--muted);
		text-decoration: none;
	}
	nav a[aria-current='page'],
	.account a[aria-current='page'] {
		color: var(--text);
		font-weight: 600;
	}
	.theme {
		border-color: transparent;
		background: none;
	}
	header:not(:has(.account)) .theme {
		margin-left: auto;
	}
	.organization {
		display: inline-flex;
		margin-left: -0.75rem;
	}
	.me {
		display: inline-flex;
		align-items: center;
		gap: 0.4rem;
	}
	.bell {
		position: relative;
		display: inline-flex;
	}
	.badge {
		position: absolute;
		top: -0.45rem;
		right: -0.6rem;
		min-width: 1.1rem;
		padding: 0 0.25rem;
		border-radius: 999px;
		background: var(--accent);
		color: var(--accent-text);
		font-size: 0.7rem;
		font-weight: 700;
		line-height: 1.1rem;
		text-align: center;
	}
	.account {
		margin-left: auto;
		display: flex;
		gap: 1rem;
		align-items: center;
	}
	@media (max-width: 760px) {
		.menu {
			display: inline-flex;
			margin-left: auto;
			border-color: transparent;
			background: none;
		}
		.site-menu {
			display: none;
			order: 10;
			width: 100%;
			flex-direction: column;
			gap: 0.75rem;
			padding: 0.5rem 0 0.25rem;
		}
		.site-menu.open {
			display: flex;
		}
		nav {
			flex-direction: column;
			gap: 0;
		}
		nav a {
			padding: 0.6rem 0;
			font-size: 1.05rem;
		}
		.account {
			margin-left: 0;
			padding-top: 0.75rem;
			border-top: 1px solid var(--line);
		}
		header .theme {
			margin-left: 0;
		}
	}
	main {
		max-width: 72rem;
		margin: 0 auto;
		padding: 1.5rem 1rem 4rem;
	}
</style>
