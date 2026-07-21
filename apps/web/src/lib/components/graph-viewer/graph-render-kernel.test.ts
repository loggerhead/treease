import { describe, expect, it, vi } from "vitest";
import { renderGraphEdges } from "./graph-render-kernel";
import type { GraphEdge, GraphNode } from "../../graph/graph-viewer-render";

class MockPen {
  public style: Record<string, unknown> | null = null;
  public moves: Array<{ x: number; y: number }> = [];
  public curves: Array<{
    c1x: number;
    c1y: number;
    c2x: number;
    c2y: number;
    toX: number;
    toY: number;
  }> = [];
  public remove = vi.fn();

  setStyle(style: Record<string, unknown>): void {
    this.style = style;
  }

  moveTo(x: number, y: number): void {
    this.moves.push({ x, y });
  }

  bezierCurveTo(
    c1x: number,
    c1y: number,
    c2x: number,
    c2y: number,
    toX: number,
    toY: number,
  ): void {
    this.curves.push({ c1x, c1y, c2x, c2y, toX, toY });
  }
}

function makeObjectNode(renderHandle: number, x: number, y: number, path: string[]): GraphNode {
  const width = renderHandle === 1 ? 306 : 338;
  return {
    renderHandle,
    kind: "object",
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

describe("renderGraphEdges", () => {
  it("preserves Core edge geometry instead of recalculating it from nodes", () => {
    const layer = { removeAll: vi.fn(), add: vi.fn() };
    const nodes = [
      makeObjectNode(1, 486, 0, ["root_step"]),
      makeObjectNode(8, 1572, 2230, ["root_step", "metrics_info"]),
    ];
    const coreEdge: GraphEdge = {
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
      edges: [coreEdge],
      layer,
      PenCtor: MockPen,
      renderConfig: { colors: { edge: "#999" } } as any,
    });

    expect(rendered).toHaveLength(1);
    expect(rendered[0]?.bezierArgs.fromX).toBe(792);
    expect(rendered[0]?.bezierArgs.toX).toBe(1116);
    const pen = vi.mocked(layer.add).mock.calls[0]?.[0] as MockPen;
    expect(pen.moves[0]).toEqual({ x: 792, y: 132 });
    expect(pen.curves[0]?.toX).toBe(1116);
  });

  it("reconciles projected edges without clearing the edge layer", () => {
    const layer = { removeAll: vi.fn(), add: vi.fn() };
    const nodes = [makeObjectNode(1, 0, 0, ["root"]), makeObjectNode(2, 500, 0, ["root", "child"])];
    const edge: GraphEdge = {
      fromRenderHandle: 1,
      fromRow: 0,
      toRenderHandle: 2,
      toRow: 0,
      bezierArgs: { fromX: 1, fromY: 2, c1x: 3, c1y: 4, c2x: 5, c2y: 6, toX: 7, toY: 8 },
    };
    const edgeRenderByKey = new Map<string, MockPen>();

    renderGraphEdges({
      nodes,
      edges: [edge],
      layer,
      PenCtor: MockPen,
      renderConfig: { colors: { edge: "#999" } } as any,
      edgeRenderByKey,
    });
    renderGraphEdges({
      nodes,
      edges: [edge],
      layer,
      PenCtor: MockPen,
      renderConfig: { colors: { edge: "#999" } } as any,
      edgeRenderByKey,
    });

    expect(layer.removeAll).not.toHaveBeenCalled();
    expect(layer.add).toHaveBeenCalledTimes(1);
    const pen = edgeRenderByKey.values().next().value as MockPen;
    renderGraphEdges({
      nodes,
      edges: [],
      layer,
      PenCtor: MockPen,
      renderConfig: { colors: { edge: "#999" } } as any,
      edgeRenderByKey,
    });
    expect(pen.remove).toHaveBeenCalledOnce();
    expect(edgeRenderByKey).toHaveLength(0);
  });

  it("draws a projected edge whose endpoint nodes are not materialized", () => {
    const layer = { removeAll: vi.fn(), add: vi.fn() };
    const projectedEdge: GraphEdge = {
      fromRenderHandle: 1,
      fromRow: 0,
      toRenderHandle: 2,
      toRow: 0,
      bezierArgs: { fromX: -100, fromY: 0, c1x: -50, c1y: 0, c2x: 50, c2y: 0, toX: 100, toY: 0 },
    };

    const rendered = renderGraphEdges({
      nodes: [],
      edges: [projectedEdge],
      layer,
      PenCtor: MockPen,
      renderConfig: { colors: { edge: "#999" } } as any,
    });

    expect(rendered).toEqual([projectedEdge]);
    expect(layer.add).toHaveBeenCalledOnce();
  });
});
