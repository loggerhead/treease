// Responsibility: calculate the desired Leafer scene from immutable graph facts and viewport input.
import type { GraphEdge, GraphNode } from './types';
import { doesBoxIntersectBounds, type ViewportBounds } from './viewport-geometry';

export type GraphModel = {
  revision: number;
  nodes: readonly GraphNode[];
  edges: readonly GraphEdge[];
  nodeById: ReadonlyMap<number, GraphNode>;
  edgeByKey: ReadonlyMap<string, GraphEdge>;
};

export type MaterializationIntent = {
  revision: number;
  targetNodeIds: readonly number[];
  contextNodeIds: readonly number[];
  anchor: ViewportBounds | null;
};

export type GraphProjection = {
  graphRevision: number;
  viewportRevision: number;
  materializationIntentRevision: number;
  revision: string;
  nodeIds: ReadonlySet<number>;
  edgeKeys: ReadonlySet<string>;
  viewport: ViewportBounds | null;
  intent: MaterializationIntent | null;
};

export type ProjectionInput = {
  viewport: ViewportBounds | null;
  viewportRevision: number;
  materializationIntent: MaterializationIntent | null;
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

export function createGraphModel(
  revision: number,
  nodes: readonly GraphNode[],
  edges: readonly GraphEdge[],
): GraphModel {
  return {
    revision,
    nodes,
    edges,
    nodeById: new Map(nodes.map((node) => [node.renderHandle, node])),
    edgeByKey: new Map(edges.map((edge) => [graphEdgeKey(edge), edge])),
  };
}

function expandViewport(
  viewport: ViewportBounds,
  overscan: number,
): ViewportBounds {
  return {
    left: viewport.left - overscan,
    right: viewport.right + overscan,
    top: viewport.top - overscan,
    bottom: viewport.bottom + overscan,
  };
}

function hasRenderableBounds(node: GraphNode): boolean {
  const { x, y, width, height } = node.boxArgs;
  return (
    [x, y, width, height].every(Number.isFinite) && width >= 0 && height >= 0
  );
}

function doesEdgeIntersectBounds(
  edge: GraphEdge,
  bounds: ViewportBounds,
): boolean {
  const curve = edge.bezierArgs;
  const left = Math.min(curve.fromX, curve.c1x, curve.c2x, curve.toX);
  const right = Math.max(curve.fromX, curve.c1x, curve.c2x, curve.toX);
  const top = Math.min(curve.fromY, curve.c1y, curve.c2y, curve.toY);
  const bottom = Math.max(curve.fromY, curve.c1y, curve.c2y, curve.toY);
  return (
    right >= bounds.left &&
    left <= bounds.right &&
    bottom >= bounds.top &&
    top <= bounds.bottom
  );
}

/** Pure: the model is complete and the result contains only scene identities. */
export function computeProjection(
  model: GraphModel,
  input: ProjectionInput,
): GraphProjection {
  const threshold =
    input.virtualizationThreshold ?? DEFAULT_VIRTUALIZATION_THRESHOLD;
  const viewport = input.viewport
    ? expandViewport(input.viewport, input.overscan ?? DEFAULT_OVERSCAN)
    : null;
  const virtualized = model.nodes.length > threshold && viewport != null;
  const intentNodeIds = new Set([
    ...(input.materializationIntent?.targetNodeIds ?? []),
    ...(input.materializationIntent?.contextNodeIds ?? []),
  ]);
  const nodeIds = new Set(
    (virtualized
      ? model.nodes.filter(
          (node) =>
            intentNodeIds.has(node.renderHandle) ||
            (hasRenderableBounds(node) &&
              doesBoxIntersectBounds(node.boxArgs, viewport)),
        )
      : model.nodes
    ).map((node) => node.renderHandle),
  );
  const edgeKeys = new Set(
    (virtualized
      ? model.edges.filter((edge) => doesEdgeIntersectBounds(edge, viewport))
      : model.edges
    ).map(graphEdgeKey),
  );
  const intentRevision = input.materializationIntent?.revision ?? 0;
  return {
    graphRevision: model.revision,
    viewportRevision: input.viewportRevision,
    materializationIntentRevision: intentRevision,
    revision: `${model.revision}:${input.viewportRevision}:${intentRevision}`,
    nodeIds,
    edgeKeys,
    viewport: input.viewport,
    intent: input.materializationIntent,
  };
}
