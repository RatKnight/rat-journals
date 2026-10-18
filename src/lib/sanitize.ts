import DOMPurify from 'dompurify';

/**
 * Sanitizes entry HTML before it is placed in the page with `innerHTML` (the print/PDF
 * layer). Entry HTML can come from imports as well as the editor, so it is treated as
 * untrusted: scripts, event handlers, `javascript:` URLs, frames, forms, and embedded
 * objects are removed. Formatting, images (including `data:` images), links, and classes
 * stay. The CSP already blocks inline script; this removes the markup before it can matter.
 */
export function sanitizeEntryHtml(html: string): string {
  return DOMPurify.sanitize(html, {
    FORBID_TAGS: [
      'style',
      'form',
      'input',
      'button',
      'textarea',
      'select',
      'iframe',
      'frame',
      'object',
      'embed',
      'link',
      'meta',
      'base',
    ],
    // Inline `style` stays: the editor stores text colours there, and CSS cannot run script.
    FORBID_ATTR: ['srcset', 'action', 'formaction'],
  });
}

/** Search snippets may contain only `<mark>` around escaped text. */
export function sanitizeSnippet(html: string): string {
  return DOMPurify.sanitize(html, { ALLOWED_TAGS: ['mark'], ALLOWED_ATTR: [] });
}
