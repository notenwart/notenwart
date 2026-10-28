// SPDX-License-Identifier: AGPL-3.0-or-later
// Light, dark or the device's setting – chosen per device, applied as data-theme on <html> (app.css).

export type Theme = 'system' | 'light' | 'dark';
const KEY = 'notenwart.theme';

function stored(): Theme {
	try {
		const value = localStorage.getItem(KEY);
		if (value === 'light' || value === 'dark') return value;
	} catch {
		// Private mode: the device's setting.
	}
	return 'system';
}

export const theme = $state<{ value: Theme }>({
	value: typeof window === 'undefined' ? 'system' : stored()
});

export function setTheme(value: Theme) {
	theme.value = value;
	try {
		if (value === 'system') localStorage.removeItem(KEY);
		else localStorage.setItem(KEY, value);
	} catch {
		// Not remembered, still applied.
	}
	apply(value);
}

export function apply(value: Theme) {
	const root = document.documentElement;
	if (value === 'system') delete root.dataset.theme;
	else root.dataset.theme = value;
}

/** The next theme when cycling through them with one button. */
export function nextTheme(value: Theme): Theme {
	return value === 'system' ? 'light' : value === 'light' ? 'dark' : 'system';
}
