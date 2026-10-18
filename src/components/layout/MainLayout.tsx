import { For, onMount, onCleanup, Show } from 'solid-js';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { createLogger } from '../../lib/logger';
import Header from './Header';
import Sidebar from './Sidebar';
import EditorPanel from './EditorPanel';
import Timeline from '../timeline/Timeline';
import GoToDateOverlay from '../overlays/GoToDateOverlay';
import PreferencesOverlay from '../overlays/preferences/PreferencesOverlay';
import StatsOverlay from '../overlays/StatsOverlay';
import ImportOverlay from '../overlays/ImportOverlay';
import ExportOverlay from '../overlays/ExportOverlay';
import TagManager from '../overlays/TagManager';
import OnboardingTour from '../overlays/OnboardingOverlay';
import SearchOverlay from '../search/SearchOverlay';
import ConfirmDialog from '../overlays/ConfirmDialog';
import {
  mainView,
  isSidebarCollapsed,
  setIsSidebarCollapsed,
  isPreferencesOpen,
  setIsPreferencesOpen,
  isStatsOpen,
  setIsStatsOpen,
  isImportOpen,
  setIsImportOpen,
  isExportOpen,
  setIsExportOpen,
  isTagManagerOpen,
  setIsTagManagerOpen,
  isAnyOverlayOpen,
} from '../../state/ui';
import { preferences } from '../../state/preferences';
import {
  authState,
  lockJournal,
  securityNotice,
  dismissSecurityNotice,
  deadManWarnings,
  dismissDeadManWarnings,
} from '../../state/auth';
import { useI18n } from '../../i18n';
import { registerKeyboardShortcuts } from '../../lib/keyboard-shortcuts';
import { isEscapeClaimed } from '../../lib/escape-claim';
import { onboardingMode, minimizeOnboarding } from '../../state/onboarding';

const log = createLogger('MainLayout');

export default function MainLayout() {
  const t = useI18n();
  // Store cleanup functions at component level
  const unlisteners: UnlistenFn[] = [];
  let unregisterShortcuts: (() => void) | undefined;

  // Escape events that arrived while a dialog was open. A dialog's own Escape handler
  // closes it during the same dispatch, and Solid's delegated listener on `document` can
  // run before handleGlobalEsc, so reading isAnyOverlayOpen() there is too late. This
  // window capture-phase listener runs before every handler and records the state.
  const escWhileOverlayOpen = new WeakSet<KeyboardEvent>();
  const recordOverlayOnEsc = (e: KeyboardEvent) => {
    if (e.key === 'Escape' && isAnyOverlayOpen()) escWhileOverlayOpen.add(e);
  };

  const handleGlobalEsc = (e: KeyboardEvent) => {
    if (e.key !== 'Escape') return;
    // An IME uses Escape to cancel composition, and a non-modal popup (e.g. the tag
    // dropdown) claims the Escape it already handled. defaultPrevented is not usable:
    // ProseMirror sets it on every Escape in the editor body.
    if (e.isComposing || isEscapeClaimed(e)) return;
    if (onboardingMode() === 'tour') {
      minimizeOnboarding();
      return;
    }
    if (onboardingMode() === 'minimized') return;
    // Never fire when any dialog is or was open at keydown — they handle their own Escape
    if (escWhileOverlayOpen.has(e) || isAnyOverlayOpen()) return;
    const action = preferences().escAction;
    if (action === 'quit') {
      getCurrentWindow()
        .close()
        .catch((err) => log.error('Failed to close window:', err));
    } else if (action === 'lock' && authState() === 'unlocked') {
      // MainLayout stays mounted during 'locking' (the lock animation), so key repeat
      // would call lockJournal() again without the 'unlocked' check.
      void lockJournal().catch((err) => log.error('Failed to lock journal:', err));
    }
  };

  onMount(async () => {
    window.addEventListener('keydown', recordOverlayOnEsc, true);
    document.addEventListener('keydown', handleGlobalEsc);
    unregisterShortcuts = registerKeyboardShortcuts();

    // Preferences is the only remaining native menu item (TODO-0065) — every other
    // action moved into the webview, so this is the last `menu-*` listener.
    unlisteners.push(
      await listen('menu-preferences', () => {
        setIsPreferencesOpen(true);
      }),
    );
  });

  // Cleanup on component unmount
  onCleanup(() => {
    unlisteners.forEach((unlisten) => unlisten());
    window.removeEventListener('keydown', recordOverlayOnEsc, true);
    document.removeEventListener('keydown', handleGlobalEsc);
    unregisterShortcuts?.();
  });

  return (
    <div class="flex h-full overflow-hidden bg-secondary">
      {/* Backdrop that fades out on mount — creates the blur-dissolve unlock animation */}
      <div class="unlock-backdrop" aria-hidden="true" />

      {/* Sidebar */}
      <Sidebar isCollapsed={isSidebarCollapsed()} onClose={() => setIsSidebarCollapsed(true)} />

      {/* Main content area */}
      <div class="flex flex-1 flex-col">
        {/* Header */}
        <Header showMenu onMenuClick={() => setIsSidebarCollapsed(!isSidebarCollapsed())} />

        {/* Journals or the vault that a dead man's switch will erase soon */}
        <Show when={deadManWarnings().length > 0}>
          <div class="px-6 pt-2">
            <div
              role="alert"
              data-testid="dead-man-warning"
              class="rounded-md bg-warning p-3 flex items-start justify-between gap-2"
            >
              <div class="text-sm text-warning">
                <For each={deadManWarnings()}>
                  {(w) => (
                    <p>
                      {t(w.kind === 'vault' ? 'deadMan.warningVault' : 'deadMan.warningJournal', {
                        name: w.name,
                        days: w.days_left,
                      })}
                    </p>
                  )}
                </For>
              </div>
              <button
                type="button"
                onClick={() => dismissDeadManWarnings()}
                class="text-sm text-warning hover:opacity-75 flex-shrink-0"
                aria-label={t('common.close')}
              >
                ×
              </button>
            </div>
          </div>
        </Show>

        {/* The encryption upgrade failed or needs all methods: the journal file is still exposed */}
        <Show when={securityNotice()}>
          <div class="px-6 pt-2">
            <div
              role="alert"
              data-testid="security-notice"
              class="rounded-md bg-warning p-3 flex items-center justify-between gap-2"
            >
              <p class="text-sm text-warning">
                {securityNotice() === 'upgrade-needs-all-methods'
                  ? t('errors.upgradeNeedsAllMethods')
                  : securityNotice() === 'clock-set-back'
                    ? t('errors.clockSetBack')
                    : t('errors.keyUpgradeFailed')}
              </p>
              <button
                type="button"
                onClick={() => dismissSecurityNotice()}
                class="text-sm text-warning hover:opacity-75 flex-shrink-0"
                aria-label={t('common.close')}
              >
                ×
              </button>
            </div>
          </div>
        </Show>

        {/* Main panel: editor or timeline list */}
        <main class="flex-1 overflow-hidden">
          <Show when={mainView() === 'timeline'} fallback={<EditorPanel />}>
            <Timeline />
          </Show>
        </main>
      </div>

      {/* Overlays */}
      <GoToDateOverlay />
      <PreferencesOverlay
        isOpen={isPreferencesOpen()}
        onClose={() => setIsPreferencesOpen(false)}
      />
      <StatsOverlay isOpen={isStatsOpen()} onClose={() => setIsStatsOpen(false)} />
      <ImportOverlay
        isOpen={isImportOpen()}
        onClose={() => setIsImportOpen(false)}
        onImportComplete={() => {
          // Sidebar will auto-refresh calendar dates via its own effect
          // Just notify user visually that import succeeded (already in overlay)
        }}
      />
      <ExportOverlay isOpen={isExportOpen()} onClose={() => setIsExportOpen(false)} />
      <TagManager isOpen={isTagManagerOpen()} onClose={() => setIsTagManagerOpen(false)} />
      <OnboardingTour />
      <SearchOverlay />
      <ConfirmDialog />
    </div>
  );
}
