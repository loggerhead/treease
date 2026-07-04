import { describe, expect, it } from 'vitest';
import type { TempModel } from '../store/editor-store';
import { clearGraphSelectionAfterEdit, clearGraphSelectionForFullEdit } from './GraphViewer.graph-highlight';

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

describe('GraphViewer graph highlight', () => {
  it('clears graph click selection after edit', () => {
    const path = [{ tag: 0, key: 'user', index: 0 }, { tag: 0, key: 'name', index: 0 }] as any[];
    const current: TempModel = {
      ...createTempModel(),
      treePath: path,
      graphHighlight: {
        path,
        target: 'value',
        revision: 3,
        source: 'graph',
      },
    };

    const next = clearGraphSelectionAfterEdit(current, path);

    expect(next.treePath).toEqual([]);
    expect(next.graphHighlight).toBeNull();
  });

  it('preserves non-graph highlight after edit', () => {
    const path = [{ tag: 0, key: 'user', index: 0 }] as any[];
    const current: TempModel = {
      ...createTempModel(),
      treePath: path,
      graphHighlight: {
        path,
        target: 'value',
        revision: 3,
        source: 'search',
      },
    };

    const next = clearGraphSelectionAfterEdit(current, path);

    expect(next).toEqual(current);
  });

  it('clears graph highlight when editing a different path', () => {
    const currentPath = [{ tag: 0, key: 'user', index: 0 }, { tag: 0, key: 'name', index: 0 }] as any[];
    const editPath = [{ tag: 0, key: 'user', index: 0 }, { tag: 0, key: 'role', index: 0 }] as any[];
    const current: TempModel = {
      ...createTempModel(),
      treePath: currentPath,
      graphHighlight: {
        path: currentPath,
        target: 'key',
        revision: 5,
        source: 'graph',
      },
    };

    const next = clearGraphSelectionAfterEdit(current, editPath);

    expect(next.treePath).toEqual([]);
    expect(next.graphHighlight).toBeNull();
  });

  it('clears any persisted tree selection before full edit replaces the document', () => {
    const path = [{ tag: 0, key: 'user', index: 0 }, { tag: 0, key: 'name', index: 0 }] as any[];
    const current: TempModel = {
      ...createTempModel(),
      treePath: path,
      graphHighlight: {
        path,
        target: 'value',
        revision: 6,
        source: 'search',
      },
    };

    const next = clearGraphSelectionForFullEdit(current);

    expect(next.treePath).toEqual([]);
    expect(next.graphHighlight).toBeNull();
  });
});
