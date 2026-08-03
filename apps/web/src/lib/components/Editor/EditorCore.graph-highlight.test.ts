import { describe, expect, it } from 'vitest';
import type { TempModel } from '../../store/editor-store';
import {
  applyFailedTreePath,
  applyResolvedTreePath,
  editorDrivenCursorReasons,
  shouldSyncGraphHighlightFromCursorReason,
} from './EditorCore.graph-highlight';

function createTempModel(): TempModel {
  return {
    diffInputText: '',
    scratchText: '',
    commandQuery: '',
    status: 'Ready',
    error: '',
    cursor: 'Ln 1, Col 1',
    selectionLength: 0,
    treePath: [],
    graphHighlight: null,
    diagnostics: [],
  };
}

describe('EditorCore graph highlight sync', () => {
  it('does not create graphHighlight during warmup or tab activation style sync', () => {
    const current = createTempModel();
    const treePath = [{ tag: 1, key: 'root', index: 0 }] as any[];

    const next = applyResolvedTreePath(current, {
      treePath,
      target: 'value',
      revision: 12,
      syncGraphHighlight: false,
    });

    expect(next.treePath).toEqual(treePath);
    expect(next.graphHighlight).toBeNull();
  });

  it('preserves external reveal graphHighlight when a non-cursor sync clears treePath', () => {
    const current = {
      ...createTempModel(),
      treePath: [{ tag: 1, key: 'before', index: 0 }] as any[],
      graphHighlight: {
        path: [{ tag: 1, key: 'before', index: 0 }] as any[],
        target: 'key' as const,
        revision: 7,
        source: 'graph' as const,
      },
    };

    const next = applyFailedTreePath(current, false);

    expect(next.treePath).toEqual([]);
    expect(next.graphHighlight).toEqual(current.graphHighlight);
  });

  it('creates editor graphHighlight only for real cursor-driven sync', () => {
    const current = createTempModel();
    const treePath = [{ tag: 1, key: 'user', index: 0 }] as any[];

    const next = applyResolvedTreePath(current, {
      treePath,
      target: 'key',
      revision: 3,
      syncGraphHighlight: true,
    });

    expect(next.graphHighlight).toEqual({
      path: treePath,
      target: 'key',
      revision: 3,
      source: 'editor',
    });
  });

  it('updates the tree path while reveal sync is off without changing graphHighlight rules', () => {
    const current = {
      ...createTempModel(),
      treePath: [{ tag: 1, key: 'previous', index: 0 }] as any[],
    };
    const treePath = [{ tag: 1, key: 'current', index: 0 }] as any[];

    const next = applyResolvedTreePath(current, {
      treePath,
      target: 'value',
      revision: 4,
      syncGraphHighlight: true,
    });

    expect(next.treePath).toEqual(treePath);
    expect(next.graphHighlight).toEqual({
      path: treePath,
      target: 'value',
      revision: 4,
      source: 'editor',
    });
  });

  it('treats only explicit user cursor reasons as graphHighlight sync triggers', () => {
    expect(shouldSyncGraphHighlightFromCursorReason(0)).toBe(false);
    expect(shouldSyncGraphHighlightFromCursorReason(1)).toBe(false);
    expect(shouldSyncGraphHighlightFromCursorReason(editorDrivenCursorReasons.explicit)).toBe(true);
    expect(shouldSyncGraphHighlightFromCursorReason(editorDrivenCursorReasons.paste)).toBe(true);
    expect(shouldSyncGraphHighlightFromCursorReason(editorDrivenCursorReasons.undo)).toBe(true);
    expect(shouldSyncGraphHighlightFromCursorReason(editorDrivenCursorReasons.redo)).toBe(true);
  });
});
