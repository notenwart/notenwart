// SPDX-License-Identifier: AGPL-3.0-or-later
// The signed-in user, shared by all pages.
import { api, type AuthStatus, type Me, type Role } from './api/index.js';
import { clearAll, getMeta, rememberStatus } from './offline.js';

export const session = $state<{ status: AuthStatus | null; me: Me | null; offline: boolean }>({
	status: null,
	me: null,
	offline: false
});

export async function loadSession() {
	try {
		const { data } = await api.GET('/auth/status');
		session.status = data ?? null;
		session.me = data?.user ?? null;
		session.offline = false;
		if (data?.user) await rememberStatus(data);
		// Signed out on the server (expired, revoked, deleted): nothing of the last user stays on the device.
		else await clearAll();
	} catch {
		// No network: carry on as the user last seen on this device, with what is stored offline.
		const cached = await getMeta<AuthStatus>('status').catch(() => undefined);
		session.status = cached ?? {
			setupRequired: false,
			mailEnabled: false,
			maxUploadBytes: 0,
			recognition: { text: false, ocr: false, ai: false },
			user: null
		};
		session.me = cached?.user ?? null;
		session.offline = true;
	}
}

export function signedIn(me: Me) {
	session.me = me;
	if (session.status) session.status = { ...session.status, setupRequired: false, user: me };
	if (session.status) rememberStatus(session.status);
}

export async function signOut() {
	await api.POST('/auth/logout').catch(() => {});
	// Music stored for offline use belongs to this user; the next one on the device must not find it.
	await clearAll();
	session.me = null;
	location.assign('/login');
}

export const roleNames: Record<Role, string> = {
	admin: 'Admin',
	librarian: 'Librarian',
	leader: 'Leader',
	member: 'Member'
};

/** Everyone but plain members may browse the library. */
export function readsLibrary(me: Me | null) {
	return !!me && me.role !== 'member';
}

/** Admins and librarians change the library and manage all ensembles. */
export function isStaff(me: Me | null) {
	return me?.role === 'admin' || me?.role === 'librarian';
}
