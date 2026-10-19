import { describe, it, expect, vi, beforeEach } from 'vitest';
import { screen, fireEvent } from '@solidjs/testing-library';
import { renderWithI18n } from '../../test/i18n-test-utils';
import UpdateCheck from './UpdateCheck';

const mocks = vi.hoisted(() => ({
  confirm: vi.fn(() => Promise.resolve(true)),
  checkForUpdate: vi.fn(),
  installUpdate: vi.fn(() => Promise.resolve()),
}));

vi.mock('../../lib/dialog', () => ({ confirm: mocks.confirm }));
vi.mock('../../lib/tauri', async () => ({
  ...(await vi.importActual<typeof import('../../lib/tauri')>('../../lib/tauri')),
  checkForUpdate: mocks.checkForUpdate,
  installUpdate: mocks.installUpdate,
}));

describe('UpdateCheck', () => {
  beforeEach(() => vi.clearAllMocks());

  it('does nothing until clicked', () => {
    renderWithI18n(() => <UpdateCheck />);
    expect(mocks.checkForUpdate).not.toHaveBeenCalled();
  });

  it('says when the app is up to date', async () => {
    mocks.checkForUpdate.mockResolvedValue(null);
    renderWithI18n(() => <UpdateCheck />);
    fireEvent.click(screen.getByTestId('update-check-button'));
    await screen.findByTestId('update-current');
  });

  it('offers a newer version and installs it after confirming', async () => {
    mocks.checkForUpdate.mockResolvedValue({ version: '26.2.0', notes: 'New things' });
    renderWithI18n(() => <UpdateCheck />);
    fireEvent.click(screen.getByTestId('update-check-button'));
    expect((await screen.findByTestId('update-available')).textContent).toContain('26.2.0');
    fireEvent.click(screen.getByTestId('update-install'));
    await vi.waitFor(() => expect(mocks.installUpdate).toHaveBeenCalledTimes(1));
    expect(mocks.confirm).toHaveBeenCalledTimes(1);
  });

  it('does not install when the confirmation is declined', async () => {
    mocks.checkForUpdate.mockResolvedValue({ version: '26.2.0', notes: null });
    mocks.confirm.mockResolvedValueOnce(false);
    renderWithI18n(() => <UpdateCheck />);
    fireEvent.click(screen.getByTestId('update-check-button'));
    fireEvent.click(await screen.findByTestId('update-install'));
    await vi.waitFor(() => expect(mocks.confirm).toHaveBeenCalledTimes(1));
    expect(mocks.installUpdate).not.toHaveBeenCalled();
  });

  it('shows a readable error when GitHub cannot be reached', async () => {
    mocks.checkForUpdate.mockRejectedValue('Could not check for updates: dns error');
    renderWithI18n(() => <UpdateCheck />);
    fireEvent.click(screen.getByTestId('update-check-button'));
    expect((await screen.findByRole('alert')).textContent).toContain('GitHub');
  });
});
