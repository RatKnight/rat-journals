import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import {
  vaultCheckFolder,
  vaultLastFolder,
  vaultCreate,
  vaultUnlockPassword,
  vaultUnlockKeyFile,
  vaultLock,
  vaultStatus,
  vaultList,
  vaultAddFiles,
  vaultPreview,
  vaultSaveCopy,
  vaultOpenInApp,
  vaultRenameFile,
  vaultMoveFile,
  vaultDeleteFile,
  vaultCreateFolder,
  vaultRenameFolder,
  vaultDeleteFolder,
  vaultChangePassword,
  vaultAddKeyFile,
  vaultRemoveUnlockMethod,
  vaultVerify,
  vaultAddBytes,
} from './vault';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const mockInvoke = vi.mocked(invoke);

describe('vault command wrappers (IPC contract)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockResolvedValue(undefined);
  });

  it.each([
    ['vaultCheckFolder', () => vaultCheckFolder('/v'), 'vault_check_folder', { path: '/v' }],
    [
      'vaultCreate',
      () => vaultCreate('/v', 'pw', '/k.key'),
      'vault_create',
      { path: '/v', password: 'pw', keyFilePath: '/k.key' },
    ],
    [
      'vaultUnlockPassword',
      () => vaultUnlockPassword('/v', 'pw'),
      'vault_unlock_password',
      { path: '/v', password: 'pw' },
    ],
    [
      'vaultUnlockKeyFile',
      () => vaultUnlockKeyFile('/v', '/k.key'),
      'vault_unlock_key_file',
      { path: '/v', keyPath: '/k.key' },
    ],
    [
      'vaultAddFiles',
      () => vaultAddFiles(['/a.pdf'], 'Medical'),
      'vault_add_files',
      { paths: ['/a.pdf'], folder: 'Medical' },
    ],
    ['vaultPreview', () => vaultPreview('id1'), 'vault_preview', { id: 'id1' }],
    [
      'vaultSaveCopy',
      () => vaultSaveCopy('id1', '/out.pdf'),
      'vault_save_copy',
      { id: 'id1', dest: '/out.pdf' },
    ],
    ['vaultOpenInApp', () => vaultOpenInApp('id1'), 'vault_open_in_app', { id: 'id1' }],
    [
      'vaultRenameFile',
      () => vaultRenameFile('id1', 'b.pdf'),
      'vault_rename_file',
      { id: 'id1', name: 'b.pdf' },
    ],
    [
      'vaultMoveFile',
      () => vaultMoveFile('id1', null),
      'vault_move_file',
      { id: 'id1', folder: null },
    ],
    ['vaultDeleteFile', () => vaultDeleteFile('id1'), 'vault_delete_file', { id: 'id1' }],
    ['vaultCreateFolder', () => vaultCreateFolder('A'), 'vault_create_folder', { name: 'A' }],
    [
      'vaultRenameFolder',
      () => vaultRenameFolder('A', 'B'),
      'vault_rename_folder',
      { oldName: 'A', newName: 'B' },
    ],
    ['vaultDeleteFolder', () => vaultDeleteFolder('A'), 'vault_delete_folder', { name: 'A' }],
    [
      'vaultChangePassword',
      () => vaultChangePassword(null, 'new', ['/a.key']),
      'vault_change_password',
      { currentPassword: null, newPassword: 'new', keyFilePaths: ['/a.key'] },
    ],
    [
      'vaultAddKeyFile',
      () => vaultAddKeyFile('/k.key', 'Backup', 'pw'),
      'vault_add_key_file',
      { path: '/k.key', label: 'Backup', currentPassword: 'pw' },
    ],
    [
      'vaultRemoveUnlockMethod',
      () => vaultRemoveUnlockMethod(2, 'pw'),
      'vault_remove_unlock_method',
      { slotId: 2, currentPassword: 'pw', keyFilePaths: [] },
    ],
  ])('%s → %s', async (_name, call, command, args) => {
    await call();
    expect(mockInvoke).toHaveBeenCalledWith(command, args);
  });

  it.each([
    ['vaultLock', () => vaultLock(), 'vault_lock'],
    ['vaultList', () => vaultList(), 'vault_list'],
    ['vaultVerify', () => vaultVerify(), 'vault_verify'],
    ['vaultLastFolder', () => vaultLastFolder(), 'vault_last_folder'],
  ])('%s → %s (no arguments)', async (_name, call, command) => {
    await call();
    expect(mockInvoke).toHaveBeenCalledWith(command);
  });

  it('vaultAddBytes sends raw bytes with base64 UTF-8 name and folder headers', async () => {
    const bytes = new Uint8Array([1, 2, 3]).buffer;
    await vaultAddBytes('Résumé.pdf', 'Médical', bytes);
    const [command, body, options] = mockInvoke.mock.calls[0];
    expect(command).toBe('vault_add_bytes');
    expect(Array.from(body as Uint8Array)).toEqual([1, 2, 3]);
    const headers = (options as { headers: Record<string, string> }).headers;
    const decode = (b64: string) =>
      new TextDecoder().decode(Uint8Array.from(atob(b64), (c) => c.charCodeAt(0)));
    expect(decode(headers['x-vault-name'])).toBe('Résumé.pdf');
    expect(decode(headers['x-vault-folder'])).toBe('Médical');
  });

  it('vaultAddBytes omits the folder header for unfiled files', async () => {
    await vaultAddBytes('a.txt', null, new ArrayBuffer(0));
    const options = mockInvoke.mock.calls[0][2] as { headers: Record<string, string> };
    expect(options.headers['x-vault-folder']).toBeUndefined();
  });

  it('vaultStatus returns null when no vault is open', async () => {
    mockInvoke.mockResolvedValueOnce(null);
    await expect(vaultStatus()).resolves.toBeNull();
    expect(mockInvoke).toHaveBeenCalledWith('vault_status');
  });
});
