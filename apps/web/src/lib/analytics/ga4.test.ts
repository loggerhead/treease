import { describe, expect, it } from 'vitest';
import { sanitizePagePath } from './ga4';

describe('sanitizePagePath', () => {
  it('keeps only a normalized pathname', () => {
    expect(sanitizePagePath('/editor///')).toBe('/editor');
    expect(sanitizePagePath('editor')).toBe('/editor');
  });

  it('preserves the root path', () => {
    expect(sanitizePagePath('/')).toBe('/');
    expect(sanitizePagePath('')).toBe('/');
  });
});
