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
/** Whether the vault (rather than the journal) is the view in front, while one is open. */
const [vaultInFront, setVaultInFront] = createSignal(true);
const [vaultClockWarning, setVaultClockWarning] = createSignal(false);

/** Whether to warn that the clock was set back since the vault was last open. */
export { vaultClockWarning };

export function dismissVaultClockWarning(): void {
  setVaultClockWarning(false);
}

export { vaultStatus, vaultListing, isVaultDialogOpen, setIsVaultDialogOpen };

/** True while a vault is unlocked. */
export const isVaultOpen = () => vaultStatus() !== null;

/** True when the vault window is the view on screen (open and not switched away from). */
export const isVaultShown = () => isVaultOpen() && vaultInFront();

/** Shows the journal side; an open vault stays unlocked in the background. */
export function showJournalView(): void {
  setVaultInFront(false);
}

/** Shows the open vault, or asks for one to open. */
export function showVaultView(): void {
  if (isVaultOpen()) setVaultInFront(true);
  else setIsVaultDialogOpen(true);
}

/** Clears everything the frontend holds about the vault. */
export function resetVaultState(): void {
  setVaultStatus(null);
  setVaultListing(EMPTY);
  setVaultClockWarning(false);
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
  setVaultClockWarning(status.clock_set_back === true);
  setVaultInFront(true);
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
