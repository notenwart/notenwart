// SPDX-License-Identifier: AGPL-3.0-or-later
// Durations of works and programmes: typed as "4:30" (minutes:seconds), "4" (minutes) or "1:02:30".

/** Seconds for "4:30", "4", "1:02:30"; null for nothing typed, NaN for something that is no duration. */
export function parseDuration(text: string): number | null {
	const trimmed = text.trim();
	if (!trimmed) return null;
	if (!/^\d+(:[0-5]?\d){0,2}$/.test(trimmed)) return NaN;
	const parts = trimmed.split(':').map(Number);
	if (parts.length === 1) return parts[0] * 60;
	if (parts.length === 2) return parts[0] * 60 + parts[1];
	return parts[0] * 3600 + parts[1] * 60 + parts[2];
}

/** "4:30", or "1:02:30" from an hour on. */
export function formatDuration(seconds: number | null | undefined): string {
	if (seconds == null) return '';
	const s = Math.round(seconds);
	const two = (n: number) => String(n).padStart(2, '0');
	if (s >= 3600)
		return `${Math.floor(s / 3600)}:${two(Math.floor((s % 3600) / 60))}:${two(s % 60)}`;
	return `${Math.floor(s / 60)}:${two(s % 60)}`;
}

/** The time of day `seconds` after a start like "19:30", e.g. "19:45" (past midnight wraps). */
export function clockAfter(start: string, seconds: number): string {
	const [hours, minutes] = start.split(':').map(Number);
	const total = Math.floor((hours * 3600 + minutes * 60 + seconds) / 60) % (24 * 60);
	return `${String(Math.floor(total / 60)).padStart(2, '0')}:${String(total % 60).padStart(2, '0')}`;
}
