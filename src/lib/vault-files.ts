import type { VaultFile } from './tauri/vault';

/** Columns the vault file list can be sorted by. */
export type VaultSortKey = 'name' | 'folder' | 'size' | 'added';
export type VaultSortDir = 'asc' | 'desc';

/** `null` = all files; otherwise a folder name. */
export type VaultFolderFilter = string | null;

/** Human-readable size: 12 B, 340 KB, 1.4 MB, 2.1 GB. */
export function formatVaultSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value >= 10 || unit === 0 ? Math.round(value) : value.toFixed(1)} ${units[unit]}`;
}

/** "Today, 9:14", "Yesterday, 18:02", or a short date for older files. */
export function formatVaultAdded(iso: string, now: Date = new Date()): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  const time = date.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' });
  const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const days = Math.round((startOfDay(now) - startOfDay(date)) / 86_400_000);
  if (days === 0) return `Today, ${time}`;
  if (days === 1) return `Yesterday, ${time}`;
  return date.toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' });
}

/** Files in the selected folder (or all), whose name contains `query` (case-insensitive). */
export function filterVaultFiles(
  files: VaultFile[],
  folder: VaultFolderFilter,
  query: string,
): VaultFile[] {
  const q = query.trim().toLowerCase();
  return files.filter(
    (f) => (folder === null || f.folder === folder) && (!q || f.name.toLowerCase().includes(q)),
  );
}

/** A sorted copy. Ties fall back to newest first so the order is stable. */
export function sortVaultFiles(
  files: VaultFile[],
  key: VaultSortKey,
  dir: VaultSortDir,
): VaultFile[] {
  const sign = dir === 'asc' ? 1 : -1;
  const byAdded = (a: VaultFile, b: VaultFile) =>
    a.added_at < b.added_at ? -1 : a.added_at > b.added_at ? 1 : 0;
  const compare = (a: VaultFile, b: VaultFile): number => {
    switch (key) {
      case 'name':
        return a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' });
      case 'folder':
        return (a.folder ?? '').localeCompare(b.folder ?? '', undefined, { sensitivity: 'base' });
      case 'size':
        return a.size - b.size;
      case 'added':
        return byAdded(a, b);
    }
  };
  return [...files].sort((a, b) => sign * compare(a, b) || -byAdded(a, b));
}

/** How a file can be shown inside the app, if at all. */
export type VaultPreviewKind = 'image' | 'pdf' | 'text' | null;

export function vaultPreviewKind(file: Pick<VaultFile, 'mime'>): VaultPreviewKind {
  const mime = file.mime ?? '';
  if (/^image\/(png|jpeg|gif|webp|bmp)$/.test(mime)) return 'image';
  if (mime === 'application/pdf') return 'pdf';
  if (/^text\/(plain|markdown|csv)$/.test(mime) || mime === 'application/json') return 'text';
  return null;
}
