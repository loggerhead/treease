import { describe, expect, it } from "vitest";
import { computeRenderableGraph } from "./graph-renderable-projection";
import type { GraphEdge, GraphNode } from "../../graph/graph-viewer-render";

function node(renderHandle: number, x: number, y = 0): GraphNode {
  return {
    renderHandle,
    kind: "scalar",
    depth: 0,
    boxArgs: { x, y, width: 40, height: 30, cornerRadius: 0 },
    path: [],
    meta: {} as GraphNode["meta"],
    rows: [],
  };
}

function edge(fromRenderHandle: number, toRenderHandle: number): GraphEdge {
  return {
    fromRenderHandle,
    fromRow: 0,
    toRenderHandle,
    toRow: 0,
    bezierArgs: { fromX: 0, fromY: 0, c1x: 10, c1y: 0, c2x: 20, c2y: 0, toX: 30, toY: 0 },
  };
}

describe("computeRenderableGraph", () => {
  it("keeps small graphs complete", () => {
    const graph = { nodes: [node(1, 0), node(2, 10_000)], edges: [edge(1, 2)] };
    const projection = computeRenderableGraph(graph, {
      viewport: { left: 0, right: 100, top: 0, bottom: 100 },
      virtualizationThreshold: 2,
    });
    expect(projection.nodes.map((entry) => entry.renderHandle)).toEqual([1, 2]);
    expect(projection.edges).toEqual(graph.edges);
  });

  it("projects visible nodes, preserves pins, and excludes incomplete edges", () => {
    const graph = {
      nodes: [node(1, 0), node(2, 1_000), node(3, 2_000)],
      edges: [edge(1, 2), edge(2, 3)],
    };
    const projection = computeRenderableGraph(graph, {
      viewport: { left: 0, right: 100, top: 0, bottom: 100 },
      overscan: 0,
      virtualizationThreshold: 1,
      pinnedRenderHandles: new Set([3]),
    });
    expect(projection.nodes.map((entry) => entry.renderHandle)).toEqual([1, 3]);
    expect(projection.edges).toEqual([]);
  });

  it("keeps a previously rendered node when a streamed layout update is incomplete", () => {
    const incomplete = node(1, 0);
    incomplete.boxArgs.x = Number.NaN;
    const projection = computeRenderableGraph(
      { nodes: [incomplete, node(2, 1_000)], edges: [] },
      {
        viewport: { left: 0, right: 100, top: 0, bottom: 100 },
        overscan: 0,
        virtualizationThreshold: 1,
        previousNodeIds: new Set([1]),
      },
    );
    expect(projection.nodeIds).toEqual(new Set([1]));
  });
});
