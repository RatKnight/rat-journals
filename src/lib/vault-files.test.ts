import { describe, it, expect } from 'vitest';
import {
  formatVaultSize,
  formatVaultAdded,
  filterVaultFiles,
  sortVaultFiles,
  vaultPreviewKind,
} from './vault-files';
import type { VaultFile } from './tauri/vault';

const file = (over: Partial<VaultFile>): VaultFile => ({
  id: over.name ?? 'id',
  name: 'a.txt',
  folder: null,
  size: 1,
  added_at: '2026-01-01T00:00:00Z',
  sha256: '',
  mime: null,
  ...over,
});

describe('vault file helpers', () => {
  it('formats sizes', () => {
    expect(formatVaultSize(12)).toBe('12 B');
    expect(formatVaultSize(2048)).toBe('2 KB');
    expect(formatVaultSize(340 * 1024)).toBe('340 KB');
    expect(formatVaultSize(1.4 * 1024 * 1024)).toBe('1.4 MB');
    expect(formatVaultSize(3 * 1024 ** 3)).toBe('3.0 GB');
  });

  it('formats added dates relative to today', () => {
    const now = new Date(2026, 9, 18, 12, 0);
    expect(formatVaultAdded(new Date(2026, 9, 18, 9, 14).toISOString(), now)).toMatch(/^Today, /);
    expect(formatVaultAdded(new Date(2026, 9, 17, 9, 14).toISOString(), now)).toMatch(
      /^Yesterday, /,
    );
    expect(formatVaultAdded(new Date(2026, 0, 2).toISOString(), now)).not.toMatch(
      /Today|Yesterday/,
    );
    expect(formatVaultAdded('not a date', now)).toBe('not a date');
  });

  it('filters by folder and case-insensitive name', () => {
    const files = [
      file({ name: 'Bloodwork.pdf', folder: 'Medical' }),
      file({ name: 'W-2.pdf', folder: 'Taxes' }),
      file({ name: 'notes.txt' }),
    ];
    expect(filterVaultFiles(files, null, '').length).toBe(3);
    expect(filterVaultFiles(files, 'Medical', '').map((f) => f.name)).toEqual(['Bloodwork.pdf']);
    expect(filterVaultFiles(files, null, 'BLOOD').map((f) => f.name)).toEqual(['Bloodwork.pdf']);
    expect(filterVaultFiles(files, 'Taxes', 'blood')).toEqual([]);
  });

  it('sorts by each column, newest first on ties', () => {
    const a = file({ name: 'b.pdf', size: 5, added_at: '2026-01-01T00:00:00Z', folder: 'Z' });
    const b = file({ name: 'a.pdf', size: 9, added_at: '2026-03-01T00:00:00Z', folder: 'A' });
    const c = file({ name: 'c.pdf', size: 5, added_at: '2026-02-01T00:00:00Z', folder: null });
    const names = (list: VaultFile[]) => list.map((f) => f.name);
    expect(names(sortVaultFiles([a, b, c], 'added', 'desc'))).toEqual(['a.pdf', 'c.pdf', 'b.pdf']);
    expect(names(sortVaultFiles([a, b, c], 'name', 'asc'))).toEqual(['a.pdf', 'b.pdf', 'c.pdf']);
    expect(names(sortVaultFiles([a, b, c], 'size', 'asc'))).toEqual(['c.pdf', 'b.pdf', 'a.pdf']);
    expect(names(sortVaultFiles([a, b, c], 'folder', 'asc'))).toEqual(['c.pdf', 'a.pdf', 'b.pdf']);
  });

  it('decides what can be previewed in the app', () => {
    expect(vaultPreviewKind({ mime: 'image/jpeg' })).toBe('image');
    expect(vaultPreviewKind({ mime: 'application/pdf' })).toBe('pdf');
    expect(vaultPreviewKind({ mime: 'text/plain' })).toBe('text');
    expect(vaultPreviewKind({ mime: 'application/zip' })).toBeNull();
    expect(vaultPreviewKind({ mime: null })).toBeNull();
  });
});
