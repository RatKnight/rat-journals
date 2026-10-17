import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { waitFor } from '@solidjs/testing-library';
import { renderWithI18n } from '../../test/i18n-test-utils';
import { mockTauriBarrel } from '../../test/mock-tauri';
import {
  resetUiState,
  setIsMoreMenuOpen,
  isLinkDialogOpen,
  setIsLinkDialogOpen,
  isTimestampDialogOpen,
  setIsTimestampDialogOpen,
  isSearchOpen,
  isPreferencesOpen,
  setIsPreferencesOpen,
  selectedDate,
  setSelectedDate,
} from '../../state/ui';
import { setPreferences, resetPreferences } from '../../state/preferences';
import { setEntryDates } from '../../state/entries';
import { startOnboarding, dismissOnboarding, onboardingMode } from '../../state/onboarding';

const mockClose = vi.hoisted(() => vi.fn(() => Promise.resolve()));

// Capture the real menu-event handlers MainLayout registers via listen(), and
// stub the navigation wrapper so we can assert the date it is invoked with.
const eventMocks = vi.hoisted(() => ({
  navigatePreviousDay: vi.fn(),
  listeners: new Map<string, (event: unknown) => void | Promise<void>>(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (event: string, handler: (e: unknown) => void | Promise<void>) => {
    eventMocks.listeners.set(event, handler);
    return () => eventMocks.listeners.delete(event);
  }),
  emit: vi.fn(async () => {}),
}));

vi.mock('../../lib/tauri', () =>
  mockTauriBarrel({ navigatePreviousDay: eventMocks.navigatePreviousDay }),
);

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ close: mockClose }),
}));

// Keep the real auth module, but control the auth state and replace lockJournal() so
// the Escape → lock tests do not run the real lock flow.
const authMocks = vi.hoisted(() => ({
  lockJournal: vi.fn(() => Promise.resolve()),
  state: 'unlocked' as string,
  securityNotice: null as string | null,
  dismissSecurityNotice: vi.fn(),
}));

vi.mock('../../state/auth', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../state/auth')>()),
  authState: () => authMocks.state,
  lockJournal: authMocks.lockJournal,
  securityNotice: () => authMocks.securityNotice,
  dismissSecurityNotice: authMocks.dismissSecurityNotice,
}));

const mockLogError = vi.hoisted(() => vi.fn());

vi.mock('../../lib/logger', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../lib/logger')>();
  return {
    ...actual,
    createLogger: (module: string) =>
      module === 'MainLayout'
        ? { ...actual.createLogger(module), error: mockLogError }
        : actual.createLogger(module),
  };
});

// Isolate MainLayout's own keydown-guard logic from its heavy child tree
// (editor, sidebar, calendar, overlays) — none of it is relevant here.
vi.mock('./Header', () => ({ default: () => null }));
vi.mock('./Sidebar', () => ({ default: () => null }));
vi.mock('./EditorPanel', () => ({ default: () => null }));
vi.mock('../timeline/Timeline', () => ({ default: () => null }));
vi.mock('../overlays/GoToDateOverlay', () => ({ default: () => null }));
vi.mock('../overlays/preferences/PreferencesOverlay', () => ({ default: () => null }));
vi.mock('../overlays/StatsOverlay', () => ({ default: () => null }));
vi.mock('../overlays/ImportOverlay', () => ({ default: () => null }));
vi.mock('../overlays/ExportOverlay', () => ({ default: () => null }));
vi.mock('../overlays/NotificationsOverlay', () => ({ default: () => null }));
vi.mock('../overlays/TagManager', () => ({ default: () => null }));
vi.mock('../overlays/OnboardingOverlay', () => ({ default: () => null }));
vi.mock('../search/SearchOverlay', () => ({ default: () => null }));

import { delegateEvents } from 'solid-js/web';
import { Editor as TiptapEditor, type Editor } from '@tiptap/core';
import StarterKit from '@tiptap/starter-kit';
import MainLayout from './MainLayout';
import { claimEscape } from '../../lib/escape-claim';
import LinkOverlay from '../editor/LinkOverlay';
import TimestampOverlay from '../editor/TimestampOverlay';

// In the app bundle, modules with a native `onKeyDown` (e.g. EntryTags) register Solid's
// delegated keydown listener on `document` at import time, before MainLayout mounts.
// Do the same here so the handler order matches the app.
delegateEvents(['keydown']);

// Just enough of an Editor for the dialogs to open and close; no insert runs.
const dialogEditor = {
  isActive: () => false,
  getAttributes: () => ({}),
  state: { selection: { empty: true, from: 0, to: 0 }, doc: { textBetween: () => '' } },
} as unknown as Editor;

describe('MainLayout global keydown guards', () => {
  beforeEach(() => {
    resetUiState();
    resetPreferences();
    mockClose.mockClear();
    authMocks.lockJournal.mockReset();
    authMocks.lockJournal.mockImplementation(() => Promise.resolve());
    authMocks.state = 'unlocked';
    mockLogError.mockClear();
    dismissOnboarding();
  });

  afterEach(() => {
    resetUiState();
    resetPreferences();
  });

  it('does not quit on Escape while the header overflow menu is open', () => {
    setPreferences({ escAction: 'quit' });
    setIsMoreMenuOpen(true);
    renderWithI18n(() => <MainLayout />);

    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));

    expect(mockClose).not.toHaveBeenCalled();
  });

  it('quits on Escape when no overlay or menu is open', () => {
    setPreferences({ escAction: 'quit' });
    renderWithI18n(() => <MainLayout />);

    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));

    expect(mockClose).toHaveBeenCalledTimes(1);
  });

  it.each([
    ['Insert Link', setIsLinkDialogOpen],
    ['Insert Timestamp', setIsTimestampDialogOpen],
  ] as const)('does not quit on Escape while the %s dialog is open', (_name, setOpen) => {
    setPreferences({ escAction: 'quit' });
    setOpen(true);
    renderWithI18n(() => <MainLayout />);

    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));

    expect(mockClose).not.toHaveBeenCalled();
  });

  // The real dialogs close themselves from a delegated onKeyDown, which Solid runs on
  // `document` before handleGlobalEsc (see delegateEvents above). The guard must still
  // see that a dialog owned this Escape.
  it.each([
    ['quit', 'Insert Link', LinkOverlay, isLinkDialogOpen, setIsLinkDialogOpen],
    ['quit', 'Insert Timestamp', TimestampOverlay, isTimestampDialogOpen, setIsTimestampDialogOpen],
    ['lock', 'Insert Link', LinkOverlay, isLinkDialogOpen, setIsLinkDialogOpen],
  ] as const)(
    'does not %s when Escape closes the real %s dialog',
    (action, _name, Overlay, isOpen, setOpen) => {
      setPreferences({ escAction: action });
      renderWithI18n(() => <MainLayout />);
      renderWithI18n(() => (
        <Overlay editor={dialogEditor} isOpen={isOpen()} onClose={() => setOpen(false)} />
      ));
      setOpen(true);

      const dialog = document.querySelector('[role="dialog"]')!;
      dialog.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }),
      );

      expect(isOpen()).toBe(false);
      expect(mockClose).not.toHaveBeenCalled();
      expect(authMocks.lockJournal).not.toHaveBeenCalled();
    },
  );

  describe('with escAction "lock"', () => {
    const pressEscape = () =>
      document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));

    beforeEach(() => {
      setPreferences({ escAction: 'lock' });
    });

    it('locks the journal on Escape and does not quit', () => {
      renderWithI18n(() => <MainLayout />);

      pressEscape();

      expect(authMocks.lockJournal).toHaveBeenCalledTimes(1);
      expect(mockClose).not.toHaveBeenCalled();
    });

    it('does not lock on Escape while Preferences is open', () => {
      setIsPreferencesOpen(true);
      renderWithI18n(() => <MainLayout />);

      pressEscape();

      expect(authMocks.lockJournal).not.toHaveBeenCalled();
    });

    it('minimizes the onboarding tour instead of locking', () => {
      startOnboarding();
      renderWithI18n(() => <MainLayout />);

      pressEscape();

      expect(onboardingMode()).toBe('minimized');
      expect(authMocks.lockJournal).not.toHaveBeenCalled();
    });

    it('does not lock again while a lock is already in progress', () => {
      authMocks.state = 'locking';
      renderWithI18n(() => <MainLayout />);

      pressEscape();

      expect(authMocks.lockJournal).not.toHaveBeenCalled();
    });

    it('locks only once when Escape repeats during the lock animation', () => {
      authMocks.lockJournal.mockImplementation(() => {
        authMocks.state = 'locking';
        return new Promise<void>(() => {});
      });
      renderWithI18n(() => <MainLayout />);

      pressEscape();
      pressEscape();

      expect(authMocks.lockJournal).toHaveBeenCalledTimes(1);
    });

    it('logs a failed lock instead of leaving an unhandled rejection', async () => {
      const failure = new Error('lock failed');
      authMocks.lockJournal.mockImplementation(() => Promise.reject(failure));
      renderWithI18n(() => <MainLayout />);

      pressEscape();

      await waitFor(() =>
        expect(mockLogError).toHaveBeenCalledWith('Failed to lock journal:', failure),
      );
    });
  });

  it('neither quits nor locks on Escape with escAction "none"', () => {
    setPreferences({ escAction: 'none' });
    renderWithI18n(() => <MainLayout />);

    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));

    expect(mockClose).not.toHaveBeenCalled();
    expect(authMocks.lockJournal).not.toHaveBeenCalled();
  });

  it('does not quit on Escape that cancels an IME composition', () => {
    setPreferences({ escAction: 'quit' });
    renderWithI18n(() => <MainLayout />);

    document.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Escape', isComposing: true, bubbles: true }),
    );

    expect(mockClose).not.toHaveBeenCalled();
  });

  it('still quits on Escape that only had preventDefault() called on it', () => {
    setPreferences({ escAction: 'quit' });
    renderWithI18n(() => <MainLayout />);

    const event = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true });
    event.preventDefault();
    document.dispatchEvent(event);

    expect(mockClose).toHaveBeenCalledTimes(1);
  });

  it('does not quit on Escape that a component claimed', () => {
    setPreferences({ escAction: 'quit' });
    renderWithI18n(() => <MainLayout />);

    const event = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true });
    claimEscape(event);
    document.dispatchEvent(event);

    expect(mockClose).not.toHaveBeenCalled();
  });

  describe('with a real editor body focused', () => {
    let editor: TiptapEditor | undefined;
    let host: HTMLDivElement | undefined;

    afterEach(() => {
      editor?.destroy();
      host?.remove();
      editor = undefined;
      host = undefined;
    });

    const pressEscapeInBody = (editable: boolean, escAction: 'quit' | 'lock' = 'quit') => {
      setPreferences({ escAction });
      renderWithI18n(() => <MainLayout />);
      host = document.createElement('div');
      document.body.appendChild(host);
      editor = new TiptapEditor({
        element: host,
        extensions: [StarterKit],
        content: '<p>x</p>',
        editable,
      });

      const event = new KeyboardEvent('keydown', {
        key: 'Escape',
        keyCode: 27,
        bubbles: true,
        cancelable: true,
      });
      editor.view.dom.dispatchEvent(event);
      return event;
    };

    // ProseMirror calls preventDefault() on every Escape in an editable view, so the
    // global action must not treat defaultPrevented as "a popup used this key".
    it('quits on Escape pressed in the entry body', () => {
      const event = pressEscapeInBody(true);

      expect(event.defaultPrevented).toBe(true);
      expect(mockClose).toHaveBeenCalledTimes(1);
    });

    it('quits on Escape pressed in the body of a locked (read-only) entry', () => {
      pressEscapeInBody(false);

      expect(mockClose).toHaveBeenCalledTimes(1);
    });

    it('locks on Escape pressed in the entry body', () => {
      pressEscapeInBody(true, 'lock');

      expect(authMocks.lockJournal).toHaveBeenCalledTimes(1);
      expect(mockClose).not.toHaveBeenCalled();
    });
  });

  it('does not open search on Ctrl+F while the header overflow menu is open', () => {
    setIsMoreMenuOpen(true);
    renderWithI18n(() => <MainLayout />);

    expect(isSearchOpen()).toBe(false);
    document.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'f', ctrlKey: true, bubbles: true }),
    );

    expect(isSearchOpen()).toBe(false);
  });
});

describe('MainLayout navigation wiring', () => {
  beforeEach(() => {
    resetUiState();
    resetPreferences();
    eventMocks.listeners.clear();
    eventMocks.navigatePreviousDay.mockReset();
    setEntryDates([]);
  });

  afterEach(() => {
    resetUiState();
    resetPreferences();
  });

  it('previous-entry-day shortcut navigates from the CURRENT selected date, not the initial one', async () => {
    setEntryDates(['2024-01-10', '2024-01-19', '2024-01-25']);

    renderWithI18n(() => <MainLayout />);

    // The user changes the selected date AFTER the shortcut handler was registered.
    setSelectedDate('2024-01-20');

    // Ctrl+[ — the handler must read selectedDate() at call time, not at mount.
    document.dispatchEvent(
      new KeyboardEvent('keydown', { key: '[', code: 'BracketLeft', ctrlKey: true, bubbles: true }),
    );

    await waitFor(() => expect(selectedDate()).toBe('2024-01-19'));
    expect(eventMocks.navigatePreviousDay).not.toHaveBeenCalled();
  });

  it('registers the surviving menu-preferences listener and opens Preferences', async () => {
    renderWithI18n(() => <MainLayout />);

    // onMount registers the listener asynchronously.
    await waitFor(() => expect(eventMocks.listeners.has('menu-preferences')).toBe(true));

    expect(isPreferencesOpen()).toBe(false);
    await eventMocks.listeners.get('menu-preferences')!(undefined);

    expect(isPreferencesOpen()).toBe(true);
  });
});

describe('MainLayout security notice', () => {
  afterEach(() => {
    authMocks.securityNotice = null;
    authMocks.dismissSecurityNotice.mockClear();
  });

  it('warns when the encryption upgrade failed and can be dismissed', async () => {
    authMocks.securityNotice = 'key-upgrade-failed';
    const { findByTestId } = renderWithI18n(() => <MainLayout />);

    const banner = await findByTestId('security-notice');
    expect(banner.getAttribute('role')).toBe('alert');
    expect(banner.textContent).toContain('could not upgrade');

    banner.querySelector('button')!.click();
    expect(authMocks.dismissSecurityNotice).toHaveBeenCalledTimes(1);
  });

  it('asks for all methods when the upgrade needs them', async () => {
    authMocks.securityNotice = 'upgrade-needs-all-methods';
    const { findByTestId } = renderWithI18n(() => <MainLayout />);

    const banner = await findByTestId('security-notice');
    expect(banner.textContent).toContain('All methods');
    expect(banner.textContent).toContain('quantum');
  });

  it('shows no warning without a notice', () => {
    const { queryByTestId } = renderWithI18n(() => <MainLayout />);
    expect(queryByTestId('security-notice')).toBeNull();
  });
});
