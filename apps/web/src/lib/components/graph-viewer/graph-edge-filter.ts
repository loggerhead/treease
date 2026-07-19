// Responsibility: GraphViewer edge filtering; filter dense offscreen edges by viewport.
import type { GraphEdge, GraphNode } from '../../graph/graph-viewer-render';
import { doesBoxIntersectBounds, getViewportBounds, isPointInBounds } from './graph-viewport-geometry';

export type TableVisibleRange = {
  start: number;
  end: number;
};

function isVisibleTableSourceRow(
  edge: GraphEdge,
  sourceNode: GraphNode | undefined,
  tableVisibleRanges?: ReadonlyMap<number, TableVisibleRange>,
): boolean {
  if (!sourceNode || sourceNode.kind !== 'table' || !sourceNode.table || !tableVisibleRanges) {
    return true;
  }
  const visibleRange = tableVisibleRanges.get(edge.fromRenderHandle);
  if (!visibleRange) {
    return true;
  }
  const headerOffset = sourceNode.table.headerHeight > 0 ? 1 : 0;
  if (headerOffset === 1 && edge.fromRow === 0) {
    return true;
  }
  const bodyRowIndex = edge.fromRow - headerOffset;
  if (bodyRowIndex < 0) {
    return false;
  }
  return bodyRowIndex >= visibleRange.start && bodyRowIndex < visibleRange.end;
}

export function filterDenseOffscreenEdges(
  nodes: GraphNode[],
  edges: GraphEdge[],
  container: HTMLElement | null,
  leafer: Parameters<typeof getViewportBounds>[1],
  maxPerSource: number,
  tableVisibleRanges?: ReadonlyMap<number, TableVisibleRange>,
): GraphEdge[] {
  const bounds = getViewportBounds(container, leafer);
  const nodeMap = new Map(nodes.map((node) => [node.renderHandle, node]));
  const counts = new Map<string, number>();
  const nextEdges: GraphEdge[] = [];
  edges.forEach((edge) => {
    const sourceNode = nodeMap.get(edge.fromRenderHandle);
    if (!isVisibleTableSourceRow(edge, sourceNode, tableVisibleRanges)) {
      return;
    }
    const from = edge.bezierArgs;
    const toNode = nodeMap.get(edge.toRenderHandle);
    if (!bounds || maxPerSource <= 0 || !from || !toNode) {
      nextEdges.push(edge);
      return;
    }
    const fromVisible = isPointInBounds({ x: from.fromX, y: from.fromY }, bounds);
    const toVisible = doesBoxIntersectBounds(toNode.boxArgs, bounds);
    if (fromVisible || toVisible) {
      nextEdges.push(edge);
      return;
    }
    const key = `${edge.fromRenderHandle}:${edge.fromRow}`;
    const count = counts.get(key) ?? 0;
    if (count < maxPerSource) {
      nextEdges.push(edge);
      counts.set(key, count + 1);
    }
  });
  return nextEdges;
}
