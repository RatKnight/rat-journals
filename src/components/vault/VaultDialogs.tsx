import { createEffect, createSignal, For, Show } from 'solid-js';
import { Dialog } from '@kobalte/core/dialog';
import { useI18n } from '../../i18n';

const inputClass =
  'w-full rounded-md border border-primary bg-primary px-3 py-2 text-sm text-primary focus:outline-none focus:ring-2 focus:ring-blue-500';

interface ShellProps {
  open: boolean;
  title: string;
  onClose: () => void;
  children: unknown;
  testId?: string;
}

function Shell(props: ShellProps) {
  return (
    <Dialog open={props.open} onOpenChange={(o) => !o && props.onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay
          class="fixed inset-0 z-[60]"
          style={{ 'background-color': 'var(--overlay-bg)' }}
        />
        <div class="fixed inset-0 z-[60] flex items-center justify-center p-4">
          <Dialog.Content
            class="w-full max-w-sm rounded-lg bg-primary p-5"
            style={{ 'box-shadow': 'var(--shadow-lg)' }}
            data-testid={props.testId}
          >
            <Dialog.Title class="mb-3 text-base font-semibold text-primary">
              {props.title}
            </Dialog.Title>
            {props.children as never}
          </Dialog.Content>
        </div>
      </Dialog.Portal>
    </Dialog>
  );
}

interface TextPromptProps {
  open: boolean;
  title: string;
  label: string;
  initial: string;
  confirmLabel: string;
  error?: string | null;
  onSubmit: (value: string) => void;
  onClose: () => void;
}

/** A one-field dialog: rename a file, name a new folder, rename a folder. */
export function VaultTextPrompt(props: TextPromptProps) {
  const t = useI18n();
  const [value, setValue] = createSignal('');
  createEffect(() => {
    if (props.open) setValue(props.initial);
  });
  return (
    <Shell open={props.open} title={props.title} onClose={props.onClose} testId="vault-text-prompt">
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (value().trim()) props.onSubmit(value().trim());
        }}
        class="space-y-3"
      >
        <input
          class={inputClass}
          value={value()}
          onInput={(e) => setValue(e.currentTarget.value)}
          aria-label={props.label}
          autofocus
        />
        <Show when={props.error}>
          <p role="alert" class="text-sm text-error">
            {props.error}
          </p>
        </Show>
        <div class="flex justify-end gap-2">
          <button
            type="button"
            class="rounded-md px-3 py-1.5 text-sm text-secondary hover:bg-hover"
            onClick={() => props.onClose()}
          >
            {t('common.cancel')}
          </button>
          <button
            type="submit"
            class="rounded-md interactive-primary px-3 py-1.5 text-sm font-medium"
            disabled={!value().trim()}
          >
            {props.confirmLabel}
          </button>
        </div>
      </form>
    </Shell>
  );
}

interface MoveDialogProps {
  open: boolean;
  folders: string[];
  current: string | null;
  error?: string | null;
  onSubmit: (folder: string | null) => void;
  onClose: () => void;
}

/** Picks a destination folder ("No folder" moves a file out of all folders). */
export function VaultMoveDialog(props: MoveDialogProps) {
  const t = useI18n();
  const [choice, setChoice] = createSignal('');
  createEffect(() => {
    if (props.open) setChoice(props.current ?? '');
  });
  return (
    <Shell
      open={props.open}
      title={t('vault.moveTitle')}
      onClose={props.onClose}
      testId="vault-move-dialog"
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          props.onSubmit(choice() || null);
        }}
        class="space-y-3"
      >
        <select
          class={inputClass}
          value={choice()}
          onChange={(e) => setChoice(e.currentTarget.value)}
          aria-label={t('vault.moveTitle')}
        >
          <option value="">{t('vault.noFolder')}</option>
          <For each={props.folders}>{(f) => <option value={f}>{f}</option>}</For>
        </select>
        <Show when={props.error}>
          <p role="alert" class="text-sm text-error">
            {props.error}
          </p>
        </Show>
        <div class="flex justify-end gap-2">
          <button
            type="button"
            class="rounded-md px-3 py-1.5 text-sm text-secondary hover:bg-hover"
            onClick={() => props.onClose()}
          >
            {t('common.cancel')}
          </button>
          <button
            type="submit"
            class="rounded-md interactive-primary px-3 py-1.5 text-sm font-medium"
          >
            {t('vault.moveButton')}
          </button>
        </div>
      </form>
    </Shell>
  );
}
