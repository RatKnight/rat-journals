import { Show } from 'solid-js';
import { BookOpen, Lock } from 'lucide-solid';
import RatVaultIcon from '../icons/RatVaultIcon';
import { useI18n } from '../../i18n';
import { authState, lockJournal } from '../../state/auth';
import {
  isVaultOpen,
  isVaultShown,
  lockVault,
  showJournalView,
  showVaultView,
} from '../../state/vault';

/**
 * The narrow navigation bar on the left of every screen: switch between the journal and the
 * vault, and lock everything. Switching away from the vault keeps it unlocked (the usual
 * auto-lock rules still close it).
 */
export default function AppRail() {
  const t = useI18n();
  const journalUnlocked = () => authState() === 'unlocked';
  const anythingOpen = () => journalUnlocked() || isVaultOpen();

  // The journal lock also closes the vault in the backend.
  const lockAll = () => {
    if (journalUnlocked()) void lockJournal();
    else if (isVaultOpen()) void lockVault();
  };

  const item = (active: boolean) =>
    `relative flex w-full flex-col items-center gap-1 rounded-md px-1 py-2 text-[11px] font-medium ${
      active ? 'interactive-primary' : 'text-secondary hover:bg-hover'
    }`;

  return (
    <nav
      class="flex w-[72px] shrink-0 flex-col items-stretch gap-1 border-r border-primary bg-primary p-2"
      aria-label={t('layout.rail.label')}
      data-testid="app-rail"
    >
      <button
        type="button"
        class={item(!isVaultShown())}
        aria-current={!isVaultShown() ? 'page' : undefined}
        onClick={showJournalView}
        data-testid="rail-journal"
      >
        <BookOpen size={20} aria-hidden="true" />
        {t('layout.rail.journal')}
      </button>
      <button
        type="button"
        class={item(isVaultShown())}
        aria-current={isVaultShown() ? 'page' : undefined}
        onClick={showVaultView}
        title={isVaultOpen() ? undefined : t('layout.header.openVault')}
        data-testid="rail-vault"
      >
        <RatVaultIcon size={20} aria-hidden="true" />
        {t('layout.rail.vault')}
        <Show when={isVaultOpen() && !isVaultShown()}>
          {/* The vault is unlocked in the background. */}
          <span
            class="absolute right-2 top-1.5 h-2 w-2 rounded-full bg-green-500"
            aria-label={t('layout.rail.vaultOpen')}
          />
        </Show>
      </button>
      <span class="flex-1" />
      <Show when={anythingOpen()}>
        <button
          type="button"
          class={item(false)}
          onClick={lockAll}
          title={t('layout.rail.lockAll')}
          data-testid="rail-lock"
        >
          <Lock size={20} aria-hidden="true" />
          {t('layout.rail.lock')}
        </button>
      </Show>
    </nav>
  );
}
