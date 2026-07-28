import { describe, expect, it } from 'vitest';
import { isExtensionMessage } from './messages';

describe('isExtensionMessage', () => {
  it('accepts a structurally complete content candidate', () => {
    expect(isExtensionMessage({ type: 'candidate', payload: {
      text: '{}', sourceTag: 'pre', domPath: 'main > pre', sourceLength: 2, pageTitle: 'Example', pageOrigin: 'https://example.test',
    }, openMode: 'user-gesture'
    })).toBe(true);
  });

  it('rejects malformed messages before they reach the Service Worker', () => {
    expect(isExtensionMessage({ type: 'candidate', payload: { text: '{}' } })).toBe(false);
    expect(isExtensionMessage({ type: 'candidate-too-large', payload: { sourceLength: 'huge' } })).toBe(false);
    expect(isExtensionMessage({ type: 'unknown' })).toBe(false);
  });
});
