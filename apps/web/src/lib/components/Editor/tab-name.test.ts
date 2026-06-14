import { describe, expect, it } from 'vitest';
import { createDefaultTabName } from './tab-name';

describe('createDefaultTabName', () => {
  it('uses sequential Tab labels for generated tabs', () => {
    expect(createDefaultTabName(1)).toBe('Tab 1');
    expect(createDefaultTabName(2)).toBe('Tab 2');
  });
});
