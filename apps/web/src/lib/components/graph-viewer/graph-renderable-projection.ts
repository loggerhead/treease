// Responsibility: derive the Leafer scene subset from the complete graph and world viewport.
import type { GraphEdge, GraphNode } from "../../graph/graph-viewer-render";
import { doesBoxIntersectBounds, type ViewportBounds } from "./graph-viewport-geometry";

export type GraphRenderableProjection = {
  nodes: GraphNode[];
  edges: GraphEdge[];
  nodeIds: Set<number>;
  edgeKeys: Set<string>;
  changed: boolean;
};

export type GraphRenderableProjectionOptions = {
  viewport: ViewportBounds | null;
  previousNodeIds?: ReadonlySet<number>;
  previousEdgeKeys?: ReadonlySet<string>;
  pinnedRenderHandles?: ReadonlySet<number>;
  virtualizationThreshold?: number;
  overscan?: number;
};

const DEFAULT_VIRTUALIZATION_THRESHOLD = 100;
const DEFAULT_OVERSCAN = 200;

export function graphEdgeKey(edge: GraphEdge): string {
  const curve = edge.bezierArgs;
  return [
    edge.fromRenderHandle,
    edge.fromRow,
    edge.toRenderHandle,
    edge.toRow,
    curve.fromX,
    curve.fromY,
    curve.c1x,
    curve.c1y,
    curve.c2x,
    curve.c2y,
    curve.toX,
    curve.toY,
  ].join(":");
}

function expandViewport(viewport: ViewportBounds, overscan: number): ViewportBounds {
  return {
    left: viewport.left - overscan,
    right: viewport.right + overscan,
    top: viewport.top - overscan,
    bottom: viewport.bottom + overscan,
  };
}

function hasRenderableBounds(node: GraphNode): boolean {
  const { x, y, width, height } = node.boxArgs;
  return [x, y, width, height].every(Number.isFinite) && width >= 0 && height >= 0;
}

function sameSet<T>(left: ReadonlySet<T> | undefined, right: ReadonlySet<T>): boolean {
  if (!left || left.size !== right.size) return false;
  for (const value of left) {
    if (!right.has(value)) return false;
  }
  return true;
}

/**
 * This is deliberately projection-only: it never mutates the complete graph.
 * Incomplete streamed nodes keep their previous rendered residency until bounds arrive.
 */
export function computeRenderableGraph(
  fullGraph: { nodes: GraphNode[]; edges: GraphEdge[] },
  options: GraphRenderableProjectionOptions,
): GraphRenderableProjection {
  const threshold = options.virtualizationThreshold ?? DEFAULT_VIRTUALIZATION_THRESHOLD;
  const pinned = options.pinnedRenderHandles ?? new Set<number>();
  const previousNodes = options.previousNodeIds ?? new Set<number>();
  const shouldVirtualize = fullGraph.nodes.length > threshold && options.viewport != null;
  const viewport = options.viewport
    ? expandViewport(options.viewport, options.overscan ?? DEFAULT_OVERSCAN)
    : null;

  const nodes =
    shouldVirtualize && viewport
      ? fullGraph.nodes.filter((node) => {
          if (pinned.has(node.renderHandle)) return true;
          if (!hasRenderableBounds(node)) return previousNodes.has(node.renderHandle);
          return doesBoxIntersectBounds(node.boxArgs, viewport);
        })
      : fullGraph.nodes;
  const nodeIds = new Set(nodes.map((node) => node.renderHandle));
  const edges = fullGraph.edges.filter(
    (edge) => nodeIds.has(edge.fromRenderHandle) && nodeIds.has(edge.toRenderHandle),
  );
  const edgeKeys = new Set(edges.map(graphEdgeKey));

  return {
    nodes,
    edges,
    nodeIds,
    edgeKeys,
    changed:
      !sameSet(options.previousNodeIds, nodeIds) || !sameSet(options.previousEdgeKeys, edgeKeys),
  };
}
