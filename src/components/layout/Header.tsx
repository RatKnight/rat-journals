import { Show } from 'solid-js';
import {
  Menu,
  Lock,
  Info,
  List,
  PenLine,
  Search,
  ChevronLeft,
  ChevronRight,
  Vault,
} from 'lucide-solid';
import {
  selectedDate,
  setIsAboutOpen,
  isSidebarCollapsed,
  mainView,
  requestMainViewChange,
  setIsSearchOpen,
  setIsGoToDateOpen,
} from '../../state/ui';
import { goToPreviousDay, goToNextDay } from '../../lib/day-navigation';
import { authState, lockJournal } from '../../state/auth';
import { preferences } from '../../state/preferences';
import { useI18n } from '../../i18n';
import HeaderMoreMenu from './HeaderMoreMenu';
import { setIsVaultDialogOpen } from '../../state/vault';

interface HeaderProps {
  onMenuClick?: () => void;
  showMenu?: boolean;
}

export default function Header(props: HeaderProps) {
  const t = useI18n();
  // Read the shared auth state, not a local flag: Escape, the idle timer and focus-loss
  // lock also start a lock, and the lock backdrop that blocks clicks is hidden under
  // prefers-reduced-motion.
  const isLocking = () => authState() === 'locking';

  // Format date: "Tuesday, January 1, 2019"
  const formattedDate = () => {
    const date = new Date(selectedDate() + 'T00:00:00');
    return date.toLocaleDateString(preferences().language, {
      weekday: 'long',
      year: 'numeric',
      month: 'long',
      day: 'numeric',
    });
  };

  const handleLock = async () => {
    if (isLocking()) return;
    await lockJournal();
  };

  return (
    <header class="flex h-16 items-center justify-between border-b border-primary bg-primary px-4">
      {/* Left: hamburger + search + date */}
      <div class="flex items-center gap-3">
        <Show when={props.showMenu}>
          <button
            onClick={() => props.onMenuClick?.()}
            data-testid="toggle-sidebar-button"
            class="rounded p-2 hover:bg-hover text-primary lg:hidden"
            aria-label={t('layout.header.toggleMenu')}
            aria-expanded={!isSidebarCollapsed()}
            aria-controls="sidebar"
          >
            <Menu size={24} />
          </button>
        </Show>
        <button
          onClick={() => setIsSearchOpen(true)}
          data-testid="search-button"
          class="rounded p-2 hover:bg-hover text-tertiary transition-colors"
          aria-label={t('layout.header.search')}
        >
          <Search size={20} />
        </button>
        <button
          onClick={() => goToPreviousDay()}
          data-testid="header-prev-day-button"
          class="rounded p-2 hover:bg-hover text-tertiary transition-colors"
          aria-label={t('layout.header.previousDay')}
        >
          <ChevronLeft size={20} />
        </button>
        <button
          onClick={() => setIsGoToDateOpen(true)}
          data-testid="header-date-title"
          class="border-0 bg-transparent p-0 text-lg font-semibold text-primary cursor-pointer hover:text-interactive transition-colors"
          aria-label={t('layout.header.goToDate')}
        >
          {formattedDate()}
        </button>
        <button
          onClick={() => goToNextDay()}
          data-testid="header-next-day-button"
          class="rounded p-2 hover:bg-hover text-tertiary transition-colors"
          aria-label={t('layout.header.nextDay')}
        >
          <ChevronRight size={20} />
        </button>
      </div>

      {/* Right: Timeline toggle + About + Lock */}
      <div class="flex items-center gap-1">
        <button
          onClick={() =>
            void requestMainViewChange(mainView() === 'timeline' ? 'editor' : 'timeline')
          }
          class="rounded p-2 hover:bg-hover text-tertiary transition-colors"
          aria-label={
            mainView() === 'timeline'
              ? t('layout.header.showEditor')
              : t('layout.header.showTimeline')
          }
          aria-pressed={mainView() === 'timeline'}
          data-testid="timeline-toggle-button"
        >
          <Show when={mainView() === 'timeline'} fallback={<List size={20} />}>
            <PenLine size={20} />
          </Show>
        </button>
        <button
          type="button"
          onClick={() => setIsVaultDialogOpen(true)}
          data-testid="header-vault-button"
          class="inline-flex items-center gap-1.5 rounded-md border border-primary px-3 py-1.5 text-sm font-medium text-secondary hover:bg-hover transition-colors"
          title={t('layout.header.openVault')}
        >
          <Vault size={18} aria-hidden="true" />
          {t('layout.header.menuVault')}
        </button>
        <button
          data-tour-target="about"
          onClick={() => setIsAboutOpen(true)}
          class="rounded p-2 hover:bg-hover text-tertiary transition-colors"
          aria-label={t('layout.header.about')}
        >
          <Info size={20} />
        </button>
        <button
          onClick={() => handleLock()}
          disabled={isLocking()}
          data-testid="lock-journal-button"
          class="rounded p-2 hover:bg-hover text-tertiary transition-colors disabled:opacity-50"
          aria-label={t('layout.header.lockJournal')}
        >
          <Lock size={20} />
        </button>
        <HeaderMoreMenu />
      </div>
    </header>
  );
}
