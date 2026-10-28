// SPDX-License-Identifier: AGPL-3.0-or-later
// Avatars without a picture: initials on a colour that stays the same for a name; and pictures scaled down in the
// browser before they are uploaded.

/** Colours that carry white text in light and dark mode (the brand's navy and brass among them). */
const PALETTE = [
	'#1D2B45',
	'#8A5A12',
	'#2F6B4F',
	'#7A2E4A',
	'#3A5A8C',
	'#6B4E9B',
	'#A0522D',
	'#2E6E73'
];

/** "Musik- und Kunstschule Seestadt" → "MS", "Anna" → "A", "Kammerchor am Tor 3" → "KT". */
export function initials(name: string): string {
	const words = name
		.split(/[\s-]+/)
		.filter((w) => /^\p{L}/u.test(w))
		.filter((w, i, all) => i === 0 || i === all.length - 1 || w[0] === w[0].toUpperCase());
	const first = words[0]?.[0] ?? '?';
	const last = words.length > 1 ? words[words.length - 1][0] : '';
	return (first + last).toUpperCase();
}

/** A colour from the palette, always the same for the same name. */
export function colorFor(name: string): string {
	let hash = 0;
	for (const char of name) hash = (hash * 31 + char.codePointAt(0)!) >>> 0;
	return PALETTE[hash % PALETTE.length];
}

/** Symbols for ensembles, to pick from. */
export const SYMBOLS = [
	'🎶',
	'🎵',
	'🎼',
	'🎤',
	'🎺',
	'🎷',
	'🎻',
	'🥁',
	'🎹',
	'🎸',
	'🪈',
	'🪕',
	'🪗',
	'🔔',
	'⭐',
	'❤️',
	'🌿',
	'🦉',
	'🐦',
	'🚂'
];
export const COLORS = PALETTE;

/** Scales an image down to at most `max` pixels on its longer side and re-encodes it (WebP, else JPEG). */
export async function shrink(file: File, max: number): Promise<Blob> {
	const bitmap = await createImageBitmap(file);
	const scale = Math.min(1, max / Math.max(bitmap.width, bitmap.height));
	const canvas = document.createElement('canvas');
	canvas.width = Math.round(bitmap.width * scale);
	canvas.height = Math.round(bitmap.height * scale);
	canvas.getContext('2d')!.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
	bitmap.close();
	const encode = (type: string) =>
		new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, type, 0.88));
	const webp = await encode('image/webp');
	if (webp && webp.type === 'image/webp') return webp;
	return (await encode('image/jpeg'))!;
}

/** Uploads an image as the form field `file`. */
export async function putImage(url: string, image: Blob): Promise<Response> {
	const form = new FormData();
	form.append('file', image, 'image');
	return fetch(url, { method: 'PUT', body: form });
}
