import { describe, expect, it } from "vitest";
import {
  computeProjection,
  createGraphModel,
} from '@treease/graph-viewer-runtime';
import type { GraphEdge, GraphNode } from '@treease/graph-viewer-runtime';

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
    bezierArgs: {
      fromX: 0,
      fromY: 0,
      c1x: 10,
      c1y: 0,
      c2x: 20,
      c2y: 0,
      toX: 30,
      toY: 0,
    },
  };
}

describe("computeProjection", () => {
  it("keeps small graphs complete", () => {
    const graph = { nodes: [node(1, 0), node(2, 10_000)], edges: [edge(1, 2)] };
    const projection = computeProjection(
      createGraphModel(1, graph.nodes, graph.edges),
      {
        viewport: { left: 0, right: 100, top: 0, bottom: 100 },
        viewportRevision: 1,
        materializationIntent: null,
        virtualizationThreshold: 2,
      },
    );
    expect(projection.nodeIds).toEqual(new Set([1, 2]));
    expect(projection.edgeKeys.size).toBe(1);
  });

  it("does not retain edges that are outside the viewport", () => {
    const graph = {
      nodes: [node(1, 0), node(2, 1_000), node(3, 2_000)],
      edges: [edge(1, 2), edge(2, 3)],
    };
    graph.edges.forEach((entry) => {
      entry.bezierArgs = {
        fromX: 1_000,
        fromY: 0,
        c1x: 1_100,
        c1y: 0,
        c2x: 1_200,
        c2y: 0,
        toX: 1_300,
        toY: 0,
      };
    });
    const projection = computeProjection(
      createGraphModel(1, graph.nodes, graph.edges),
      {
        viewport: { left: 0, right: 100, top: 0, bottom: 100 },
        viewportRevision: 1,
        materializationIntent: {
          revision: 1,
          targetNodeIds: [3],
          contextNodeIds: [],
          anchor: null,
        },
        overscan: 0,
        virtualizationThreshold: 1,
      },
    );
    expect(projection.nodeIds).toEqual(new Set([1, 3]));
    expect(projection.edgeKeys).toEqual(new Set());
  });

  it("keeps a Bezier edge that crosses the viewport even when both endpoint nodes are virtualized", () => {
    const crossing = edge(1, 2);
    crossing.bezierArgs = {
      fromX: -1_000,
      fromY: 50,
      c1x: -500,
      c1y: 50,
      c2x: 500,
      c2y: 50,
      toX: 1_000,
      toY: 50,
    };
    const projection = computeProjection(
      createGraphModel(1, [node(1, -1_000), node(2, 1_000)], [crossing]),
      {
        viewport: { left: -100, right: 100, top: 0, bottom: 100 },
        viewportRevision: 1,
        materializationIntent: null,
        overscan: 0,
        virtualizationThreshold: 1,
      },
    );

    expect(projection.nodeIds).toEqual(new Set());
    expect(projection.edgeKeys.size).toBe(1);
  });

  it("excludes incomplete layout nodes rather than reading renderer history", () => {
    const incomplete = node(1, 0);
    incomplete.boxArgs.x = Number.NaN;
    const projection = computeProjection(
      createGraphModel(1, [incomplete, node(2, 1_000)], []),
      {
        viewport: { left: 0, right: 100, top: 0, bottom: 100 },
        viewportRevision: 1,
        materializationIntent: null,
        overscan: 0,
        virtualizationThreshold: 1,
      },
    );
    expect(projection.nodeIds).toEqual(new Set());
  });
});
