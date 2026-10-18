/// <reference types="vite/client" />

interface Navigator {
  readonly userAgentData?: {
    readonly platform?: string;
  };
}

// pdf.js ships no types for its worker entry; it is only imported for its side effect of
// exposing WorkerMessageHandler (see src/lib/pdf-preview.ts).
declare module 'pdfjs-dist/build/pdf.worker.mjs';
