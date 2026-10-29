// SPDX-License-Identifier: AGPL-3.0-or-later
// The chosen colour mode before the first paint, so nothing flashes (see src/lib/theme.svelte.ts). A classic
// script loaded by index.html, so it runs before the page is drawn.
try {
	var mode = localStorage.getItem('notenwart.theme');
	if (mode === 'light' || mode === 'dark') document.documentElement.dataset.theme = mode;
} catch {
	// Private mode: the device's setting.
}
