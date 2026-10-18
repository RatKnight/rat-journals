import { describe, it, expect } from 'vitest';
import { sanitizeEntryHtml, sanitizeSnippet } from './sanitize';

describe('sanitizeEntryHtml', () => {
  it('removes scripts, handlers, javascript: links, frames and forms', () => {
    const dirty =
      '<p>ok</p><script>alert(1)</script><img src="x" onerror="alert(2)">' +
      '<a href="javascript:alert(3)">link</a><iframe src="https://e.com"></iframe>' +
      '<form action="https://e.com"><input name="a"></form><object data="x"></object>';
    const clean = sanitizeEntryHtml(dirty);
    expect(clean).toContain('<p>ok</p>');
    for (const bad of [
      '<script',
      'onerror',
      'javascript:',
      '<iframe',
      '<form',
      '<input',
      '<object',
    ]) {
      expect(clean).not.toContain(bad);
    }
  });

  it('keeps formatting, colours, classes and data images', () => {
    const html =
      '<p class="x"><strong>b</strong> <span style="color: #ff0000">red</span></p>' +
      '<img src="data:image/png;base64,iVBORw0KGgo=" alt="i">';
    const clean = sanitizeEntryHtml(html);
    expect(clean).toContain('<strong>b</strong>');
    expect(clean).toContain('style="color: #ff0000"');
    expect(clean).toContain('class="x"');
    expect(clean).toContain('src="data:image/png;base64,iVBORw0KGgo="');
  });
});

describe('sanitizeSnippet', () => {
  it('allows only <mark>', () => {
    expect(sanitizeSnippet('a <mark>b</mark> <b onclick="x">c</b>')).toBe('a <mark>b</mark> c');
  });
});
