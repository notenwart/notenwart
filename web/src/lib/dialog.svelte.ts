// SPDX-License-Identifier: AGPL-3.0-or-later
// App-wide confirmation dialog and short notices, rendered once in the layout (components/Dialogs.svelte).

export interface ConfirmRequest {
	title: string;
	message?: string;
	confirm: string;
	danger?: boolean;
	resolve: (ok: boolean) => void;
}

export interface Notice {
	id: number;
	text: string;
	kind: 'info' | 'error';
}

export const dialogs = $state<{ confirm: ConfirmRequest | null; notices: Notice[] }>({
	confirm: null,
	notices: []
});

/** Asks before something that cannot be undone; resolves true when confirmed. */
export function confirm(options: Omit<ConfirmRequest, 'resolve'>): Promise<boolean> {
	return new Promise((resolve) => {
		dialogs.confirm?.resolve(false);
		dialogs.confirm = { ...options, resolve };
	});
}

let next = 1;
/** A short message at the bottom of the screen that goes away by itself. */
export function notify(text: string, kind: Notice['kind'] = 'info') {
	const id = next++;
	dialogs.notices = [...dialogs.notices, { id, text, kind }];
	setTimeout(() => (dialogs.notices = dialogs.notices.filter((n) => n.id !== id)), 4000);
}
