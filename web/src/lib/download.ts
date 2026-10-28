// SPDX-License-Identifier: AGPL-3.0-or-later

/** Lets the browser save `content` as a file. */
export function download(name: string, content: string, type = 'text/csv;charset=utf-8') {
	const url = URL.createObjectURL(new Blob([content], { type }));
	const link = document.createElement('a');
	link.href = url;
	link.download = name;
	link.click();
	URL.revokeObjectURL(url);
}
