import { describe, expect, it, vi } from 'vitest';
import { renderGraphEdges } from './graph-render-kernel';
import type { GraphEdge, GraphNode } from '../../graph/graph-viewer-render';

class MockPen {
  public style: Record<string, unknown> | null = null;
  public moves: Array<{ x: number; y: number }> = [];
  public curves: Array<{ c1x: number; c1y: number; c2x: number; c2y: number; toX: number; toY: number }> = [];

  setStyle(style: Record<string, unknown>): void {
    this.style = style;
  }

  moveTo(x: number, y: number): void {
    this.moves.push({ x, y });
  }

  bezierCurveTo(c1x: number, c1y: number, c2x: number, c2y: number, toX: number, toY: number): void {
    this.curves.push({ c1x, c1y, c2x, c2y, toX, toY });
  }
}

function makeTableNode(renderHandle: number): GraphNode {
  return {
    renderHandle,
    kind: 'table',
    depth: 1,
    path: [{ tag: 0, key: 'influences', index: 0 }],
    boxArgs: { x: 294, y: 3292, width: 138, height: 1002, cornerRadius: 4 },
    meta: {
      text: 'influences',
      value: '[3600]',
      valueType: 'array',
      isIndex: false,
      path: [],
      editable: false,
      boxArgs: { x: 0, y: 0, width: 0, height: 0, cornerRadius: 0 },
      textArgs: { x: 0, y: 0, width: 0, height: 0, text: 'influences', textAlign: 'left', verticalAlign: 'middle', editable: false },
    },
    rows: [],
    table: {
      columns: [],
      rows: [],
      headerHeight: 0,
      totalHeight: 79200,
      viewHeight: 1000,
      rowHeight: 22,
    },
  };
}

function makeChildNode(renderHandle: number, y: number): GraphNode {
  return {
    renderHandle,
    kind: 'table',
    depth: 2,
    path: [{ tag: 0, key: 'influences', index: 0 }, { tag: 0, key: '', index: renderHandle }],
    boxArgs: { x: 764, y, width: 98, height: 46, cornerRadius: 4 },
    meta: {
      text: 'item',
      value: '[2]',
      valueType: 'array',
      isIndex: false,
      path: [],
      editable: false,
      boxArgs: { x: 0, y: 0, width: 0, height: 0, cornerRadius: 0 },
      textArgs: { x: 0, y: 0, width: 0, height: 0, text: 'item', textAlign: 'left', verticalAlign: 'middle', editable: false },
    },
    rows: [],
    table: {
      columns: [],
      rows: [],
      headerHeight: 0,
      totalHeight: 44,
      viewHeight: 44,
      rowHeight: 22,
    },
  };
}

function makeEdge(fromRow: number, toRenderHandle: number, fromY: number, toY: number): GraphEdge {
  return {
    fromRenderHandle: 5,
    fromRow,
    toRenderHandle,
    toRow: 0,
    bezierArgs: {
      fromX: 432,
      fromY,
      c1x: 492,
      c1y: fromY,
      c2x: 704,
      c2y: toY,
      toX: 764,
      toY,
    },
  };
}

function makeObjectNode(renderHandle: number, x: number, y: number, path: string[]): GraphNode {
  const width = renderHandle === 1 ? 306 : 338;
  return {
    renderHandle,
    kind: 'object',
    depth: path.length - 1,
    path: path.map((key) => ({ tag: 0, key, index: 0 })),
    boxArgs: { x, y, width, height: 156, cornerRadius: 8 },
    meta: null,
    rows: Array.from({ length: 6 }, (_, index) => ({
      index,
      boxArgs: { x, y: y + index * 24, width, height: 24, cornerRadius: 0 },
      cellBoxArgs: { x, y: y + index * 24, width, height: 24, cornerRadius: 0 },
      cells: [],
    })),
    table: null,
  };
}

describe('renderGraphEdges', () => {
  it('skips edges from table rows outside the visible runtime window', () => {
    const layer = { removeAll: vi.fn(), add: vi.fn() };
    const nodes = [makeTableNode(5), makeChildNode(50, 7956), makeChildNode(51, 8062)];
    const visibleEdge = makeEdge(45, 50, 4294, 7956);
    const offscreenEdge = makeEdge(46, 51, 4316, 8062);

    const rendered = renderGraphEdges({
      nodes,
      edges: [visibleEdge, offscreenEdge],
      layer,
      PenCtor: MockPen,
      renderConfig: { colors: { edge: '#999' } } as any,
      maxPerSource: 10,
      tableVisibleRanges: new Map([[5, { start: 0, end: 46 }]]),
    });

    expect(rendered).toEqual([visibleEdge]);
    expect(layer.add).toHaveBeenCalledTimes(1);
  });

  it('uses current node layout when rendering edges after incremental relayout', () => {
    const layer = { removeAll: vi.fn(), add: vi.fn() };
    const nodes = [
      makeObjectNode(1, 486, 0, ['root_step']),
      makeObjectNode(8, 1572, 2230, ['root_step', 'metrics_info']),
    ];
    const staleEdge: GraphEdge = {
      fromRenderHandle: 1,
      fromRow: 5,
      toRenderHandle: 8,
      toRow: 0,
      bezierArgs: {
        fromX: 792,
        fromY: 132,
        c1x: 954,
        c1y: 132,
        c2x: 954,
        c2y: 2242,
        toX: 1116,
        toY: 2242,
      },
    };

    const rendered = renderGraphEdges({
      nodes,
      edges: [staleEdge],
      layer,
      PenCtor: MockPen,
      renderConfig: { colors: { edge: '#999' } } as any,
      maxPerSource: null,
    });

    expect(rendered).toHaveLength(1);
    expect(rendered[0]?.bezierArgs.fromX).toBe(792);
    expect(rendered[0]?.bezierArgs.toX).toBe(1572);
    const pen = vi.mocked(layer.add).mock.calls[0]?.[0] as MockPen;
    expect(pen.moves[0]).toEqual({ x: 792, y: 132 });
    expect(pen.curves[0]?.toX).toBe(1572);
  });
});
