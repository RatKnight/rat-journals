import { invoke } from '@tauri-apps/api/core';

// Authentication commands
export async function createJournal(password: string): Promise<void> {
  await invoke('create_diary', { password });
}

export async function unlockJournal(password: string): Promise<void> {
  await invoke('unlock_diary', { password });
}

export async function lockJournal(): Promise<void> {
  await invoke('lock_diary');
}

/** Takes the security notice left by the last unlock (e.g. `'key-upgrade-failed'`), or null. */
export async function takeSecurityNotice(): Promise<string | null> {
  return (await invoke<string | null>('take_security_notice')) ?? null;
}

export async function journalExists(): Promise<boolean> {
  return await invoke('diary_exists');
}

export async function checkJournalPath(path: string): Promise<boolean> {
  return invoke<boolean>('check_diary_path', { path });
}

export async function isJournalUnlocked(): Promise<boolean> {
  return await invoke('is_diary_unlocked');
}

export async function getJournalPath(): Promise<string> {
  return await invoke('get_diary_path');
}

export async function changeJournalDirectory(newDir: string, moveBackups: boolean): Promise<void> {
  await invoke('change_diary_directory', { newDir, moveBackups });
}

export async function changePassword(oldPassword: string, newPassword: string): Promise<void> {
  await invoke('change_password', { oldPassword, newPassword });
}

export async function resetJournal(): Promise<void> {
  await invoke('reset_diary');
}

export async function unlockJournalWithKeypair(keyPath: string): Promise<void> {
  await invoke('unlock_diary_with_keypair', { keyPath });
}

export async function verifyPassword(password: string): Promise<void> {
  await invoke('verify_password', { password });
}

// Auth method management
export interface AuthMethodInfo {
  id: number;
  slot_type: string;
  label: string;
  public_key_hex: string | null;
  created_at: string;
  last_used: string | null;
}

export interface AuthSlotPeek {
  id: number;
  slot_type: string;
  label: string;
}

export interface JournalPeek {
  slots: AuthSlotPeek[];
  require_all_auth: boolean;
  /** Legacy key wrapping remains; with several methods, finishing it needs all of them. */
  upgrade_pending?: boolean;
}

export async function peekAuthSlotTypes(): Promise<JournalPeek> {
  return await invoke('peek_auth_slot_types');
}

export type MultiAuthCredential =
  { type: 'password'; value: string } | { type: 'keypair'; key_path: string };

export async function listAuthMethods(): Promise<AuthMethodInfo[]> {
  return await invoke('list_auth_methods');
}

/**
 * Creates a key file at `path` (must end in `.key`) and registers it as an unlock method.
 * The secret is generated and written by the backend; it never reaches the frontend.
 */
export async function createKeyFile(
  path: string,
  label: string,
  currentPassword: string | null,
): Promise<void> {
  await invoke('create_key_file', { path, label, currentPassword });
}

export async function registerPassword(newPassword: string): Promise<void> {
  await invoke('register_password', { newPassword });
}

export async function removeAuthMethod(
  slotId: number,
  currentPassword: string | null,
): Promise<void> {
  await invoke('remove_auth_method', { slotId, currentPassword });
}

export async function unlockJournalAllMethods(credentials: MultiAuthCredential[]): Promise<void> {
  await invoke('unlock_diary_all_methods', { credentials });
}

/**
 * Turns "require all unlock methods" on or off. Both directions re-split the journal key,
 * so every credential is needed: the password (if the journal has one) and a path to each
 * registered key file.
 */
export async function setRequireAllAuth(
  enabled: boolean,
  password: string | null = null,
  keyFilePaths: string[] = [],
): Promise<void> {
  await invoke('set_require_all_auth', { enabled, password, keyFilePaths });
}

export async function createJournalAuto(): Promise<void> {
  await invoke('create_diary_auto');
}

export async function unlockJournalAuto(): Promise<void> {
  await invoke('unlock_diary_auto');
}
