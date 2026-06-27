import { describe, expect, it } from 'vitest';
import { normalizeWorkspaceGraphEdgeRows } from './graph-subgraph-workspace';
import type { GraphEdge, GraphNode } from '../../graph/graph-viewer-render';

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
