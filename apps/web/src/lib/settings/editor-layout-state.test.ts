import { describe, expect, it } from 'vitest';
import {
  getColumnNavigatorHeight,
  getEditorSplitRatio,
  mergeEditorLayoutState,
  normalizeColumnNavigatorHeight,
  normalizeEditorSplitRatio,
  omitEditorLayoutState,
  withColumnNavigatorHeight,
  withEditorSplitRatio,
} from './editor-layout-state';

describe('editor-layout-state', () => {
  it('reads and clamps a persisted split ratio', () => {
    expect(getEditorSplitRatio(withEditorSplitRatio({}, 0.42))).toBe(0.42);
    expect(normalizeEditorSplitRatio(0.1)).toBe(0.2);
    expect(normalizeEditorSplitRatio(0.9)).toBe(0.8);
    expect(normalizeEditorSplitRatio('0.42')).toBeNull();
  });

  it('reads and clamps a persisted column navigator height', () => {
    expect(getColumnNavigatorHeight(withColumnNavigatorHeight({}, 320))).toBe(320);
    expect(normalizeColumnNavigatorHeight(80)).toBe(100);
    expect(normalizeColumnNavigatorHeight('320')).toBeNull();
  });

  it('keeps editor layout state out of the Settings dialog document', () => {
    const document = withEditorSplitRatio({ parser: { enableNest: true } }, 0.42);
    expect(omitEditorLayoutState(document)).toEqual({ parser: { enableNest: true } });
  });

  it('preserves hidden layout state when a Settings dialog document is saved', () => {
    const existing = withEditorSplitRatio({}, 0.42);
    expect(mergeEditorLayoutState({ parser: { enableNest: false } }, existing)).toEqual({
      parser: { enableNest: false },
      __treeaseEditorLayout: { splitRatio: 0.42 },
    });
  });

});
