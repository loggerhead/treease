import { describe, expect, it } from 'vitest';

import { restoreShareResource } from './share-restore';

const interaction = { treePath: [], focus: null, columnNavigator: { activePath: [] } };

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
    }, { ...ports(calls), setViewMode: (mode) => calls.push(`mode.${mode}`), clearCompareState: () => calls.push('compare.clear'), restoreTreePath: () => true, restoreGraphFocus: async () => true, waitForGraphReady: async () => true, restoreColumnNavigator: async () => true, reportNavigationWarning: () => calls.push('warning') });
    expect(calls).toEqual(['editor.ready', 'editor.write', 'editor.idle', 'mode.text', 'right.write', 'compare', 'left.viewport', 'right.viewport']);
  });

  it('restores a left-only snapshot without starting compare or creating a right editor', async () => {
    const calls: string[] = [];
    await restoreShareResource({
      type: 'text_snapshot',
      payload: { schemaVersion: 1, left: { text: '{"left":1}', languageId: 'json' }, right: null, layout: { viewMode: 'graph', activePane: 'left' }, interaction },
    }, { ...ports(calls), setViewMode: (mode) => calls.push(`mode.${mode}`), clearCompareState: () => calls.push('compare.clear'), restoreTreePath: () => true, restoreGraphFocus: async () => true, waitForGraphReady: async () => true, restoreColumnNavigator: async () => true, reportNavigationWarning: () => calls.push('warning') });
    expect(calls).toEqual(['editor.ready', 'editor.write', 'editor.idle', 'compare.clear', 'mode.graph']);
  });

  it('restores the source selection before graph focus', async () => {
    const calls: string[] = [];
    await restoreShareResource({
      type: 'text_snapshot',
      payload: { schemaVersion: 1, left: { text: '{"left":1}', languageId: 'json' }, right: null, layout: { viewMode: 'graph', activePane: 'left' }, interaction: { treePath: [], focus: { type: 'graph', path: [{ type: 'key', key: 'left' }], target: 'value', editorSelection: { startLine: 1, startColumn: 2, endLine: 1, endColumn: 6 } }, columnNavigator: { activePath: [] } } },
    }, { ...ports(calls), setViewMode: (mode) => calls.push(`mode.${mode}`), clearCompareState: () => calls.push('compare.clear'), restoreTreePath: () => true, restoreGraphFocus: async () => { calls.push('graph.focus'); return true; }, waitForGraphReady: async () => { calls.push('graph.ready'); return true; }, restoreColumnNavigator: async () => true, reportNavigationWarning: () => calls.push('warning') });
    expect(calls).toEqual(['editor.ready', 'editor.write', 'editor.idle', 'compare.clear', 'mode.graph', 'selection', 'graph.ready', 'graph.focus']);
  });

  it('reports graph focus restoration when the runtime cannot resolve the target', async () => {
    const calls: string[] = [];
    await restoreShareResource({
      type: 'text_snapshot',
      payload: { schemaVersion: 1, left: { text: '{"left":1}', languageId: 'json' }, right: null, layout: { viewMode: 'graph', activePane: 'left' }, interaction: { treePath: [], focus: { type: 'graph', path: [{ type: 'key', key: 'missing' }], target: 'node', editorSelection: { startLine: 1, startColumn: 1, endLine: 1, endColumn: 1 } }, columnNavigator: { activePath: [] } } },
    }, { ...ports(calls), setViewMode: (mode) => calls.push(`mode.${mode}`), clearCompareState: () => calls.push('compare.clear'), restoreTreePath: () => true, restoreGraphFocus: async () => false, waitForGraphReady: async () => true, restoreColumnNavigator: async () => true, reportNavigationWarning: () => calls.push('warning') });
    expect(calls).toContain('warning');
  });
});
