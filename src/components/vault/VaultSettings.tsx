import { createSignal, For, Show } from 'solid-js';
import { Dialog } from '@kobalte/core/dialog';
import { KeyRound, ShieldCheck, Trash2, X } from 'lucide-solid';
import * as tauri from '../../lib/tauri';
import { open, save } from '../../lib/dialog';
import { mapTauriError } from '../../lib/errors';
import { useI18n } from '../../i18n';
import { refreshVaultStatus, vaultStatus } from '../../state/vault';

interface Props {
  open: boolean;
  onClose: () => void;
}

/** Unlock methods (password, key files) and an integrity check for the open vault. */
export default function VaultSettings(props: Props) {
  const t = useI18n();
  const [current, setCurrent] = createSignal('');
  const [newPassword, setNewPassword] = createSignal('');
  const [confirmPassword, setConfirmPassword] = createSignal('');
  const [keyLabel, setKeyLabel] = createSignal('');
  const [message, setMessage] = createSignal<string | null>(null);
  const [error, setError] = createSignal<string | null>(null);
  const [busy, setBusy] = createSignal(false);

  const slots = () => vaultStatus()?.slots ?? [];
  const hasPassword = () => slots().some((s) => s.slot_type === 'password');
  const currentOrNull = () => (hasPassword() ? current() : null);

  const reset = () => {
    setCurrent('');
    setNewPassword('');
    setConfirmPassword('');
    setKeyLabel('');
    setMessage(null);
    setError(null);
  };

  const run = async (work: () => Promise<void>, done: string) => {
    setError(null);
    setMessage(null);
    setBusy(true);
    try {
      await work();
      await refreshVaultStatus();
      setMessage(done);
      setNewPassword('');
      setConfirmPassword('');
    } catch (err) {
      setError(mapTauriError(err, t));
    } finally {
      setBusy(false);
    }
  };

  /**
   * Changing the password or removing a method rotates the vault's master key, which
   * re-registers every remaining key file — so each one has to be picked. Returns their
   * paths, or null when the user cancels a picker.
   */
  const pickKeyFiles = async (excludeSlotId?: number): Promise<string[] | null> => {
    const paths: string[] = [];
    for (const slot of slots()) {
      if (slot.slot_type !== 'keyfile' || slot.id === excludeSlotId) continue;
      const path = await open({
        title: t('vault.selectKeyFile', { label: slot.label }),
        multiple: false,
        filters: [{ name: 'Key Files', extensions: ['key'] }],
      });
      if (typeof path !== 'string') return null;
      paths.push(path);
    }
    return paths;
  };

  const changePassword = async (e: Event) => {
    e.preventDefault();
    if (!newPassword()) return setError(t('vault.passwordRequired'));
    if (newPassword() !== confirmPassword()) return setError(t('vault.passwordsDontMatch'));
    // Adding a first password revokes nothing, so no rotation and no key files needed.
    const keyFiles = hasPassword() ? await pickKeyFiles() : [];
    if (!keyFiles) return;
    void run(
      () => tauri.vaultChangePassword(currentOrNull(), newPassword(), keyFiles),
      hasPassword() ? t('vault.passwordChanged') : t('vault.passwordAdded'),
    );
  };

  const addKeyFile = async () => {
    const label = keyLabel().trim() || t('vault.keyFileDefaultLabel');
    const path = await save({
      title: t('vault.saveKeyFileTitle'),
      defaultPath: 'rat-journal-vault.key',
      filters: [{ name: 'Key Files', extensions: ['key'] }],
    });
    if (!path) return;
    void run(() => tauri.vaultAddKeyFile(path, label, currentOrNull()), t('vault.keyFileAdded'));
  };

  const remove = async (slotId: number) => {
    const keyFiles = await pickKeyFiles(slotId);
    if (!keyFiles) return;
    void run(
      () => tauri.vaultRemoveUnlockMethod(slotId, currentOrNull(), keyFiles),
      t('vault.methodRemoved'),
    );
  };

  const verify = async () => {
    setError(null);
    setMessage(t('vault.verifying'));
    setBusy(true);
    try {
      const damaged = await tauri.vaultVerify();
      setMessage(
        damaged.length
          ? t('vault.verifyDamaged', { names: damaged.join(', ') })
          : t('vault.verifyOk'),
      );
    } catch (err) {
      setMessage(null);
      setError(mapTauriError(err, t));
    } finally {
      setBusy(false);
    }
  };

  const inputClass =
    'w-full rounded-md border border-primary bg-primary px-3 py-2 text-sm text-primary focus:outline-none focus:ring-2 focus:ring-blue-500';

  return (
    <Dialog
      open={props.open}
      onOpenChange={(o) => {
        if (!o) {
          reset();
          props.onClose();
        }
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay
          class="fixed inset-0 z-[60]"
          style={{ 'background-color': 'var(--overlay-bg)' }}
        />
        <div class="fixed inset-0 z-[60] flex items-center justify-center p-4">
          <Dialog.Content
            class="max-h-[90vh] w-full max-w-md overflow-y-auto rounded-lg bg-primary p-6"
            style={{ 'box-shadow': 'var(--shadow-lg)' }}
            data-testid="vault-settings"
          >
            <div class="mb-4 flex items-center">
              <Dialog.Title class="flex-1 text-lg font-semibold text-primary">
                {t('vault.settings')}
              </Dialog.Title>
              <Dialog.CloseButton
                class="rounded p-1 text-tertiary hover:bg-hover"
                aria-label={t('common.close')}
              >
                <X size={18} />
              </Dialog.CloseButton>
            </div>

            <h3 class="mb-2 text-xs font-medium text-secondary">{t('vault.unlockMethods')}</h3>
            <p class="mb-2 text-xs text-tertiary">{t('vault.rotateHint')}</p>
            <ul class="mb-4 rounded-md border border-primary">
              <For each={slots()}>
                {(slot) => (
                  <li class="flex items-center gap-2 border-b border-primary px-3 py-2 text-sm text-primary last:border-b-0">
                    <KeyRound size={14} class="text-tertiary" />
                    <span class="flex-1">
                      {slot.slot_type === 'password' ? t('vault.methodPassword') : slot.label}
                    </span>
                    <Show when={slots().length > 1}>
                      <button
                        type="button"
                        class="rounded p-1 text-tertiary hover:bg-hover disabled:opacity-50"
                        aria-label={t('vault.removeMethod')}
                        title={t('vault.removeMethod')}
                        disabled={busy()}
                        onClick={() => void remove(slot.id)}
                      >
                        <Trash2 size={14} />
                      </button>
                    </Show>
                  </li>
                )}
              </For>
            </ul>

            <Show when={hasPassword()}>
              <label class="mb-1 block text-xs font-medium text-secondary">
                {t('vault.currentPassword')}
              </label>
              <input
                type="password"
                class={`${inputClass} mb-1`}
                value={current()}
                onInput={(e) => setCurrent(e.currentTarget.value)}
                autocomplete="current-password"
                aria-label={t('vault.currentPassword')}
              />
              <p class="mb-4 text-xs text-tertiary">{t('vault.currentPasswordHint')}</p>
            </Show>

            <form onSubmit={(e) => void changePassword(e)} class="mb-4 space-y-2">
              <h3 class="text-xs font-medium text-secondary">
                {hasPassword() ? t('vault.changePassword') : t('vault.addPassword')}
              </h3>
              <input
                type="password"
                class={inputClass}
                value={newPassword()}
                onInput={(e) => setNewPassword(e.currentTarget.value)}
                placeholder={t('vault.newPasswordPlaceholder')}
                aria-label={t('vault.newPasswordPlaceholder')}
                autocomplete="new-password"
              />
              <input
                type="password"
                class={inputClass}
                value={confirmPassword()}
                onInput={(e) => setConfirmPassword(e.currentTarget.value)}
                placeholder={t('vault.confirmPasswordPlaceholder')}
                aria-label={t('vault.confirmPasswordPlaceholder')}
                autocomplete="new-password"
              />
              <button
                type="submit"
                class="rounded-md interactive-primary px-3 py-1.5 text-sm font-medium disabled:opacity-50"
                disabled={busy()}
              >
                {hasPassword() ? t('vault.changePassword') : t('vault.addPassword')}
              </button>
            </form>

            <div class="mb-4 space-y-2">
              <h3 class="text-xs font-medium text-secondary">{t('vault.addKeyFile')}</h3>
              <input
                class={inputClass}
                value={keyLabel()}
                onInput={(e) => setKeyLabel(e.currentTarget.value)}
                placeholder={t('vault.keyFileLabelPlaceholder')}
                aria-label={t('vault.keyFileLabelPlaceholder')}
              />
              <button
                type="button"
                class="rounded-md border border-primary px-3 py-1.5 text-sm text-secondary hover:bg-hover disabled:opacity-50"
                disabled={busy()}
                onClick={() => void addKeyFile()}
              >
                {t('vault.createKeyFile')}
              </button>
              <p class="text-xs text-tertiary">{t('vault.keyFileHint')}</p>
            </div>

            <button
              type="button"
              class="inline-flex items-center gap-2 rounded-md border border-primary px-3 py-1.5 text-sm text-secondary hover:bg-hover disabled:opacity-50"
              disabled={busy()}
              onClick={() => void verify()}
            >
              <ShieldCheck size={15} />
              {t('vault.verify')}
            </button>

            <Show when={message()}>
              <p class="mt-3 text-sm text-secondary" data-testid="vault-settings-message">
                {message()}
              </p>
            </Show>
            <Show when={error()}>
              <p role="alert" class="mt-3 text-sm text-error">
                {error()}
              </p>
            </Show>
          </Dialog.Content>
        </div>
      </Dialog.Portal>
    </Dialog>
  );
}
