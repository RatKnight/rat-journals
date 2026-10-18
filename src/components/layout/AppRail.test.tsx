import { describe, it, expect, vi, beforeEach } from 'vitest';
import { screen, fireEvent } from '@solidjs/testing-library';
import { renderWithI18n } from '../../test/i18n-test-utils';

const mocks = vi.hoisted(() => ({
  auth: 'unlocked' as string,
  vaultOpen: false,
  vaultShown: false,
  lockJournal: vi.fn(() => Promise.resolve()),
  lockVault: vi.fn(() => Promise.resolve()),
  showJournalView: vi.fn(),
  showVaultView: vi.fn(),
}));

vi.mock('../../state/auth', () => ({
  authState: () => mocks.auth,
  lockJournal: mocks.lockJournal,
}));
vi.mock('../../state/vault', () => ({
  isVaultOpen: () => mocks.vaultOpen,
  isVaultShown: () => mocks.vaultShown,
  lockVault: mocks.lockVault,
  showJournalView: mocks.showJournalView,
  showVaultView: mocks.showVaultView,
}));

import AppRail from './AppRail';

describe('AppRail', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.auth = 'unlocked';
    mocks.vaultOpen = false;
    mocks.vaultShown = false;
  });

  it('switches between the journal and the vault', () => {
    renderWithI18n(() => <AppRail />);
    expect(screen.getByTestId('rail-journal')).toHaveAttribute('aria-current', 'page');
    fireEvent.click(screen.getByTestId('rail-vault'));
    expect(mocks.showVaultView).toHaveBeenCalled();
    fireEvent.click(screen.getByTestId('rail-journal'));
    expect(mocks.showJournalView).toHaveBeenCalled();
  });

  it('marks the vault as current while it is shown, and flags it unlocked in the background', () => {
    mocks.vaultOpen = true;
    mocks.vaultShown = true;
    const { unmount } = renderWithI18n(() => <AppRail />);
    expect(screen.getByTestId('rail-vault')).toHaveAttribute('aria-current', 'page');
    unmount();
    mocks.vaultShown = false;
    renderWithI18n(() => <AppRail />);
    expect(screen.getByLabelText('Vault is unlocked')).toBeInTheDocument();
  });

  it('Lock locks the journal (which closes the vault), or the vault alone', () => {
    renderWithI18n(() => <AppRail />);
    fireEvent.click(screen.getByTestId('rail-lock'));
    expect(mocks.lockJournal).toHaveBeenCalled();
  });

  it('Lock with only a vault open locks the vault; nothing open hides Lock', () => {
    mocks.auth = 'locked';
    mocks.vaultOpen = true;
    const { unmount } = renderWithI18n(() => <AppRail />);
    fireEvent.click(screen.getByTestId('rail-lock'));
    expect(mocks.lockVault).toHaveBeenCalled();
    expect(mocks.lockJournal).not.toHaveBeenCalled();
    unmount();
    mocks.vaultOpen = false;
    renderWithI18n(() => <AppRail />);
    expect(screen.queryByTestId('rail-lock')).toBeNull();
  });
});
