/**
 * E2E test: encrypted file vault
 *
 * Creates a vault and stores a PDF through the backend commands (the folder picker is a
 * native dialog WebDriver cannot drive), reloads so the frontend picks up the open vault,
 * then checks the vault window lists the file, previews the PDF in-app (pdf.js running on
 * the main thread under the app's strict CSP), and locks.
 */
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { connectToApp } from './helpers';

/** A minimal one-page PDF with correct xref offsets. */
function tinyPdf(): number[] {
  const objects = [
    '<< /Type /Catalog /Pages 2 0 R >>',
    '<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>',
    null, // content stream, filled below
    '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
  ];
  const stream = 'BT /F1 24 Tf 40 100 Td (Vault test) Tj ET';
  objects[3] = `<< /Length ${stream.length} >>\nstream\n${stream}\nendstream`;
  let out = '%PDF-1.4\n';
  const offsets: number[] = [];
  objects.forEach((body, i) => {
    offsets.push(out.length);
    out += `${i + 1} 0 obj\n${body}\nendobj\n`;
  });
  const xref = out.length;
  out += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`;
  offsets.forEach((o) => (out += `${String(o).padStart(10, '0')} 00000 n \n`));
  out += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  return Array.from(Buffer.from(out, 'latin1'));
}

describe('Encrypted file vault', () => {
  it('lists, previews a PDF, and locks', async () => {
    await connectToApp();
    const vaultDir = join(mkdtempSync(join(tmpdir(), 'rat-journal-e2e-vault-')), 'Vault');

    const error = await browser.execute(
      async (path: string, pdf: number[]) => {
        const invoke = (
          window as unknown as {
            __TAURI_INTERNALS__: {
              invoke: (c: string, a?: unknown, o?: unknown) => Promise<unknown>;
            };
          }
        ).__TAURI_INTERNALS__.invoke;
        try {
          await invoke('vault_create', { path, password: 'pw', keyFilePath: null });
          await invoke('vault_add_bytes', new Uint8Array(pdf), {
            headers: { 'x-vault-name': btoa('test.pdf') },
          });
          return null;
        } catch (e) {
          return String(e);
        }
      },
      vaultDir,
      tinyPdf(),
    );
    expect(error).toBeNull();

    await browser.refresh();
    const window = await $('[data-testid="vault-window"]');
    await window.waitForDisplayed({ timeout: 15000 });

    const row = await $('[data-testid="vault-file-row"]');
    await row.waitForDisplayed({ timeout: 10000 });
    expect(await row.getText()).toContain('test.pdf');

    await row.doubleClick();
    const page = await $('canvas.vault-pdf-page');
    await page.waitForExist({ timeout: 20000 });
    expect(await page.getAttribute('width')).not.toBe('0');

    await browser.keys('Escape');
    await (await $('[data-testid="vault-preview"]')).waitForExist({ reverse: true, timeout: 5000 });

    await (await $('[data-testid="vault-lock"]')).click();
    await window.waitForExist({ reverse: true, timeout: 5000 });
  });
});
