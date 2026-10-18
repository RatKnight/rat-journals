import { createSignal } from 'solid-js';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import * as tauri from '../lib/tauri';
import type { VaultListing, VaultStatus } from '../lib/tauri';
import { createLogger } from '../lib/logger';

const log = createLogger('Vault');

const EMPTY: VaultListing = { files: [], folders: [] };

const [vaultStatus, setVaultStatus] = createSignal<VaultStatus | null>(null);
const [vaultListing, setVaultListing] = createSignal<VaultListing>(EMPTY);
/** The "Open or create a vault" dialog. */
const [isVaultDialogOpen, setIsVaultDialogOpen] = createSignal(false);

export { vaultStatus, vaultListing, isVaultDialogOpen, setIsVaultDialogOpen };

/** True while a vault is unlocked (its window is shown). */
export const isVaultOpen = () => vaultStatus() !== null;

/** Clears everything the frontend holds about the vault. */
export function resetVaultState(): void {
  setVaultStatus(null);
  setVaultListing(EMPTY);
}

/** Reloads the file list from the backend. */
export async function refreshVault(): Promise<void> {
  if (!vaultStatus()) return;
  try {
    setVaultListing(await tauri.vaultList());
  } catch (err) {
    log.warn('Failed to list the vault:', err);
    // The backend closed the vault (for example a lock that arrived mid-operation).
    resetVaultState();
  }
}

/** Called after a successful create or unlock. */
export async function showOpenedVault(status: VaultStatus): Promise<void> {
  setVaultStatus(status);
  setIsVaultDialogOpen(false);
  await refreshVault();
}

/** Re-reads the unlock methods (after adding or removing one). */
export async function refreshVaultStatus(): Promise<void> {
  try {
    setVaultStatus(await tauri.vaultStatus());
  } catch (err) {
    log.warn('Failed to read the vault status:', err);
  }
}

/** Locks the vault (the journal is left as it is). */
export async function lockVault(): Promise<void> {
  try {
    await tauri.vaultLock();
  } finally {
    resetVaultState();
  }
}

/**
 * The backend closes the vault on every lock (Lock button, idle timer, OS screen lock,
 * focus loss) and announces it with `vault-locked`.
 */
export async function setupVaultEventListeners(): Promise<UnlistenFn> {
  const unlisten = await listen('vault-locked', () => resetVaultState());
  // A vault the backend still holds (for example after a WebView reload) is shown again.
  await refreshVaultStatus();
  await refreshVault();
  return unlisten;
}
