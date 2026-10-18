import { createSignal, Match, onCleanup, onMount, Show, Switch } from 'solid-js';
import { Dialog } from '@kobalte/core/dialog';
import { X } from 'lucide-solid';
import * as tauri from '../../lib/tauri';
import type { VaultFile } from '../../lib/tauri';
import { mapTauriError } from '../../lib/errors';
import { vaultPreviewKind } from '../../lib/vault-files';
import { useI18n } from '../../i18n';

const MAX_TEXT_CHARS = 2_000_000;

interface Props {
  file: VaultFile;
  onClose: () => void;
  onOpenInApp: (file: VaultFile) => void;
}

/**
 * Shows an image, PDF, or text file inside the app. The decrypted bytes stay in memory
 * only and are dropped (and best-effort zeroed) when the preview closes.
 */
export default function VaultPreview(props: Props) {
  const t = useI18n();
  const kind = vaultPreviewKind(props.file);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);
  const [imageUrl, setImageUrl] = createSignal<string | null>(null);
  const [text, setText] = createSignal('');
  let pdfContainer: HTMLDivElement | undefined;
  let cancelled = false;
  let bytes: ArrayBuffer | null = null;

  const tooLarge = props.file.size > tauri.VAULT_MAX_PREVIEW_BYTES;

  onMount(async () => {
    if (tooLarge || !kind) {
      setLoading(false);
      return;
    }
    try {
      bytes = await tauri.vaultPreview(props.file.id);
      if (cancelled) return;
      if (kind === 'image') {
        setImageUrl(URL.createObjectURL(new Blob([bytes], { type: props.file.mime ?? '' })));
      } else if (kind === 'text') {
        setText(new TextDecoder().decode(bytes).slice(0, MAX_TEXT_CHARS));
      } else if (kind === 'pdf' && pdfContainer) {
        const { renderPdfInto } = await import('../../lib/pdf-preview');
        await renderPdfInto(bytes, pdfContainer, {
          width: Math.min(pdfContainer.clientWidth - 32, 1000),
          isCancelled: () => cancelled,
        });
      }
    } catch (err) {
      if (!cancelled) setError(mapTauriError(err, t));
    } finally {
      if (!cancelled) setLoading(false);
    }
  });

  onCleanup(() => {
    cancelled = true;
    const url = imageUrl();
    if (url) URL.revokeObjectURL(url);
    // Best-effort wipe. pdf.js takes ownership of the buffer it parses, which detaches
    // ours (byteLength 0); touching a detached buffer throws, and an exception in a cleanup
    // would abort the rest of the component teardown.
    if (bytes && bytes.byteLength > 0) {
      try {
        new Uint8Array(bytes).fill(0);
      } catch {
        // Already detached.
      }
    }
    bytes = null;
    setText('');
    pdfContainer?.replaceChildren();
  });

  return (
    <Dialog open onOpenChange={(o) => !o && props.onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay
          class="fixed inset-0 z-[60]"
          style={{ 'background-color': 'var(--overlay-bg)' }}
        />
        <div class="fixed inset-0 z-[60] flex items-center justify-center p-4">
          <Dialog.Content
            class="flex h-full max-h-[90vh] w-full max-w-5xl flex-col rounded-lg bg-primary"
            style={{ 'box-shadow': 'var(--shadow-lg)' }}
            data-testid="vault-preview"
          >
            <div class="flex items-center gap-3 border-b border-primary px-4 py-3">
              <Dialog.Title class="min-w-0 flex-1 truncate text-sm font-semibold text-primary">
                {props.file.name}
              </Dialog.Title>
              <button
                type="button"
                class="rounded-md border border-primary px-3 py-1 text-xs text-secondary hover:bg-hover"
                onClick={() => props.onOpenInApp(props.file)}
              >
                {t('vault.openInApp')}
              </button>
              <Dialog.CloseButton
                class="rounded p-1 text-tertiary hover:bg-hover"
                aria-label={t('common.close')}
              >
                <X size={18} />
              </Dialog.CloseButton>
            </div>
            <div class="min-h-0 flex-1 overflow-auto bg-secondary p-4">
              <Show when={loading()}>
                <p class="text-sm text-secondary">{t('vault.decrypting')}</p>
              </Show>
              <Show when={error()}>
                <p role="alert" class="text-sm text-error">
                  {error()}
                </p>
              </Show>
              <Show when={!loading() && (tooLarge || !kind)}>
                <p class="text-sm text-secondary">
                  {tooLarge ? t('vault.previewTooLarge') : t('vault.previewUnsupported')}
                </p>
              </Show>
              <Switch>
                <Match when={kind === 'image' && imageUrl()}>
                  <img
                    src={imageUrl()!}
                    alt={props.file.name}
                    class="mx-auto max-h-full max-w-full object-contain"
                  />
                </Match>
                <Match when={kind === 'text' && !loading()}>
                  <pre class="whitespace-pre-wrap break-words text-sm text-primary">{text()}</pre>
                </Match>
              </Switch>
              <div ref={(el) => (pdfContainer = el)} class="flex flex-col items-center gap-4" />
            </div>
          </Dialog.Content>
        </div>
      </Dialog.Portal>
    </Dialog>
  );
}
