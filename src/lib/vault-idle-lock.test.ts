import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { createRoot, createSignal } from 'solid-js';
import { createVaultIdleLock } from './vault-idle-lock';

describe('createVaultIdleLock', () => {
  let clock = 0;
  beforeEach(() => {
    vi.useFakeTimers();
    clock = 0;
  });
  afterEach(() => vi.useRealTimers());

  const setup = (minutes: number, shown = true) =>
    createRoot((dispose) => {
      const [open, setOpen] = createSignal(true);
      const [isShown, setShown] = createSignal(shown);
      const lock = vi.fn(() => setOpen(false));
      createVaultIdleLock({
        minutes: () => minutes,
        isOpen: open,
        isShown,
        lock,
        now: () => clock,
      });
      return { lock, setShown, dispose };
    });

  const advance = (ms: number) => {
    clock += ms;
    vi.advanceTimersByTime(ms);
  };

  it('locks after the set minutes without vault activity', () => {
    const { lock, dispose } = setup(2);
    advance(60_000);
    document.dispatchEvent(new Event('keydown'));
    advance(90_000);
    expect(lock).not.toHaveBeenCalled(); // activity reset the period
    advance(35_000);
    expect(lock).toHaveBeenCalledTimes(1);
    dispose();
  });

  it('ignores activity while the journal is on screen', () => {
    const { lock, dispose } = setup(1, false);
    advance(30_000);
    document.dispatchEvent(new Event('mousemove'));
    advance(35_000);
    expect(lock).toHaveBeenCalled();
    dispose();
  });

  it('does nothing when set to 0', () => {
    const { lock, dispose } = setup(0);
    advance(10 * 60_000);
    expect(lock).not.toHaveBeenCalled();
    dispose();
  });
});
