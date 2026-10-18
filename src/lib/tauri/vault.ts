import { invoke } from '@tauri-apps/api/core';

// Encrypted file vault commands (backend: src-tauri/src/commands/vault.rs).

export interface VaultFile {
  id: string;
  name: string;
  folder: string | null;
  /** Plaintext size in bytes. */
  size: number;
  /** RFC 3339 timestamp. */
  added_at: string;
  sha256: string;
  mime: string | null;
}

export interface VaultSlotInfo {
  id: number;
  slot_type: 'password' | 'keyfile' | string;
  label: string;
  /** For key-file slots: identifies which key file opens it (not a secret). */
  slot_id?: string | null;
}

export interface VaultStatus {
  path: string;
  name: string;
  slots: VaultSlotInfo[];
}

export interface VaultListing {
  /** Newest first. */
  files: VaultFile[];
  folders: string[];
}

export interface VaultFolderInfo {
  is_vault: boolean;
  slots: VaultSlotInfo[];
}

/** The vault folder opened last time, ready to unlock. */
export interface VaultLastFolder {
  path: string;
  slots: VaultSlotInfo[];
}

export interface VaultAddResult {
  added: VaultFile[];
  failed: { name: string; error: string }[];
}

/** Largest file the in-app preview loads (matches the backend's MAX_PREVIEW_BYTES). */
export const VAULT_MAX_PREVIEW_BYTES = 200 * 1024 * 1024;

/** The vault folder opened last time, or null if none was saved or it is gone. */
export async function vaultLastFolder(): Promise<VaultLastFolder | null> {
  return await invoke('vault_last_folder');
}

export async function vaultCheckFolder(path: string): Promise<VaultFolderInfo> {
  return await invoke('vault_check_folder', { path });
}

export async function vaultCreate(
  path: string,
  password: string | null,
  keyFilePath: string | null,
): Promise<VaultStatus> {
  return await invoke('vault_create', { path, password, keyFilePath });
}

export async function vaultUnlockPassword(path: string, password: string): Promise<VaultStatus> {
  return await invoke('vault_unlock_password', { path, password });
}

export async function vaultUnlockKeyFile(path: string, keyPath: string): Promise<VaultStatus> {
  return await invoke('vault_unlock_key_file', { path, keyPath });
}

export async function vaultLock(): Promise<void> {
  await invoke('vault_lock');
}

export async function vaultStatus(): Promise<VaultStatus | null> {
  return (await invoke<VaultStatus | null>('vault_status')) ?? null;
}

export async function vaultList(): Promise<VaultListing> {
  return await invoke('vault_list');
}

export async function vaultAddFiles(
  paths: string[],
  folder: string | null,
): Promise<VaultAddResult> {
  return await invoke('vault_add_files', { paths, folder });
}

/** Largest file accepted by drag and drop (matches the backend's MAX_DROP_BYTES). */
export const VAULT_MAX_DROP_BYTES = 200 * 1024 * 1024;

/** UTF-8 text as base64, for request headers (which must be ASCII). */
function base64Utf8(text: string): string {
  let binary = '';
  for (const byte of new TextEncoder().encode(text)) binary += String.fromCharCode(byte);
  return btoa(binary);
}

/** Encrypts a dropped file's bytes into the vault. */
export async function vaultAddBytes(
  name: string,
  folder: string | null,
  bytes: ArrayBuffer,
): Promise<VaultFile> {
  const headers: Record<string, string> = { 'x-vault-name': base64Utf8(name) };
  if (folder) headers['x-vault-folder'] = base64Utf8(folder);
  return await invoke('vault_add_bytes', new Uint8Array(bytes), { headers });
}

/** Decrypted bytes for in-app preview (binary response). */
export async function vaultPreview(id: string): Promise<ArrayBuffer> {
  return await invoke('vault_preview', { id });
}

export async function vaultSaveCopy(id: string, dest: string): Promise<void> {
  await invoke('vault_save_copy', { id, dest });
}

/** Decrypts to the app's private temp folder and opens the default app; wiped on lock. */
export async function vaultOpenInApp(id: string): Promise<void> {
  await invoke('vault_open_in_app', { id });
}

export async function vaultRenameFile(id: string, name: string): Promise<void> {
  await invoke('vault_rename_file', { id, name });
}

export async function vaultMoveFile(id: string, folder: string | null): Promise<void> {
  await invoke('vault_move_file', { id, folder });
}

export async function vaultDeleteFile(id: string): Promise<void> {
  await invoke('vault_delete_file', { id });
}

export async function vaultCreateFolder(name: string): Promise<void> {
  await invoke('vault_create_folder', { name });
}

export async function vaultRenameFolder(oldName: string, newName: string): Promise<void> {
  await invoke('vault_rename_folder', { oldName, newName });
}

export async function vaultDeleteFolder(name: string): Promise<void> {
  await invoke('vault_delete_folder', { name });
}

/**
 * Changes the vault password. When a password already exists this rotates the master key
 * (re-encrypting the whole vault), so every registered key file must be given in
 * `keyFilePaths`; afterwards the old password opens nothing, not even old header copies.
 */
export async function vaultChangePassword(
  currentPassword: string | null,
  newPassword: string,
  keyFilePaths: string[] = [],
): Promise<void> {
  await invoke('vault_change_password', { currentPassword, newPassword, keyFilePaths });
}

/** Creates a new key file at `path` (must end in .key) and adds it as an unlock method. */
export async function vaultAddKeyFile(
  path: string,
  label: string,
  currentPassword: string | null,
): Promise<void> {
  await invoke('vault_add_key_file', { path, label, currentPassword });
}

/**
 * Removes an unlock method by rotating the master key, so the removed credential stops
 * working even against old header copies. Every key file that stays registered must be
 * given in `keyFilePaths`.
 */
export async function vaultRemoveUnlockMethod(
  slotId: number,
  currentPassword: string | null,
  keyFilePaths: string[] = [],
): Promise<void> {
  await invoke('vault_remove_unlock_method', { slotId, currentPassword, keyFilePaths });
}

/** Checks every file decrypts intact; returns the names of damaged files. */
export async function vaultVerify(): Promise<string[]> {
  return await invoke('vault_verify');
}
