/**
 * Explicit "this Escape is handled" marks for non-modal popups (e.g. the tag dropdown).
 *
 * The global Escape action (MainLayout) cannot read `defaultPrevented` for this:
 * ProseMirror calls `preventDefault()` on every Escape in an editable editor, so that
 * flag is set even when no popup used the key. Only app code calls `claimEscape`.
 */
const claimed = new WeakSet<KeyboardEvent>();

export function claimEscape(e: KeyboardEvent): void {
  claimed.add(e);
}

export function isEscapeClaimed(e: KeyboardEvent): boolean {
  return claimed.has(e);
}
