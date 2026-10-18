import { createEffect, onCleanup } from 'solid-js';

const ACTIVITY_EVENTS = ['mousemove', 'keydown', 'click', 'touchstart', 'scroll'] as const;
const CHECK_INTERVAL_MS = 5_000;

interface Options {
  /** Minutes without using the vault before it locks; 0 turns this off. */
  minutes: () => number;
  /** True while a vault is unlocked. */
  isOpen: () => boolean;
  /** True while the vault is the view on screen (only then does activity count). */
  isShown: () => boolean;
  lock: () => void;
  now?: () => number;
}

/**
 * Locks the vault after a set time without using it, independently of the app-wide
 * auto-lock: the journal can stay open while the vault relocks. Activity counts only while
 * the vault is on screen, so working in the journal does not keep the vault unlocked.
 * Must be called inside a component (it registers listeners for the component's lifetime).
 */
export function createVaultIdleLock(opts: Options): void {
  const now = opts.now ?? (() => Date.now());
  let lastUsed = now();

  const onActivity = () => {
    if (opts.isShown()) lastUsed = now();
  };

  createEffect(() => {
    const minutes = opts.minutes();
    if (!opts.isOpen() || minutes <= 0) return;
    // A vault that has just opened (or a changed setting) starts a fresh period.
    lastUsed = now();
    ACTIVITY_EVENTS.forEach((e) => document.addEventListener(e, onActivity, { passive: true }));
    const timer = setInterval(() => {
      if (opts.isOpen() && now() - lastUsed >= minutes * 60_000) opts.lock();
    }, CHECK_INTERVAL_MS);
    onCleanup(() => {
      clearInterval(timer);
      ACTIVITY_EVENTS.forEach((e) => document.removeEventListener(e, onActivity));
    });
  });
}
