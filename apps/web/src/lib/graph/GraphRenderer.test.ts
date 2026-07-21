// Responsibility: unit tests for graph-render-kernel.
import { describe, expect, it, vi } from 'vitest';
import { renderGraphEdges } from '../components/graph-viewer/graph-render-kernel';
import type { GraphEdge, GraphNode } from './graph-viewer-render';

function makeNode(renderHandle: number, x: number, y: number, w = 100, h = 50): GraphNode {
  return {
    renderHandle,
    kind: 'object',
    depth: 0,
    boxArgs: { x, y, width: w, height: h, cornerRadius: 4 },
    path: [],
    meta: {} as any,
    rows: [],
  };
}

function makeEdge(fromRenderHandle: number, fromRow: number, toRenderHandle: number, fx: number, fy: number): GraphEdge {
  return {
    fromRenderHandle,
    fromRow,
    toRenderHandle,
    toRow: 0,
    bezierArgs: { fromX: fx, fromY: fy, c1x: fx + 10, c1y: fy, c2x: fx + 20, c2y: fy, toX: fx + 30, toY: fy },
  };
}

function createMockPenCtor() {
  const pens: any[] = [];
  function MockPen(this: any) {
    this.setStyle = vi.fn();
    this.moveTo = vi.fn();
    this.bezierCurveTo = vi.fn();
    pens.push(this);
  }
  (MockPen as any).pens = pens;
  return MockPen as any;
}

describe('renderGraphEdges', () => {
  it('returns empty array when layer is null', () => {
    const PenCtor = createMockPenCtor();
    expect(renderGraphEdges({ nodes: [], edges: [], layer: null, PenCtor, renderConfig: {} as any })).toEqual([]);
  });

  it('draws bezier curves with configured edge color', () => {
    const PenCtor = createMockPenCtor();
    const layer = { removeAll: vi.fn(), add: vi.fn() };
    const nodes = [makeNode(1, 0, 0), makeNode(2, 200, 0)];
    const edge = makeEdge(1, 0, 2, 100, 25);
    const result = renderGraphEdges({ nodes, edges: [edge], layer, PenCtor, renderConfig: { colors: { edge: '#ccc' } } as any });
    expect(result).toEqual([edge]);
    expect(layer.removeAll).toHaveBeenCalledWith(true);
    expect(PenCtor.pens[0].setStyle).toHaveBeenCalledWith({ stroke: '#ccc', strokeWidth: 1 });
    expect(PenCtor.pens[0].moveTo).toHaveBeenCalledWith(100, 25);
    expect(PenCtor.pens[0].bezierCurveTo).toHaveBeenCalledWith(110, 25, 120, 25, 130, 25);
    expect(layer.add).toHaveBeenCalledWith(PenCtor.pens[0]);
  });

  it('skips edges where endpoints are missing from node map', () => {
    const PenCtor = createMockPenCtor();
    const layer = { removeAll: vi.fn(), add: vi.fn() };
    renderGraphEdges({
      nodes: [makeNode(1, 0, 0)],
      edges: [makeEdge(1, 0, 999, 50, 25)],
      layer,
      PenCtor,
      renderConfig: { colors: { edge: '#ccc' } } as any,
    });
    expect(PenCtor.pens).toHaveLength(0);
    expect(layer.add).not.toHaveBeenCalled();
  });
});
