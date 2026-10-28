// SPDX-License-Identifier: AGPL-3.0-or-later
// pdf.js, loaded on demand so the start page stays small.
import type { PDFDocumentProxy } from 'pdfjs-dist';

let library: Promise<typeof import('pdfjs-dist')> | undefined;

function pdfjs() {
	library ??= Promise.all([
		import('pdfjs-dist'),
		import('pdfjs-dist/build/pdf.worker.min.mjs?url')
	]).then(([pdfjs, worker]) => {
		pdfjs.GlobalWorkerOptions.workerSrc = worker.default;
		return pdfjs;
	});
	return library;
}

/** Opens a PDF from a URL or from bytes (e.g. an offline copy). */
export async function openPdf(source: string | ArrayBuffer): Promise<PDFDocumentProxy> {
	const { getDocument } = await pdfjs();
	return getDocument(typeof source === 'string' ? { url: source } : { data: source }).promise;
}

/**
 * Renders a page as large as fits into a box of `width` × `height` CSS pixels, at the screen's resolution.
 * Returns a bitmap that can be drawn again instantly, and its size in CSS pixels.
 */
export async function renderFitted(
	document: PDFDocumentProxy,
	number: number,
	width: number,
	height: number
): Promise<{ bitmap: ImageBitmap; cssWidth: number; cssHeight: number }> {
	const page = await document.getPage(number);
	const unscaled = page.getViewport({ scale: 1 });
	const fit = Math.min(width / unscaled.width, height / unscaled.height);
	const ratio = window.devicePixelRatio || 1;
	const viewport = page.getViewport({ scale: fit * ratio });
	const canvas = window.document.createElement('canvas');
	canvas.width = Math.floor(viewport.width);
	canvas.height = Math.floor(viewport.height);
	await page.render({ canvas, viewport }).promise;
	const bitmap = await createImageBitmap(canvas);
	return { bitmap, cssWidth: canvas.width / ratio, cssHeight: canvas.height / ratio };
}

/** Renders a page into the canvas, `width` CSS pixels wide. */
export async function renderPage(
	document: PDFDocumentProxy,
	number: number,
	canvas: HTMLCanvasElement,
	width: number
) {
	const page = await document.getPage(number);
	const unscaled = page.getViewport({ scale: 1 });
	const ratio = window.devicePixelRatio || 1;
	const viewport = page.getViewport({ scale: (width / unscaled.width) * ratio });
	canvas.width = Math.floor(viewport.width);
	canvas.height = Math.floor(viewport.height);
	canvas.style.width = `${width}px`;
	await page.render({ canvas, viewport }).promise;
}
