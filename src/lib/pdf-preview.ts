// In-app PDF preview with pdf.js, loaded only when a PDF is opened.
//
// This app nulls `Worker` and sets `worker-src 'none'` (see network-isolation-script.ts and
// the CSP in tauri.conf.json), so pdf.js cannot start its usual Web Worker. Importing the
// worker module and exposing it as `globalThis.pdfjsWorker` makes pdf.js run its parser on
// the main thread instead. pdf.js 6 uses no `eval`, so it stays within `script-src 'self'`,
// and nothing is fetched: the document comes from memory, and fonts that are not embedded
// fall back to system fonts.

type PdfJs = typeof import('pdfjs-dist');

let loading: Promise<PdfJs> | null = null;

async function loadPdfJs(): Promise<PdfJs> {
  loading ??= (async () => {
    const worker = await import('pdfjs-dist/build/pdf.worker.mjs');
    (globalThis as { pdfjsWorker?: unknown }).pdfjsWorker = worker;
    return await import('pdfjs-dist');
  })();
  return loading;
}

/** Renders every page of `bytes` as a canvas appended to `container`. Returns page count. */
export async function renderPdfInto(
  bytes: ArrayBuffer,
  container: HTMLElement,
  options: { width: number; isCancelled: () => boolean },
): Promise<number> {
  const pdfjs = await loadPdfJs();
  const task = pdfjs.getDocument({
    data: new Uint8Array(bytes),
    useSystemFonts: true,
    disableAutoFetch: true,
    disableStream: true,
    enableXfa: false,
  });
  try {
    const doc = await task.promise;
    const scale = window.devicePixelRatio || 1;
    for (let n = 1; n <= doc.numPages; n++) {
      if (options.isCancelled()) break;
      const page = await doc.getPage(n);
      const base = page.getViewport({ scale: 1 });
      const viewport = page.getViewport({ scale: (options.width / base.width) * scale });
      const canvas = document.createElement('canvas');
      canvas.width = Math.floor(viewport.width);
      canvas.height = Math.floor(viewport.height);
      canvas.style.width = `${Math.floor(viewport.width / scale)}px`;
      canvas.className = 'vault-pdf-page';
      container.appendChild(canvas);
      await page.render({ canvas, viewport }).promise;
      page.cleanup();
    }
    return doc.numPages;
  } finally {
    await task.destroy();
  }
}
