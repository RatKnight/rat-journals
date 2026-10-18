import { describe, it, expect, vi, beforeEach } from 'vitest';
import { screen, fireEvent } from '@solidjs/testing-library';
import { renderWithI18n } from '../../test/i18n-test-utils';

const mocks = vi.hoisted(() => ({
  open: vi.fn(),
  save: vi.fn(),
  vaultCheckFolder: vi.fn(),
  vaultLastFolder: vi.fn(() => Promise.resolve(null as unknown)),
  vaultUnlockPassword: vi.fn(),
  vaultCreate: vi.fn(),
  showOpenedVault: vi.fn(() => Promise.resolve()),
}));

vi.mock('../../lib/dialog', () => ({ open: mocks.open, save: mocks.save }));
vi.mock('../../lib/tauri', async () => {
  const actual = await vi.importActual<typeof import('../../lib/tauri')>('../../lib/tauri');
  return {
    ...actual,
    vaultCheckFolder: mocks.vaultCheckFolder,
    vaultLastFolder: mocks.vaultLastFolder,
    vaultUnlockPassword: mocks.vaultUnlockPassword,
    vaultCreate: mocks.vaultCreate,
  };
});
vi.mock('../../state/vault', async () => {
  const actual = await vi.importActual<typeof import('../../state/vault')>('../../state/vault');
  return { ...actual, showOpenedVault: mocks.showOpenedVault };
});

import VaultOpenDialog from './VaultOpenDialog';
import { setIsVaultDialogOpen } from '../../state/vault';

const STATUS = { path: 'G:/Vault', name: 'Vault', slots: [] };

describe('VaultOpenDialog', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.vaultLastFolder.mockResolvedValue(null);
    setIsVaultDialogOpen(true);
  });

  it('goes straight to unlocking the vault opened last time', async () => {
    mocks.vaultLastFolder.mockResolvedValue({
      path: 'G:/My Drive/Vault',
      slots: [
        { id: 1, slot_type: 'password', label: 'Password' },
        { id: 2, slot_type: 'keyfile', label: 'USB' },
      ],
    });
    mocks.vaultUnlockPassword.mockResolvedValueOnce(STATUS);
    renderWithI18n(() => <VaultOpenDialog />);

    const pw = await screen.findByLabelText('Vault password');
    expect(screen.getByText('G:/My Drive/Vault')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Unlock with key file/ })).toBeInTheDocument();
    expect(mocks.open).not.toHaveBeenCalled();

    fireEvent.input(pw, { target: { value: 'secret' } });
    fireEvent.click(screen.getByRole('button', { name: 'Unlock vault' }));
    await vi.waitFor(() =>
      expect(mocks.vaultUnlockPassword).toHaveBeenCalledWith('G:/My Drive/Vault', 'secret'),
    );
  });

  it('can still choose a different folder after the remembered one is shown', async () => {
    mocks.vaultLastFolder.mockResolvedValue({
      path: 'G:/Old',
      slots: [{ id: 1, slot_type: 'password', label: 'Password' }],
    });
    renderWithI18n(() => <VaultOpenDialog />);
    await screen.findByLabelText('Vault password');
    fireEvent.click(screen.getByRole('button', { name: 'Choose a different folder' }));
    expect(await screen.findByRole('button', { name: /Choose folder/ })).toBeInTheDocument();
    expect(screen.queryByLabelText('Vault password')).toBeNull();
  });

  it('shows the folder picker when nothing is remembered', async () => {
    renderWithI18n(() => <VaultOpenDialog />);
    await vi.waitFor(() => expect(mocks.vaultLastFolder).toHaveBeenCalled());
    expect(screen.getByRole('button', { name: /Choose folder/ })).toBeInTheDocument();
  });

  it('unlocks an existing vault with its password', async () => {
    mocks.open.mockResolvedValueOnce('G:/Vault');
    mocks.vaultCheckFolder.mockResolvedValueOnce({
      is_vault: true,
      slots: [{ id: 1, slot_type: 'password', label: 'Password' }],
    });
    mocks.vaultUnlockPassword.mockResolvedValueOnce(STATUS);
    renderWithI18n(() => <VaultOpenDialog />);

    fireEvent.click(screen.getByRole('button', { name: /Choose folder/ }));
    const pw = await screen.findByLabelText('Vault password');
    expect(screen.queryByRole('button', { name: /key file/ })).toBeNull();
    fireEvent.input(pw, { target: { value: 'secret' } });
    fireEvent.click(screen.getByRole('button', { name: 'Unlock vault' }));

    await vi.waitFor(() =>
      expect(mocks.vaultUnlockPassword).toHaveBeenCalledWith('G:/Vault', 'secret'),
    );
    await vi.waitFor(() => expect(mocks.showOpenedVault).toHaveBeenCalledWith(STATUS));
  });

  it('shows the error and clears the password when unlocking fails', async () => {
    mocks.open.mockResolvedValueOnce('G:/Vault');
    mocks.vaultCheckFolder.mockResolvedValueOnce({
      is_vault: true,
      slots: [{ id: 1, slot_type: 'password', label: 'Password' }],
    });
    mocks.vaultUnlockPassword.mockRejectedValueOnce(
      'That password or key file does not open this vault',
    );
    renderWithI18n(() => <VaultOpenDialog />);
    fireEvent.click(screen.getByRole('button', { name: /Choose folder/ }));
    const pw = (await screen.findByLabelText('Vault password')) as HTMLInputElement;
    fireEvent.input(pw, { target: { value: 'wrong' } });
    fireEvent.click(screen.getByRole('button', { name: 'Unlock vault' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('does not open this vault');
    expect(pw.value).toBe('');
    expect(mocks.showOpenedVault).not.toHaveBeenCalled();
  });

  it('creates a vault in an empty folder and checks the confirmation', async () => {
    mocks.open.mockResolvedValueOnce('G:/New');
    mocks.vaultCheckFolder.mockResolvedValueOnce({ is_vault: false, slots: [] });
    mocks.vaultCreate.mockResolvedValueOnce(STATUS);
    renderWithI18n(() => <VaultOpenDialog />);
    fireEvent.click(screen.getByRole('button', { name: /Choose folder/ }));

    fireEvent.input(await screen.findByLabelText('New password'), { target: { value: 'a b c' } });
    fireEvent.input(screen.getByLabelText('Confirm password'), { target: { value: 'nope' } });
    fireEvent.click(screen.getByRole('button', { name: 'Create vault' }));
    expect(await screen.findByRole('alert')).toHaveTextContent("passwords don't match");
    expect(mocks.vaultCreate).not.toHaveBeenCalled();

    fireEvent.input(screen.getByLabelText('Confirm password'), { target: { value: 'a b c' } });
    fireEvent.click(screen.getByRole('button', { name: 'Create vault' }));
    await vi.waitFor(() => expect(mocks.vaultCreate).toHaveBeenCalledWith('G:/New', 'a b c', null));
  });

  it('can create a key-file-only vault', async () => {
    mocks.open.mockResolvedValueOnce('G:/New');
    mocks.vaultCheckFolder.mockResolvedValueOnce({ is_vault: false, slots: [] });
    mocks.save.mockResolvedValueOnce('E:/vault.key');
    mocks.vaultCreate.mockResolvedValueOnce(STATUS);
    renderWithI18n(() => <VaultOpenDialog />);
    fireEvent.click(screen.getByRole('button', { name: /Choose folder/ }));
    fireEvent.click(await screen.findByRole('checkbox'));
    fireEvent.click(screen.getByRole('button', { name: 'Create vault' }));
    await vi.waitFor(() =>
      expect(mocks.vaultCreate).toHaveBeenCalledWith('G:/New', null, 'E:/vault.key'),
    );
  });
});
