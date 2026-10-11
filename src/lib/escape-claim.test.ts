import { describe, it, expect } from 'vitest';
import { claimEscape, isEscapeClaimed } from './escape-claim';

const escape = () => new KeyboardEvent('keydown', { key: 'Escape', cancelable: true });

describe('escape-claim', () => {
  it('reads an unclaimed event as not claimed, even when defaultPrevented', () => {
    const e = escape();
    e.preventDefault();
    expect(isEscapeClaimed(e)).toBe(false);
  });

  it('reads a claimed event as claimed', () => {
    const e = escape();
    claimEscape(e);
    expect(isEscapeClaimed(e)).toBe(true);
  });

  it('keeps claims per event', () => {
    const claimed = escape();
    const other = escape();
    claimEscape(claimed);
    expect(isEscapeClaimed(other)).toBe(false);
  });
});
