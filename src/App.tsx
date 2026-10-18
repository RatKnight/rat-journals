import { Match, Show, Switch, createEffect, onCleanup, onMount } from 'solid-js';
import { authState, initializeAuth, lockJournal, setupAuthEventListeners } from './state/auth';
import { initializeTheme } from './lib/theme';
import { createLogger } from './lib/logger';
import { createFocusLossAutoLock } from './lib/focus-lock';
import { createVaultIdleLock } from './lib/vault-idle-lock';
import { preferences } from './state/preferences';
import { setLocale, useI18n } from './i18n';
import { setSpellcheckEnabled, updateMenuLocale } from './lib/tauri';
import { isAboutOpen, setIsAboutOpen } from './state/ui';
import JournalPicker from './components/auth/JournalPicker';
import PasswordCreation from './components/auth/PasswordCreation';
import PasswordPrompt from './components/auth/PasswordPrompt';
import MainLayout from './components/layout/MainLayout';
import AppRail from './components/layout/AppRail';
import AboutOverlay from './components/overlays/AboutOverlay';
import VaultOpenDialog from './components/vault/VaultOpenDialog';
import VaultWindow from './components/vault/VaultWindow';
import { isVaultOpen, isVaultShown, lockVault, setupVaultEventListeners } from './state/vault';

const log = createLogger('App');

/** Locks whatever is open: the journal lock also closes a vault in the backend. */
function lockEverything(): void {
  if (authState() === 'unlocked') void lockJournal();
  else if (isVaultOpen()) void lockVault();
}

function App() {
  const t = useI18n();
  const ACTIVITY_EVENTS = ['mousemove', 'keydown', 'click', 'touchstart', 'scroll'] as const;
  let idleTimer: ReturnType<typeof setTimeout> | null = null;

  // Sync locale signal from persisted preference — runs immediately on mount
  // and re-runs whenever preferences().language changes.
  // Also updates native OS menu labels via the backend command.
  createEffect(() => {
    const lang = preferences().language;
    setLocale(lang);
    void updateMenuLocale(lang);
  });

  // Push the spellcheck preference down to the WebView itself. The HTML `spellcheck`
  // attribute in the editor is enough on Windows/macOS, but WebKitGTK runs no checker
  // until it is enabled on the web context — see src-tauri/src/spellcheck.rs. Kept
  // separate from the locale effect: different concern, and it must re-run on either input.
  createEffect(() => {
    const { enableSpellcheck, language } = preferences();
    void setSpellcheckEnabled(enableSpellcheck, language);
  });

  createEffect(() => {
    const { autoLockEnabled, autoLockTimeout } = preferences();
    const state = authState();

    if (!autoLockEnabled || (state !== 'unlocked' && !isVaultOpen())) return;

    function handleActivity() {
      if (idleTimer !== null) clearTimeout(idleTimer);
      idleTimer = setTimeout(lockEverything, autoLockTimeout * 1000);
    }

    handleActivity(); // start initial timer
    ACTIVITY_EVENTS.forEach((e) => document.addEventListener(e, handleActivity, { passive: true }));

    onCleanup(() => {
      if (idleTimer !== null) {
        clearTimeout(idleTimer);
        idleTimer = null;
      }
      ACTIVITY_EVENTS.forEach((e) => document.removeEventListener(e, handleActivity));
    });
  });

  createVaultIdleLock({
    minutes: () => preferences().vaultLockMinutes,
    isOpen: isVaultOpen,
    isShown: isVaultShown,
    lock: () => void lockVault(),
  });

  createFocusLossAutoLock({
    enabled: () => preferences().autoLockOnFocusLoss,
    isUnlocked: () => authState() === 'unlocked' || isVaultOpen(),
    lock: lockEverything,
  });

  onMount(() => {
    initializeAuth();
    initializeTheme();

    let cleanupAuthListeners: (() => void) | undefined;
    void setupAuthEventListeners()
      .then((cleanup) => {
        cleanupAuthListeners = cleanup;
      })
      .catch((error) => {
        // Listener setup failure should not block app startup.
        log.error('Failed to setup auth event listeners:', error);
      });

    let cleanupVaultListeners: (() => void) | undefined;
    void setupVaultEventListeners()
      .then((cleanup) => {
        cleanupVaultListeners = cleanup;
      })
      .catch((error) => log.error('Failed to setup vault event listeners:', error));

    onCleanup(() => {
      cleanupAuthListeners?.();
      cleanupVaultListeners?.();
    });
  });

  return (
    <div class="flex h-full">
      <Show when={authState() !== 'checking'}>
        <AppRail />
      </Show>
      <div class="relative h-full min-w-0 flex-1">
        <Switch>
          <Match when={authState() === 'checking'}>
            <div class="flex h-full items-center justify-center bg-secondary">
              <div class="text-center">
                <div class="mb-4 h-12 w-12 animate-spin rounded-full border-4 spinner-border border-t-transparent mx-auto" />
                <p class="text-secondary">{t('layout.loading')}</p>
              </div>
            </div>
          </Match>

          <Match when={authState() === 'journal-select'}>
            <JournalPicker />
          </Match>

          <Match when={authState() === 'no-journal'}>
            <PasswordCreation />
          </Match>

          <Match when={authState() === 'locked'}>
            <PasswordPrompt />
          </Match>

          <Match when={authState() === 'unlocked' || authState() === 'locking'}>
            <MainLayout />
          </Match>
        </Switch>

        {/* Lock backdrop — fades in over the mounted MainLayout, then locked state unmounts it */}
        <Show when={authState() === 'locking'}>
          <div class="lock-backdrop" aria-hidden="true" />
        </Show>

        {/* The vault is independent of the journal: it covers the journal side while it is the
          view in front, and stays unlocked in the background when the rail switches away. */}
        <Show when={isVaultShown()}>
          <VaultWindow />
        </Show>
      </div>

      <AboutOverlay isOpen={isAboutOpen()} onClose={() => setIsAboutOpen(false)} />
      <VaultOpenDialog />
    </div>
  );
}

export default App;
