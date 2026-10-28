// SPDX-License-Identifier: AGPL-3.0-or-later
import type { Recording, RecordingInput } from './api/index.js';

/** What the file picker offers: audio only. */
export const AUDIO_ACCEPT =
	'audio/*,.mp3,.m4a,.m4b,.aac,.wav,.flac,.ogg,.oga,.opus,.weba,.webm,.aif,.aiff';

/** For a part: its own practice recordings first, then full recordings, then the other parts'. */
export function forPart(recordings: Recording[], partId: string | null): Recording[] {
	const rank = (r: Recording) => (r.partId && r.partId === partId ? 0 : r.kind === 'full' ? 1 : 2);
	return [...recordings].sort((a, b) => rank(a) - rank(b) || a.title.localeCompare(b.title));
}

/** "Sopran langsam.mp3" → "Sopran langsam". */
export function titleFromFile(name: string): string {
	return name
		.replace(/\.[a-z0-9]{2,5}$/i, '')
		.replace(/[_]+/g, ' ')
		.trim()
		.slice(0, 120);
}

/** The length of an audio file, read by the browser; null when it cannot tell. */
export function measure(file: File): Promise<number | null> {
	return new Promise((resolve) => {
		const audio = document.createElement('audio');
		const url = URL.createObjectURL(file);
		const done = (value: number | null) => {
			URL.revokeObjectURL(url);
			resolve(value);
		};
		audio.preload = 'metadata';
		audio.onloadedmetadata = () => done(Number.isFinite(audio.duration) ? audio.duration : null);
		audio.onerror = () => done(null);
		audio.src = url;
	});
}

/** Uploads a recording with progress. */
export function uploadRecording(
	workId: string,
	file: File,
	meta: RecordingInput,
	onProgress: (fraction: number) => void
): Promise<Recording> {
	return new Promise((resolve, reject) => {
		const request = new XMLHttpRequest();
		request.open('POST', `/api/works/${workId}/recordings`);
		request.responseType = 'json';
		request.upload.onprogress = (e) => e.lengthComputable && onProgress(e.loaded / e.total);
		request.onload = () =>
			request.status === 201
				? resolve(request.response as Recording)
				: reject(new Error(request.response?.message ?? `HTTP ${request.status}`));
		request.onerror = () => reject(new Error('network'));
		const form = new FormData();
		form.append('meta', JSON.stringify(meta));
		form.append('file', file);
		request.send(form);
	});
}
