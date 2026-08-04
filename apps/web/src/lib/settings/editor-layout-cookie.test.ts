import { describe, expect, it } from 'vitest';
import { readEditorSplitRatioCookie, readSidebarExpandedCookie } from './editor-layout-cookie';

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

  it('accepts only explicit sidebar boolean cookie values', () => {
    expect(readSidebarExpandedCookie('true')).toBe(true);
    expect(readSidebarExpandedCookie('false')).toBe(false);
    expect(readSidebarExpandedCookie(undefined)).toBeNull();
    expect(readSidebarExpandedCookie('1')).toBeNull();
  });
});
