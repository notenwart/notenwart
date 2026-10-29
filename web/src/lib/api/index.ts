// SPDX-License-Identifier: AGPL-3.0-or-later
// Typed client for the API in ../api/openapi.json. Regenerate the types with `pnpm api:types`.
import createClient from 'openapi-fetch';
import type { components, paths } from './schema';
import { t } from '../i18n/index.svelte.js';

export const api = createClient<paths>({ baseUrl: '/api' });

// An expired or revoked session sends the user to the sign-in page (but not the sign-in requests themselves),
// and the music stored on the device for offline use goes with it.
api.use({
	async onResponse({ request, response }) {
		const path = new URL(request.url).pathname;
		if (
			response.status === 401 &&
			!path.startsWith('/api/auth/') &&
			location.pathname !== '/login'
		) {
			const { clearAll } = await import('../offline.js');
			await clearAll();
			location.assign(`/login?next=${encodeURIComponent(location.pathname + location.search)}`);
		}
	}
});

export type Instrument = components['schemas']['Instrument'];
export type InstrumentInput = components['schemas']['InstrumentInput'];
export type InstrumentFamily = components['schemas']['InstrumentFamily'];
export type Work = components['schemas']['Work'];
export type WorkInput = components['schemas']['WorkInput'];
export type WorkSummary = components['schemas']['WorkSummary'];
export type WorkFacets = components['schemas']['WorkFacets'];
export type Recognition = components['schemas']['Recognition'];
export type WorkDetail = components['schemas']['WorkDetail'];
export type SourceFile = components['schemas']['SourceFile'];
export type Part = components['schemas']['Part'];
export type PartInput = components['schemas']['PartInput'];
export type ApiError = components['schemas']['Error'];
export type Me = components['schemas']['Me'];
export type Role = components['schemas']['Role'];
export type AuthStatus = components['schemas']['AuthStatus'];
export type User = components['schemas']['User'];
export type Invitation = components['schemas']['Invitation'];
export type Ensemble = components['schemas']['Ensemble'];
export type EnsembleRef = components['schemas']['EnsembleRef'];
export type BulkInput = components['schemas']['BulkInput'];
export type BulkResult = components['schemas']['BulkResult'];
export type EnsembleDetail = components['schemas']['EnsembleDetail'];
export type EnsembleMember = components['schemas']['EnsembleMember'];
export type PlayedPart = components['schemas']['PlayedPart'];
export type MyEnsemble = components['schemas']['MyEnsemble'];
export type ImportResult = components['schemas']['ImportResult'];
export type Program = components['schemas']['Program'];
export type Organization = components['schemas']['Organization'];
export type System = components['schemas']['System'];
export type Loan = components['schemas']['Loan'];
export type Notification = components['schemas']['Notification'];
export type NotificationKind = components['schemas']['NotificationKind'];
export type NotificationSetting = components['schemas']['NotificationSetting'];
export type Deletion = components['schemas']['Deletion'];
export type LoanInput = components['schemas']['LoanInput'];
export type Borrower = components['schemas']['Borrower'];
export type ProgramItem = components['schemas']['ProgramItem'];
export type EnsemblePart = components['schemas']['EnsemblePart'];
export type GuestLink = components['schemas']['GuestLink'];
export type Recording = components['schemas']['Recording'];
export type RecordingInput = components['schemas']['RecordingInput'];
export type GuestView = components['schemas']['GuestView'];
export type Grant = components['schemas']['Grant'];
export type Stroke = components['schemas']['Stroke'];
export type Annotations = components['schemas']['Annotations'];

/** The message of an API error in the UI language (server messages are English), or a generic one. */
export function message(error: unknown): string {
	if (error && typeof error === 'object' && 'message' in error) return t(String(error.message));
	return t('Something went wrong.');
}

/** Uploads one PDF with progress (fetch cannot report upload progress). */
/** The work has this file already (same content). */
export class AlreadyUploaded extends Error {}

export function uploadSource(
	workId: string,
	file: File,
	onProgress: (fraction: number) => void
): Promise<SourceFile> {
	return new Promise((resolve, reject) => {
		const request = new XMLHttpRequest();
		request.open('POST', `/api/works/${workId}/sources`);
		request.responseType = 'json';
		request.upload.onprogress = (event) => {
			if (event.lengthComputable) onProgress(event.loaded / event.total);
		};
		request.onload = () => {
			if (request.status === 201) resolve(request.response as SourceFile);
			else if (request.status === 409) reject(new AlreadyUploaded(message(request.response)));
			else if (request.status === 413)
				reject(new Error(t('The file is larger than this server allows.')));
			else reject(new Error(message(request.response)));
		};
		request.onerror = () => reject(new Error(t('The server cannot be reached.')));
		const form = new FormData();
		form.append('file', file);
		request.send(form);
	});
}
