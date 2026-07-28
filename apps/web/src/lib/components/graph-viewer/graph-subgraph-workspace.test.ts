import { describe, expect, it } from 'vitest';
import {
  normalizeWorkspaceGraphEdgeRows,
  rebaseSubgraphWorkspacePath,
  shouldOpenSubgraphWorkspaceContent,
  shouldIgnoreSubgraphOpenCell,
} from './graph-subgraph-workspace';
import type { GraphEdge, GraphNode } from '@treease/graph-viewer-runtime';
import type { PathSeg } from '../../store/tree-path';
import { PathSegTag } from '@core-wasm/index';

function makeHeaderTableNode(renderHandle: number): GraphNode {
  return {
    renderHandle,
    kind: 'table',
    depth: 1,
    path: [],
    boxArgs: { x: 200, y: 100, width: 260, height: 160, cornerRadius: 4 },
    meta: {
      text: 'steps',
      value: '[2]',
      valueType: 'array',
      isIndex: false,
      path: [],
      editable: false,
      boxArgs: { x: 0, y: 0, width: 0, height: 0, cornerRadius: 0 },
      textArgs: { x: 0, y: 0, width: 0, height: 0, text: 'steps', textAlign: 'left', verticalAlign: 'middle', editable: false },
    },
    rows: [],
    table: {
      columns: [],
      rows: [
        {
          boxArgs: { x: 200, y: 132, width: 260, height: 24, cornerRadius: 0 },
          cellBoxArgs: { x: 200, y: 132, width: 260, height: 24, cornerRadius: 0 },
          cells: [],
        },
        {
          boxArgs: { x: 200, y: 156, width: 260, height: 24, cornerRadius: 0 },
          cellBoxArgs: { x: 200, y: 156, width: 260, height: 24, cornerRadius: 0 },
          cells: [],
        },
      ],
      headerHeight: 32,
      totalHeight: 80,
      viewHeight: 80,
      rowHeight: 24,
    },
  };
}

function makeObjectNode(renderHandle: number): GraphNode {
  return {
    renderHandle,
    kind: 'object',
    depth: 2,
    path: [],
    boxArgs: { x: 520, y: 140, width: 180, height: 60, cornerRadius: 4 },
    meta: {
      text: 'child',
      value: '{2}',
      valueType: 'object',
      isIndex: false,
      path: [],
      editable: false,
      boxArgs: { x: 0, y: 0, width: 0, height: 0, cornerRadius: 0 },
      textArgs: { x: 0, y: 0, width: 0, height: 0, text: 'child', textAlign: 'left', verticalAlign: 'middle', editable: false },
    },
    rows: [],
  };
}

describe('normalizeWorkspaceGraphEdgeRows', () => {
  it('shifts workspace table edges by header offset when projection rows start at body index zero', () => {
    const nodes = [makeHeaderTableNode(10), makeObjectNode(11)];
    const edges: GraphEdge[] = [
      {
        fromRenderHandle: 10,
        fromRow: 0,
        toRenderHandle: 11,
        toRow: 0,
        bezierArgs: { fromX: 0, fromY: 0, c1x: 0, c1y: 0, c2x: 0, c2y: 0, toX: 0, toY: 0 },
      },
      {
        fromRenderHandle: 10,
        fromRow: 1,
        toRenderHandle: 11,
        toRow: 0,
        bezierArgs: { fromX: 0, fromY: 0, c1x: 0, c1y: 0, c2x: 0, c2y: 0, toX: 0, toY: 0 },
      },
    ];

    const normalized = normalizeWorkspaceGraphEdgeRows(nodes, edges);

    expect(normalized.map((edge) => edge.fromRow)).toEqual([1, 2]);
  });

  it('keeps already-offset table rows unchanged', () => {
    const nodes = [makeHeaderTableNode(10), makeObjectNode(11)];
    const edges: GraphEdge[] = [
      {
        fromRenderHandle: 10,
        fromRow: 1,
        toRenderHandle: 11,
        toRow: 0,
        bezierArgs: { fromX: 0, fromY: 0, c1x: 0, c1y: 0, c2x: 0, c2y: 0, toX: 0, toY: 0 },
      },
      {
        fromRenderHandle: 10,
        fromRow: 2,
        toRenderHandle: 11,
        toRow: 0,
        bezierArgs: { fromX: 0, fromY: 0, c1x: 0, c1y: 0, c2x: 0, c2y: 0, toX: 0, toY: 0 },
      },
    ];

    const normalized = normalizeWorkspaceGraphEdgeRows(nodes, edges);

    expect(normalized.map((edge) => edge.fromRow)).toEqual([1, 2]);
  });
});

function keySeg(key: string): PathSeg {
  return { tag: PathSegTag.KEY, key: key as any, index: 0 } as PathSeg;
}

function indexSeg(index: number): PathSeg {
  return { tag: PathSegTag.INDEX, key: '' as any, index } as PathSeg;
}

describe('rebaseSubgraphWorkspacePath', () => {
  it('rebases relative workspace cell paths onto the workspace root path', () => {
    expect(
      rebaseSubgraphWorkspacePath([keySeg('preview'), keySeg('uris')], [indexSeg(1)]),
    ).toEqual([keySeg('preview'), keySeg('uris'), indexSeg(1)]);
  });

  it('preserves absolute paths that already include the workspace root', () => {
    const absolutePath = [keySeg('preview'), keySeg('uris'), indexSeg(1)];
    expect(
      rebaseSubgraphWorkspacePath([keySeg('preview'), keySeg('uris')], absolutePath),
    ).toBe(absolutePath);
  });

  it('maps empty relative paths back to the workspace root path', () => {
    expect(rebaseSubgraphWorkspacePath([keySeg('preview')], [])).toEqual([keySeg('preview')]);
  });
});

describe('shouldIgnoreSubgraphOpenCell', () => {
  it('ignores miss placeholder cells for subgraph expansion', () => {
    expect(
      shouldIgnoreSubgraphOpenCell({
        text: 'miss',
        value: 'miss',
        isMissing: true,
        valueType: 'object',
        isIndex: false,
        path: [],
        editable: false,
        boxArgs: { x: 0, y: 0, width: 0, height: 0, cornerRadius: 0 },
        textArgs: { x: 0, y: 0, width: 0, height: 0, text: 'miss', textAlign: 'left', verticalAlign: 'middle', editable: false },
      }),
    ).toBe(true);
  });

  it('keeps real user values clickable even when their text is miss', () => {
    expect(
      shouldIgnoreSubgraphOpenCell({
        text: 'miss',
        value: 'miss',
        isMissing: false,
        valueType: 'string',
        isIndex: false,
        path: [keySeg('label')],
        editable: true,
        boxArgs: { x: 0, y: 0, width: 0, height: 0, cornerRadius: 0 },
        textArgs: { x: 0, y: 0, width: 0, height: 0, text: 'miss', textAlign: 'left', verticalAlign: 'middle', editable: true },
      }),
    ).toBe(false);
  });
});

describe('shouldOpenSubgraphWorkspaceContent', () => {
  it('keeps scalar values on the content-pane path', () => {
    expect(shouldOpenSubgraphWorkspaceContent({ valueType: 'number', displayText: '123' })).toBe(true);
    expect(shouldOpenSubgraphWorkspaceContent({ valueType: 'string', displayText: '"Alice"' })).toBe(true);
  });

  it('keeps non-empty containers on the graph-pane path', () => {
    expect(shouldOpenSubgraphWorkspaceContent({ valueType: 'object', displayText: '{2}' })).toBe(false);
    expect(shouldOpenSubgraphWorkspaceContent({ valueType: 'array', displayText: '[3]' })).toBe(false);
  });

  it('treats empty containers as single-cell content panes', () => {
    expect(shouldOpenSubgraphWorkspaceContent({ valueType: 'object', displayText: '{}' })).toBe(true);
    expect(shouldOpenSubgraphWorkspaceContent({ valueType: 'array', displayText: '[]' })).toBe(true);
  });
});
