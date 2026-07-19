import { describe, expect, it } from 'vitest';

import { restoreShareResource } from './share-restore';

const interaction = { treePath: [], focus: null, subgraphWorkspace: { panePaths: [] } };

function ports(calls: string[]) {
  return {
    editor: {
      async ensureReady() { calls.push('editor.ready'); },
      async replaceActiveFromFile() { calls.push('editor.write'); },
      async waitForIdle() { calls.push('editor.idle'); },
      restoreViewportAnchor() { calls.push('left.viewport'); },
      restoreSelection() { calls.push('selection'); },
    },
    viewer: {
      async showTextPreview() { calls.push('right.write'); },
      async runCompare() { calls.push('compare'); },
      restoreViewportAnchor() { calls.push('right.viewport'); },
    },
  };
}

describe('share restore', () => {
  it('restores compare text, applies compare, then restores both viewport anchors', async () => {
    const calls: string[] = [];
    await restoreShareResource({
      type: 'compare',
      payload: { schemaVersion: 1, left: { text: '{"left":1}', languageId: 'json' }, right: { text: '{"right":2}', languageId: 'yaml' }, actions: [{ type: 'compare' }, { type: 'viewport_changed', payload: { left: { topLine: 24, scrollLeft: 0 }, right: { topLine: 31, scrollLeft: 0 } } }], interaction },
    }, { ...ports(calls), setViewMode: (mode) => calls.push(`mode.${mode}`), clearCompareState: () => calls.push('compare.clear'), restoreTreePath: () => true, restoreGraphFocus: () => true, rebuildSubgraphWorkspace: async () => true, reportNavigationWarning: () => calls.push('warning') });
    expect(calls).toEqual(['editor.ready', 'editor.write', 'editor.idle', 'mode.text', 'right.write', 'compare', 'left.viewport', 'right.viewport']);
  });

  it('restores a left-only snapshot without starting compare or creating a right editor', async () => {
    const calls: string[] = [];
    await restoreShareResource({
      type: 'text_snapshot',
      payload: { schemaVersion: 1, left: { text: '{"left":1}', languageId: 'json' }, right: null, layout: { viewMode: 'graph', activePane: 'left' }, interaction },
    }, { ...ports(calls), setViewMode: (mode) => calls.push(`mode.${mode}`), clearCompareState: () => calls.push('compare.clear'), restoreTreePath: () => true, restoreGraphFocus: () => true, rebuildSubgraphWorkspace: async () => true, reportNavigationWarning: () => calls.push('warning') });
    expect(calls).toEqual(['editor.ready', 'editor.write', 'editor.idle', 'compare.clear', 'mode.graph']);
  });
});
