import { describe, it, expect, vi, beforeEach } from 'vitest';
import { screen, fireEvent, within } from '@solidjs/testing-library';
import { renderWithI18n } from '../../test/i18n-test-utils';
import type { VaultFile, VaultListing, VaultStatus } from '../../lib/tauri';

const mocks = vi.hoisted(() => ({
  listing: { files: [], folders: [] } as VaultListing,
  status: { path: 'G:/My Drive/Vault', name: 'Vault', slots: [] } as VaultStatus,
  lockVault: vi.fn(() => Promise.resolve()),
  refreshVault: vi.fn(() => Promise.resolve()),
  vaultDeleteFiles: vi.fn(() => Promise.resolve()),
  vaultMoveFiles: vi.fn(() => Promise.resolve()),
  vaultSaveCopies: vi.fn(() => Promise.resolve(2)),
  vaultPasteFiles: vi.fn(),
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
  return {
    ...actual,
    vaultDeleteFiles: mocks.vaultDeleteFiles,
    vaultMoveFiles: mocks.vaultMoveFiles,
    vaultSaveCopies: mocks.vaultSaveCopies,
    vaultPasteFiles: mocks.vaultPasteFiles,
    vaultAddFiles: mocks.vaultAddFiles,
  };
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

const rowNames = () => screen.getAllByTestId('vault-file-name').map((c) => c.textContent!.trim());

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
    await vi.waitFor(() => expect(mocks.vaultDeleteFiles).toHaveBeenCalledWith(['Bloodwork.pdf']));
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

  it('selects with checkboxes, Ctrl/Shift-click and select all, then deletes the selection', async () => {
    renderWithI18n(() => <VaultWindow />);
    const rows = () => screen.getAllByTestId('vault-file-row');
    const boxes = () => screen.getAllByTestId('vault-file-checkbox') as HTMLInputElement[];

    fireEvent.click(boxes()[0]);
    fireEvent.click(boxes()[2]);
    expect(screen.getByTestId('vault-selection-bar').textContent).toContain('2 selected');

    // A plain click selects only that row; Shift-click extends from it.
    fireEvent.click(rows()[1]);
    expect(boxes().map((b) => b.checked)).toEqual([false, true, false]);
    fireEvent.click(rows()[2], { shiftKey: true });
    expect(boxes().map((b) => b.checked)).toEqual([false, true, true]);
    fireEvent.click(rows()[0], { ctrlKey: true });
    expect(boxes().map((b) => b.checked)).toEqual([true, true, true]);

    const all = screen.getByTestId('vault-select-all') as HTMLInputElement;
    expect(all.checked).toBe(true);
    fireEvent.click(all);
    expect(screen.queryByTestId('vault-selection-bar')).toBeNull();

    fireEvent.keyDown(rows()[0], { key: 'a', ctrlKey: true });
    fireEvent.click(screen.getByTestId('vault-delete-selected'));
    await vi.waitFor(() =>
      expect(mocks.vaultDeleteFiles).toHaveBeenCalledWith([
        'Bloodwork.pdf',
        'notes.txt',
        'W-2.pdf',
      ]),
    );
    expect((mocks.confirm.mock.calls[0] as unknown[])[0]).toContain('Delete 3 files');
  });

  it('moves the selection and saves several copies into a chosen folder', async () => {
    mocks.open.mockResolvedValueOnce('D:/Out');
    renderWithI18n(() => <VaultWindow />);
    const boxes = () => screen.getAllByTestId('vault-file-checkbox');
    fireEvent.click(boxes()[0]);
    fireEvent.click(boxes()[1]);
    fireEvent.click(screen.getByTestId('vault-save-selected'));
    await vi.waitFor(() =>
      expect(mocks.vaultSaveCopies).toHaveBeenCalledWith(['Bloodwork.pdf', 'notes.txt'], 'D:/Out'),
    );
    await vi.waitFor(() =>
      expect(screen.getByTestId('vault-status').textContent).toBe('Saved 2 decrypted copies'),
    );
    fireEvent.click(screen.getByTestId('vault-move-selected'));
    expect(await screen.findByTestId('vault-move-dialog')).toBeInTheDocument();
  });

  it('pastes files copied in File Explorer with the button or Ctrl+V', async () => {
    mocks.vaultPasteFiles.mockResolvedValue({
      added: [file('x.pdf', '2026-10-18T10:00:00Z')],
      failed: [],
    });
    renderWithI18n(() => <VaultWindow />);
    fireEvent.click(screen.getByTestId('vault-paste'));
    await vi.waitFor(() => expect(mocks.vaultPasteFiles).toHaveBeenCalledWith(null));
    fireEvent.keyDown(screen.getAllByTestId('vault-file-row')[0], { key: 'v', ctrlKey: true });
    await vi.waitFor(() => expect(mocks.vaultPasteFiles).toHaveBeenCalledTimes(2));
    await vi.waitFor(() =>
      expect(screen.getByTestId('vault-status').textContent).toBe('Files added: 1'),
    );
  });
});
