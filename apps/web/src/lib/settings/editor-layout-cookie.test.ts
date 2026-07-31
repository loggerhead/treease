import { describe, expect, it } from 'vitest';
import { readEditorSplitRatioCookie } from './editor-layout-cookie';

describe('editor-layout-cookie', () => {
  it('accepts and clamps a decimal split ratio', () => {
    expect(readEditorSplitRatioCookie('0.425')).toBe(0.425);
    expect(readEditorSplitRatioCookie('0.9')).toBe(0.8);
  });

  it('rejects malformed cookie values', () => {
    expect(readEditorSplitRatioCookie(undefined)).toBeNull();
    expect(readEditorSplitRatioCookie('42')).toBeNull();
    expect(readEditorSplitRatioCookie('0.4px')).toBeNull();
  });
});
