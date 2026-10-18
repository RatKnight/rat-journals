import { describe, it, expect, vi, beforeEach } from 'vitest';
import { screen, fireEvent, within } from '@solidjs/testing-library';
import { renderWithI18n } from '../../test/i18n-test-utils';
import type { VaultFile, VaultListing, VaultStatus } from '../../lib/tauri';

const mocks = vi.hoisted(() => ({
  listing: { files: [], folders: [] } as VaultListing,
  status: { path: 'G:/My Drive/Vault', name: 'Vault', slots: [] } as VaultStatus,
  lockVault: vi.fn(() => Promise.resolve()),
  refreshVault: vi.fn(() => Promise.resolve()),
  vaultDeleteFile: vi.fn(() => Promise.resolve()),
  vaultAddFiles: vi.fn(),
  open: vi.fn(),
  save: vi.fn(),
  confirm: vi.fn(() => Promise.resolve(true)),
}));

vi.mock('../../state/vault', () => ({
  vaultListing: () => mocks.listing,
  vaultStatus: () => mocks.status,
  lockVault: mocks.lockVault,
  refreshVault: mocks.refreshVault,
  refreshVaultStatus: vi.fn(),
}));

vi.mock('../../lib/dialog', () => ({ open: mocks.open, save: mocks.save, confirm: mocks.confirm }));

vi.mock('../../lib/tauri', async () => {
  const actual = await vi.importActual<typeof import('../../lib/tauri')>('../../lib/tauri');
  return { ...actual, vaultDeleteFile: mocks.vaultDeleteFile, vaultAddFiles: mocks.vaultAddFiles };
});

import VaultWindow from './VaultWindow';

const file = (
  name: string,
  added: string,
  folder: string | null = null,
  size = 100,
): VaultFile => ({
  id: name,
  name,
  folder,
  size,
  added_at: added,
  sha256: '',
  mime: name.endsWith('.pdf') ? 'application/pdf' : null,
});

const rowNames = () =>
  screen.getAllByTestId('vault-file-row').map((r) => r.querySelector('td')!.textContent!.trim());

describe('VaultWindow', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.listing = {
      folders: ['Medical', 'Taxes'],
      files: [
        file('W-2.pdf', '2026-02-02T10:00:00Z', 'Taxes', 5000),
        file('Bloodwork.pdf', '2026-10-18T09:14:00Z', 'Medical', 400),
        file('notes.txt', '2026-05-01T08:00:00Z', null, 20),
      ],
    };
  });

  it('lists all files newest first by default', () => {
    renderWithI18n(() => <VaultWindow />);
    expect(rowNames()).toEqual(['Bloodwork.pdf', 'notes.txt', 'W-2.pdf']);
    expect(screen.getByText('Files: 3')).toBeInTheDocument();
  });

  it('sorts by a column and toggles direction', () => {
    renderWithI18n(() => <VaultWindow />);
    fireEvent.click(screen.getByRole('button', { name: /^Name/ }));
    expect(rowNames()).toEqual(['Bloodwork.pdf', 'notes.txt', 'W-2.pdf']);
    fireEvent.click(screen.getByRole('button', { name: /^Name/ }));
    expect(rowNames()).toEqual(['W-2.pdf', 'notes.txt', 'Bloodwork.pdf']);
    fireEvent.click(screen.getByRole('button', { name: /^Size/ }));
    expect(rowNames()).toEqual(['W-2.pdf', 'Bloodwork.pdf', 'notes.txt']);
  });

  it('filters by folder and by search text', () => {
    renderWithI18n(() => <VaultWindow />);
    const nav = screen.getByRole('navigation');
    fireEvent.click(within(nav).getByRole('button', { name: /Medical/ }));
    expect(rowNames()).toEqual(['Bloodwork.pdf']);
    fireEvent.click(screen.getByTestId('vault-all-files'));
    fireEvent.input(screen.getByTestId('vault-search'), { target: { value: 'NOTES' } });
    expect(rowNames()).toEqual(['notes.txt']);
  });

  it('shows the empty-vault invitation', () => {
    mocks.listing = { files: [], folders: [] };
    renderWithI18n(() => <VaultWindow />);
    expect(screen.getByText(/This vault is empty/)).toBeInTheDocument();
  });

  it('locks the vault from the toolbar', () => {
    renderWithI18n(() => <VaultWindow />);
    fireEvent.click(screen.getByTestId('vault-lock'));
    expect(mocks.lockVault).toHaveBeenCalled();
  });

  it('deletes the selected file with the Delete key after confirming', async () => {
    renderWithI18n(() => <VaultWindow />);
    const row = screen.getAllByTestId('vault-file-row')[0];
    fireEvent.click(row);
    fireEvent.keyDown(row, { key: 'Delete' });
    await vi.waitFor(() => expect(mocks.vaultDeleteFile).toHaveBeenCalledWith('Bloodwork.pdf'));
    expect(mocks.confirm).toHaveBeenCalled();
  });

  it('adds picked files into the selected folder and reports failures', async () => {
    mocks.open.mockResolvedValueOnce(['C:/scan.pdf', 'C:/folder']);
    mocks.vaultAddFiles.mockResolvedValueOnce({
      added: [file('scan.pdf', '2026-10-18T10:00:00Z', 'Medical')],
      failed: [{ name: 'folder', error: 'Only files can be added (not folders)' }],
    });
    renderWithI18n(() => <VaultWindow />);
    fireEvent.click(
      within(screen.getByRole('navigation')).getByRole('button', { name: /Medical/ }),
    );
    fireEvent.click(screen.getByTestId('vault-add-files'));
    await vi.waitFor(() =>
      expect(mocks.vaultAddFiles).toHaveBeenCalledWith(['C:/scan.pdf', 'C:/folder'], 'Medical'),
    );
    await vi.waitFor(() =>
      expect(screen.getByRole('alert').textContent).toContain('folder: Only files can be added'),
    );
    expect(screen.getByTestId('vault-status').textContent).toBe('Files added: 1');
  });
});
