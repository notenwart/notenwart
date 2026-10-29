// SPDX-License-Identifier: AGPL-3.0-or-later
// Every page of the app by path. Static routes come before ones with parameters (`/works/import` before
// `/works/[id]`); the first that fits wins.
import type { Route } from '#lib/router.svelte.js';

export const routes: Route[] = [
	{ pattern: '/', load: () => import('./pages/Home.svelte') },
	{ pattern: '/account', load: () => import('./pages/Account.svelte') },
	{ pattern: '/auth', load: () => import('./pages/Auth.svelte') },
	{ pattern: '/ensembles', load: () => import('./pages/Ensembles.svelte') },
	{ pattern: '/ensembles/[id]', load: () => import('./pages/Ensemble.svelte') },
	{
		pattern: '/ensembles/[id]/programs/[programId]',
		load: () => import('./pages/Program.svelte')
	},
	{
		pattern: '/ensembles/[id]/programs/[programId]/setlist',
		load: () => import('./pages/Setlist.svelte')
	},
	{ pattern: '/guest/[token]', load: () => import('./pages/Guest.svelte') },
	{ pattern: '/instruments', load: () => import('./pages/Instruments.svelte') },
	{ pattern: '/loans', load: () => import('./pages/Loans.svelte') },
	{ pattern: '/login', load: () => import('./pages/Login.svelte') },
	{ pattern: '/notifications', load: () => import('./pages/Notifications.svelte') },
	{ pattern: '/organization', load: () => import('./pages/Organization.svelte') },
	{ pattern: '/people', load: () => import('./pages/People.svelte') },
	{ pattern: '/setup', load: () => import('./pages/Setup.svelte') },
	{ pattern: '/system', load: () => import('./pages/System.svelte') },
	{ pattern: '/view/[id]', load: () => import('./pages/View.svelte') },
	{ pattern: '/works', load: () => import('./pages/Works.svelte') },
	{ pattern: '/works/import', load: () => import('./pages/WorksImport.svelte') },
	{ pattern: '/works/[id]', load: () => import('./pages/Work.svelte') }
];
