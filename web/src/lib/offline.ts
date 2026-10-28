// SPDX-License-Identifier: AGPL-3.0-or-later
// Parts kept on the device for playing without a network, in IndexedDB.
//
// Rules: only parts the user may see are stored; the next sync while online removes parts whose access was
// withdrawn and fetches new versions; signing out or another user signing in on the device wipes everything.
import type { AuthStatus, MyEnsemble } from './api/index.js';

export interface StoredPart {
	id: string;
	label: string;
	pageCount: number;
	updatedAt: string;
	workId: string;
	workTitle: string;
	composer: string | null;
	/** Offline ensembles the part is stored for. */
	ensembleIds: string[];
	pdf: Blob;
	savedAt: string;
}

export type PartMeta = Omit<StoredPart, 'pdf'>;

const DB = 'notenwart';
// 2: annotations (kept parts and ensembles stay).
const VERSION = 2;

function open(): Promise<IDBDatabase> {
	return new Promise((resolve, reject) => {
		const request = indexedDB.open(DB, VERSION);
		request.onupgradeneeded = () => {
			const db = request.result;
			const stores = db.objectStoreNames;
			if (!stores.contains('parts')) db.createObjectStore('parts', { keyPath: 'id' });
			if (!stores.contains('ensembles')) db.createObjectStore('ensembles', { keyPath: 'id' });
			if (!stores.contains('meta')) db.createObjectStore('meta');
			if (!stores.contains('annotations'))
				db.createObjectStore('annotations', { keyPath: 'partId' });
		};
		request.onsuccess = () => resolve(request.result);
		request.onerror = () => reject(request.error);
	});
}

async function tx<T>(
	store: string,
	mode: IDBTransactionMode,
	run: (store: IDBObjectStore) => IDBRequest<T> | void
): Promise<T | undefined> {
	const db = await open();
	return new Promise((resolve, reject) => {
		const transaction = db.transaction(store, mode);
		const request = run(transaction.objectStore(store));
		transaction.oncomplete = () => {
			db.close();
			resolve(request ? request.result : undefined);
		};
		transaction.onerror = () => {
			db.close();
			reject(transaction.error);
		};
	});
}

export async function getPart(id: string): Promise<StoredPart | undefined> {
	return tx<StoredPart>('parts', 'readonly', (s) => s.get(id));
}

export async function listParts(): Promise<PartMeta[]> {
	const all = (await tx<StoredPart[]>('parts', 'readonly', (s) => s.getAll())) ?? [];
	return all.map(({ pdf, ...meta }) => {
		void pdf;
		return meta;
	});
}

async function putPart(part: StoredPart) {
	await tx('parts', 'readwrite', (s) => s.put(part));
}

async function deletePart(id: string) {
	await tx('parts', 'readwrite', (s) => s.delete(id));
}

export async function offlineEnsembles(): Promise<string[]> {
	return ((await tx<{ id: string }[]>('ensembles', 'readonly', (s) => s.getAll())) ?? []).map(
		(e) => e.id
	);
}

export async function setEnsembleOffline(id: string, on: boolean) {
	await tx('ensembles', 'readwrite', (s) => {
		if (on) s.put({ id });
		else s.delete(id);
	});
	// Ask the browser not to evict the music under storage pressure.
	if (on) await navigator.storage?.persist?.().catch(() => false);
}

export async function getMeta<T>(key: string): Promise<T | undefined> {
	return tx<T>('meta', 'readonly', (s) => s.get(key));
}

export async function setMeta(key: string, value: unknown) {
	// A plain copy: reactive state (Svelte proxies) cannot be stored in IndexedDB.
	const plain = JSON.parse(JSON.stringify(value));
	await tx('meta', 'readwrite', (s) => s.put(plain, key));
}

/** Removes everything: the next user of the device must not find this one's music. */
export async function clearAll() {
	await new Promise<void>((resolve) => {
		const request = indexedDB.deleteDatabase(DB);
		request.onsuccess = request.onerror = request.onblocked = () => resolve();
	});
}

/** Remembers who is signed in; a different user wipes the device's copies first. */
export async function rememberStatus(status: AuthStatus) {
	const user = status.user;
	if (!user) return;
	const previous = await getMeta<AuthStatus>('status');
	if (previous?.user && previous.user.id !== user.id) await clearAll();
	await setMeta('status', status);
}

export interface Wanted {
	id: string;
	updatedAt: string;
	ensembleIds: string[];
}

/** What a sync has to do, given what is stored and what should be. */
export function plan(
	stored: Pick<PartMeta, 'id' | 'updatedAt' | 'ensembleIds'>[],
	wanted: Wanted[]
) {
	const want = new Map(wanted.map((w) => [w.id, w]));
	const have = new Map(stored.map((s) => [s.id, s]));
	return {
		remove: stored.filter((s) => !want.has(s.id)).map((s) => s.id),
		download: wanted.filter((w) => have.get(w.id)?.updatedAt !== w.updatedAt).map((w) => w.id),
		relabel: wanted
			.filter((w) => {
				const s = have.get(w.id);
				return s && s.updatedAt === w.updatedAt && s.ensembleIds.join() !== w.ensembleIds.join();
			})
			.map((w) => w.id)
	};
}

/** The parts of the offline ensembles in `music`, each once, with every offline ensemble it belongs to. */
export function wantedParts(
	music: MyEnsemble[],
	ensembles: string[]
): Map<string, Wanted & PartInfoLite> {
	const result = new Map<string, Wanted & PartInfoLite>();
	for (const ensemble of music) {
		if (!ensembles.includes(ensemble.id)) continue;
		for (const work of ensemble.works) {
			for (const part of work.parts) {
				const entry = result.get(part.id);
				if (entry) entry.ensembleIds.push(ensemble.id);
				else
					result.set(part.id, {
						id: part.id,
						label: part.label,
						pageCount: part.pageCount,
						updatedAt: part.updatedAt,
						workId: work.id,
						workTitle: work.title,
						composer: work.composer ?? null,
						ensembleIds: [ensemble.id]
					});
			}
		}
	}
	return result;
}

type PartInfoLite = Omit<PartMeta, 'ensembleIds' | 'savedAt'>;

export interface SyncProgress {
	done: number;
	total: number;
}

/**
 * Brings the stored parts in line with `music` (fetched just now, while online). Ensembles that are no longer in
 * `music` stop being offline.
 */
export async function sync(music: MyEnsemble[], onProgress?: (progress: SyncProgress) => void) {
	await setMeta('music', music);
	const known = new Set(music.map((e) => e.id));
	const enabled = await offlineEnsembles();
	for (const id of enabled.filter((id) => !known.has(id))) await setEnsembleOffline(id, false);
	const wanted = wantedParts(
		music,
		enabled.filter((id) => known.has(id))
	);
	const stored = await listParts();
	const todo = plan(stored, [...wanted.values()]);
	for (const id of todo.remove) await deletePart(id);
	for (const id of todo.relabel) {
		const part = await getPart(id);
		if (part) await putPart({ ...part, ensembleIds: wanted.get(id)!.ensembleIds });
	}
	let done = 0;
	onProgress?.({ done, total: todo.download.length });
	for (const id of todo.download) {
		const response = await fetch(`/api/parts/${id}/file`);
		if (!response.ok) continue;
		const pdf = await response.blob();
		await putPart({ ...wanted.get(id)!, pdf, savedAt: new Date().toISOString() });
		onProgress?.({ done: ++done, total: todo.download.length });
	}
	return todo;
}

/** A person's annotations on a part as kept on this device; `dirty` until the server has them. */
export interface StoredAnnotations {
	partId: string;
	pages: Record<string, unknown[]>;
	/** The server's time of the version this one is based on, or null. */
	updatedAt: string | null;
	dirty: boolean;
}

export async function getAnnotations(partId: string): Promise<StoredAnnotations | undefined> {
	return tx<StoredAnnotations>('annotations', 'readonly', (s) => s.get(partId));
}

export async function putAnnotations(record: StoredAnnotations) {
	// A plain copy: reactive state cannot be stored in IndexedDB.
	const plain = JSON.parse(JSON.stringify(record));
	await tx('annotations', 'readwrite', (s) => s.put(plain));
}
