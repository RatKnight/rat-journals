import { createEffect, createSignal, Show, Switch, Match, on } from 'solid-js';
import { Dialog } from '@kobalte/core/dialog';
import { FolderOpen, KeyRound } from 'lucide-solid';
import * as tauri from '../../lib/tauri';
import { open, save } from '../../lib/dialog';
import { mapTauriError } from '../../lib/errors';
import { useI18n } from '../../i18n';
import { isVaultDialogOpen, setIsVaultDialogOpen, showOpenedVault } from '../../state/vault';

type Step = 'choose' | 'unlock' | 'create';

/** Opens an existing vault folder, or creates a new vault in an empty folder. */
export default function VaultOpenDialog() {
  const t = useI18n();
  const [step, setStep] = createSignal<Step>('choose');
  const [folder, setFolder] = createSignal('');
  const [hasPassword, setHasPassword] = createSignal(true);
  const [hasKeyFile, setHasKeyFile] = createSignal(false);
  const [password, setPassword] = createSignal('');
  const [confirmPassword, setConfirmPassword] = createSignal('');
  const [withKeyFile, setWithKeyFile] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [busy, setBusy] = createSignal(false);

  const reset = () => {
    setStep('choose');
    setFolder('');
    setPassword('');
    setConfirmPassword('');
    setWithKeyFile(false);
    setError(null);
    setBusy(false);
  };

  const showUnlock = (path: string, slots: tauri.VaultSlotInfo[]) => {
    setFolder(path);
    setHasPassword(slots.some((s) => s.slot_type === 'password'));
    setHasKeyFile(slots.some((s) => s.slot_type === 'keyfile'));
    setStep('unlock');
  };

  // Each time the dialog opens, go straight to unlocking the vault opened last time (if
  // it is still there). "Choose a different folder" goes back to the folder picker.
  createEffect(
    on(isVaultDialogOpen, (isOpen) => {
      if (!isOpen) return;
      void tauri
        .vaultLastFolder()
        .then((last) => {
          // Only if the user has not already picked something while this loaded.
          if (last && isVaultDialogOpen() && step() === 'choose' && !folder()) {
            showUnlock(last.path, last.slots);
          }
        })
        .catch(() => {
          // Nothing remembered: the folder picker is shown as usual.
        });
    }),
  );

  const handleOpenChange = (isOpen: boolean) => {
    if (!isOpen && busy()) return;
    if (!isOpen) reset();
    setIsVaultDialogOpen(isOpen);
  };

  const chooseFolder = async () => {
    setError(null);
    const picked = await open({ directory: true, multiple: false, title: t('vault.chooseFolder') });
    if (!picked || typeof picked !== 'string') return;
    try {
      const info = await tauri.vaultCheckFolder(picked);
      if (info.is_vault) {
        showUnlock(picked, info.slots);
      } else {
        setFolder(picked);
        setStep('create');
      }
    } catch (err) {
      setError(mapTauriError(err, t));
    }
  };

  const run = async (work: () => Promise<tauri.VaultStatus>) => {
    setError(null);
    setBusy(true);
    try {
      const status = await work();
      reset();
      await showOpenedVault(status);
    } catch (err) {
      setError(mapTauriError(err, t));
      setPassword('');
      setConfirmPassword('');
    } finally {
      setBusy(false);
    }
  };

  const unlockWithPassword = (e: Event) => {
    e.preventDefault();
    if (!password()) {
      setError(t('vault.passwordRequired'));
      return;
    }
    void run(() => tauri.vaultUnlockPassword(folder(), password()));
  };

  const unlockWithKeyFile = async () => {
    const key = await open({
      multiple: false,
      directory: false,
      title: t('vault.chooseKeyFile'),
      filters: [{ name: 'Key Files', extensions: ['key', 'txt', '*'] }],
    });
    if (!key || typeof key !== 'string') return;
    void run(() => tauri.vaultUnlockKeyFile(folder(), key));
  };

  const create = async (e: Event) => {
    e.preventDefault();
    if (!password() && !withKeyFile()) {
      setError(t('vault.needPasswordOrKey'));
      return;
    }
    if (password() && password() !== confirmPassword()) {
      setError(t('vault.passwordsDontMatch'));
      return;
    }
    let keyPath: string | null = null;
    if (withKeyFile()) {
      keyPath = await save({
        title: t('vault.saveKeyFileTitle'),
        defaultPath: 'rat-journal-vault.key',
        filters: [{ name: 'Key Files', extensions: ['key'] }],
      });
      if (!keyPath) return;
    }
    void run(() => tauri.vaultCreate(folder(), password() || null, keyPath));
  };

  const inputClass =
    'w-full rounded-md border border-primary bg-primary px-3 py-2 text-sm text-primary focus:outline-none focus:ring-2 focus:ring-blue-500';
  const primaryButton =
    'w-full rounded-md interactive-primary px-4 py-2 text-sm font-medium disabled:opacity-50';
  const secondaryButton =
    'flex w-full items-center justify-center gap-2 rounded-md border border-primary px-4 py-2 text-sm text-secondary hover:bg-hover disabled:opacity-50';

  return (
    <Dialog open={isVaultDialogOpen()} onOpenChange={handleOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay
          class="fixed inset-0 z-50"
          style={{ 'background-color': 'var(--overlay-bg)' }}
        />
        <div class="fixed inset-0 z-50 flex items-center justify-center p-4">
          <Dialog.Content
            class="w-full max-w-md rounded-lg bg-primary p-6"
            style={{ 'box-shadow': 'var(--shadow-lg)' }}
            data-testid="vault-open-dialog"
          >
            <Dialog.Title class="mb-2 text-lg font-semibold text-primary">
              {step() === 'create' ? t('vault.createTitle') : t('vault.openTitle')}
            </Dialog.Title>

            <Switch>
              <Match when={step() === 'choose'}>
                <p class="mb-4 text-sm text-secondary">{t('vault.chooseIntro')}</p>
                <button type="button" class={primaryButton} onClick={() => void chooseFolder()}>
                  <span class="inline-flex items-center gap-2">
                    <FolderOpen size={16} />
                    {t('vault.chooseFolderButton')}
                  </span>
                </button>
              </Match>

              <Match when={step() === 'unlock'}>
                <p class="mb-4 break-all text-xs text-tertiary">{folder()}</p>
                <Show when={hasPassword()}>
                  <form onSubmit={unlockWithPassword} class="mb-3 space-y-3">
                    <input
                      type="password"
                      class={inputClass}
                      value={password()}
                      onInput={(e) => setPassword(e.currentTarget.value)}
                      placeholder={t('vault.passwordPlaceholder')}
                      aria-label={t('vault.passwordPlaceholder')}
                      autocomplete="current-password"
                      autofocus
                      disabled={busy()}
                    />
                    <button type="submit" class={primaryButton} disabled={busy()}>
                      {busy() ? t('vault.unlocking') : t('vault.unlockButton')}
                    </button>
                  </form>
                </Show>
                <Show when={hasKeyFile()}>
                  <button
                    type="button"
                    class={secondaryButton}
                    disabled={busy()}
                    onClick={() => void unlockWithKeyFile()}
                  >
                    <KeyRound size={16} />
                    {t('vault.unlockWithKeyFile')}
                  </button>
                </Show>
              </Match>

              <Match when={step() === 'create'}>
                <p class="mb-1 text-sm text-secondary">{t('vault.createIntro')}</p>
                <p class="mb-4 break-all text-xs text-tertiary">{folder()}</p>
                <form onSubmit={(e) => void create(e)} class="space-y-3">
                  <input
                    type="password"
                    class={inputClass}
                    value={password()}
                    onInput={(e) => setPassword(e.currentTarget.value)}
                    placeholder={t('vault.newPasswordPlaceholder')}
                    aria-label={t('vault.newPasswordPlaceholder')}
                    autocomplete="new-password"
                    disabled={busy()}
                  />
                  <input
                    type="password"
                    class={inputClass}
                    value={confirmPassword()}
                    onInput={(e) => setConfirmPassword(e.currentTarget.value)}
                    placeholder={t('vault.confirmPasswordPlaceholder')}
                    aria-label={t('vault.confirmPasswordPlaceholder')}
                    autocomplete="new-password"
                    disabled={busy()}
                  />
                  <p class="text-xs text-tertiary">{t('vault.passphraseHint')}</p>
                  <label class="flex items-start gap-2 text-sm text-secondary">
                    <input
                      type="checkbox"
                      class="mt-1"
                      checked={withKeyFile()}
                      onChange={(e) => setWithKeyFile(e.currentTarget.checked)}
                      disabled={busy()}
                    />
                    <span>{t('vault.alsoKeyFile')}</span>
                  </label>
                  <button type="submit" class={primaryButton} disabled={busy()}>
                    {busy() ? t('vault.creating') : t('vault.createButton')}
                  </button>
                </form>
              </Match>
            </Switch>

            <Show when={error()}>
              <p role="alert" class="mt-3 text-sm text-error">
                {error()}
              </p>
            </Show>

            <div class="mt-4 flex justify-between">
              <Show when={step() !== 'choose'}>
                <button
                  type="button"
                  class="text-sm text-tertiary hover:text-secondary underline"
                  disabled={busy()}
                  onClick={() => {
                    reset();
                  }}
                >
                  {t('vault.chooseDifferentFolder')}
                </button>
              </Show>
              <button
                type="button"
                class="ml-auto text-sm text-tertiary hover:text-secondary underline"
                disabled={busy()}
                onClick={() => handleOpenChange(false)}
              >
                {t('common.cancel')}
              </button>
            </div>
          </Dialog.Content>
        </div>
      </Dialog.Portal>
    </Dialog>
  );
}
