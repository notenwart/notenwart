// SPDX-License-Identifier: AGPL-3.0-or-later
import type { Work, WorkInput } from './api/index.js';

/**
 * The body for saving a work with some fields changed. `PUT /works/{id}` replaces every field, so every save starts
 * from the whole work – a field added later is never lost by a form that does not know it.
 */
export function workInput(work: Work, changes: Partial<WorkInput> = {}): WorkInput {
	return {
		title: work.title,
		composer: work.composer,
		arranger: work.arranger,
		publisher: work.publisher,
		lyricist: work.lyricist,
		societyNumber: work.societyNumber,
		archiveLocation: work.archiveLocation,
		instrumentation: work.instrumentation,
		difficulty: work.difficulty,
		durationSeconds: work.durationSeconds,
		source: work.source,
		notes: work.notes,
		tags: work.tags,
		...changes
	};
}
