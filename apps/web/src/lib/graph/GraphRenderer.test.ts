// Responsibility: unit tests for graph-edge-filter and graph-render-kernel.
import { describe, expect, it, vi } from 'vitest';
import { filterDenseOffscreenEdges } from '../components/graph-viewer/graph-edge-filter';
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

describe('graph edge filtering', () => {
  it('returns all edges when bounds cannot be resolved', () => {
    const nodes = [makeNode(1, 0, 0), makeNode(2, 200, 0)];
    const edges = [makeEdge(1, 0, 2, 100, 25)];
    expect(filterDenseOffscreenEdges(nodes, edges, null, null, 5)).toEqual(edges);
  });

  it('returns all edges when maxPerSource <= 0', () => {
    const nodes = [makeNode(1, 0, 0)];
    const edges = [makeEdge(1, 0, 1, 50, 25)];
    const container = { getBoundingClientRect: () => ({ width: 500, height: 500 }) } as any;
    const leafer = { zoomLayer: { scaleX: 1, scaleY: 1, x: 0, y: 0 } } as any;
    expect(filterDenseOffscreenEdges(nodes, edges, container, leafer, 0)).toEqual(edges);
  });

  it('keeps edges where from or to is visible', () => {
    const container = { getBoundingClientRect: () => ({ width: 500, height: 500 }) } as any;
    const leafer = { zoomLayer: { scaleX: 1, scaleY: 1, x: 0, y: 0 } } as any;
    const nodes = [makeNode(1, 50, 50), makeNode(2, 200, 200)];
    const edges = [makeEdge(1, 0, 2, 100, 75)];
    expect(filterDenseOffscreenEdges(nodes, edges, container, leafer, 5)).toHaveLength(1);
  });

  it('limits offscreen edges per source key', () => {
    const container = { getBoundingClientRect: () => ({ width: 100, height: 100 }) } as any;
    const leafer = { zoomLayer: { scaleX: 1, scaleY: 1, x: 0, y: 0 } } as any;
    const nodes = Array.from({ length: 6 }, (_, i) => makeNode(i + 10, 5000 + i * 200, 5000, 50, 50));
    nodes.push(makeNode(1, 5000, 5000, 50, 50));
    const edges = nodes.slice(0, 5).map((node) => makeEdge(1, 0, node.renderHandle, 5050, 5025));
    expect(filterDenseOffscreenEdges(nodes, edges, container, leafer, 2)).toHaveLength(2);
  });

  it('keeps edge with missing bezierArgs or missing target node', () => {
    const container = { getBoundingClientRect: () => ({ width: 100, height: 100 }) } as any;
    const leafer = { zoomLayer: { scaleX: 1, scaleY: 1, x: 0, y: 0 } } as any;
    const nodes = [makeNode(1, 5000, 5000)];
    const edgeNoBezier = { fromRenderHandle: 1, fromRow: 0, toRenderHandle: 999, toRow: 0, bezierArgs: undefined as any };
    expect(filterDenseOffscreenEdges(nodes, [edgeNoBezier], container, leafer, 5)).toHaveLength(1);
  });
});

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
