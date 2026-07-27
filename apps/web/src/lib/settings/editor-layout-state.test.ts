import { describe, expect, it } from 'vitest';
import {
  DEFAULT_EDITOR_SPLIT_RATIO,
  getEditorSplitRatio,
  mergeEditorLayoutState,
  normalizeEditorSplitRatio,
  omitEditorLayoutState,
  withEditorSplitRatio,
} from './editor-layout-state';

describe('editor-layout-state', () => {
  it('reads and clamps a persisted split ratio', () => {
    expect(getEditorSplitRatio(withEditorSplitRatio({}, 0.42))).toBe(0.42);
    expect(normalizeEditorSplitRatio(0.1)).toBe(0.2);
    expect(normalizeEditorSplitRatio(0.9)).toBe(0.8);
    expect(normalizeEditorSplitRatio('0.42')).toBeNull();
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

  it('uses the documented default split ratio', () => {
    expect(DEFAULT_EDITOR_SPLIT_RATIO).toBe(0.28);
  });
});
