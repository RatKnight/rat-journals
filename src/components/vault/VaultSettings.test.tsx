import { describe, it, expect, vi, beforeEach } from 'vitest';
import { screen, fireEvent } from '@solidjs/testing-library';
import { renderWithI18n } from '../../test/i18n-test-utils';
import type { VaultStatus } from '../../lib/tauri';

const mocks = vi.hoisted(() => ({
  status: { path: 'G:/Vault', name: 'Vault', slots: [] } as VaultStatus,
  open: vi.fn(),
  save: vi.fn(),
  vaultChangePassword: vi.fn(() => Promise.resolve()),
  vaultRemoveUnlockMethod: vi.fn(() => Promise.resolve()),
}));

vi.mock('../../state/vault', () => ({
  vaultStatus: () => mocks.status,
  refreshVaultStatus: vi.fn(() => Promise.resolve()),
}));
vi.mock('../../lib/dialog', () => ({ open: mocks.open, save: mocks.save }));
vi.mock('../../lib/tauri', async () => {
  const actual = await vi.importActual<typeof import('../../lib/tauri')>('../../lib/tauri');
  return {
    ...actual,
    vaultChangePassword: mocks.vaultChangePassword,
    vaultRemoveUnlockMethod: mocks.vaultRemoveUnlockMethod,
  };
});

import VaultSettings from './VaultSettings';

const password = { id: 1, slot_type: 'password', label: 'Password', slot_id: null };
const keyFile = (id: number, label: string) => ({
  id,
  slot_type: 'keyfile',
  label,
  slot_id: `sid-${id}`,
});

const render = () => renderWithI18n(() => <VaultSettings open onClose={() => {}} />);

const fillPasswords = (current: string | null, next: string) => {
  if (current !== null) {
    fireEvent.input(screen.getByLabelText('Current password'), { target: { value: current } });
  }
  fireEvent.input(screen.getByLabelText('New password'), { target: { value: next } });
  fireEvent.input(screen.getByLabelText('Confirm password'), { target: { value: next } });
};

describe('VaultSettings', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('changing the password asks for every registered key file', async () => {
    mocks.status = {
      path: 'G:/Vault',
      name: 'Vault',
      slots: [password, keyFile(2, 'Backup'), keyFile(3, 'Spare')],
    };
    mocks.open.mockResolvedValueOnce('E:/backup.key').mockResolvedValueOnce('E:/spare.key');
    render();
    fillPasswords('old', 'new password');
    fireEvent.click(screen.getByRole('button', { name: 'Change password' }));
    await vi.waitFor(() =>
      expect(mocks.vaultChangePassword).toHaveBeenCalledWith('old', 'new password', [
        'E:/backup.key',
        'E:/spare.key',
      ]),
    );
    expect(mocks.open).toHaveBeenCalledTimes(2);
    expect(mocks.open.mock.calls[0][0].title).toContain('Backup');
  });

  it('cancelling a key-file picker aborts the change', async () => {
    mocks.status = { path: 'G:/Vault', name: 'Vault', slots: [password, keyFile(2, 'Backup')] };
    mocks.open.mockResolvedValueOnce(null);
    render();
    fillPasswords('old', 'new password');
    fireEvent.click(screen.getByRole('button', { name: 'Change password' }));
    await vi.waitFor(() => expect(mocks.open).toHaveBeenCalled());
    expect(mocks.vaultChangePassword).not.toHaveBeenCalled();
  });

  it('adding a first password needs no key files', async () => {
    mocks.status = { path: 'G:/Vault', name: 'Vault', slots: [keyFile(1, 'Backup')] };
    render();
    fillPasswords(null, 'new password');
    fireEvent.click(screen.getByRole('button', { name: 'Add a password' }));
    await vi.waitFor(() =>
      expect(mocks.vaultChangePassword).toHaveBeenCalledWith(null, 'new password', []),
    );
    expect(mocks.open).not.toHaveBeenCalled();
  });

  it('removing a method asks for the key files that stay', async () => {
    mocks.status = {
      path: 'G:/Vault',
      name: 'Vault',
      slots: [password, keyFile(2, 'Backup'), keyFile(3, 'Spare')],
    };
    mocks.open.mockResolvedValueOnce('E:/spare.key');
    render();
    fireEvent.input(screen.getByLabelText('Current password'), { target: { value: 'pw' } });
    fireEvent.click(screen.getAllByRole('button', { name: 'Remove' })[1]);
    await vi.waitFor(() =>
      expect(mocks.vaultRemoveUnlockMethod).toHaveBeenCalledWith(2, 'pw', ['E:/spare.key']),
    );
    // Only the staying key file (Spare) is asked for, not the removed one.
    expect(mocks.open).toHaveBeenCalledTimes(1);
    expect(mocks.open.mock.calls[0][0].title).toContain('Spare');
  });
});
