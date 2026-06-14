// 职责：Graph hover panel prewarm 收集：isPrewarmableStructuredCell、candidate 遍历与去重
import type { GraphCell, GraphEdge, GraphNode } from '../../graph/graph-viewer-render';
import { buildPathKey } from '../../graph/graph-viewer-path';
import type { GraphViewerClickTarget } from './model';
import type { TooltipPanelPrewarmCandidate } from './graph-hover-panel-types';

export const tooltipPanelPrewarmLimit = 10;

export function isEmptyCompositeCell(cell: GraphCell): boolean {
  const text = cell.text.trim();
  return (cell.valueType === 'object' && (text === '{}' || text === '{0}')) || (cell.valueType === 'array' && (text === '[]' || text === '[0]'));
}

function isScrollableTableNode(node: GraphNode): boolean {
  const table = node.table;
  if (!table) return false;
  const viewHeight = table.viewHeight ?? 0;
  const totalHeight = table.totalHeight ?? 0;
  if (viewHeight > 0 && totalHeight > viewHeight) return true;
  if (viewHeight <= 0) return false;
  const headerHeight = table.headerHeight ?? 0;
  let rowEndY = headerHeight;
  table.rows.forEach((row) => {
    rowEndY = Math.max(rowEndY, row.boxArgs.y + row.boxArgs.height);
  });
  return Math.max(0, rowEndY - headerHeight) > viewHeight;
}

export function isPrewarmableStructuredCell(cell: GraphCell): boolean {
  const isStructuredValue = cell.valueType === 'object' || cell.valueType === 'array';
  return !!cell.isTableCell && (!cell.isHeaderlessTable || cell.isScrollableTable) && isStructuredValue && !isEmptyCompositeCell(cell);
}

function buildPrewarmCandidateKey(cell: GraphCell): string {
  const pathKey = buildPathKey(cell.path ?? []);
  if (pathKey) return `path:${pathKey}`;
  return '';
}

function compareGraphNodeForPrewarm(left: GraphNode, right: GraphNode): number {
  if (left.boxArgs.x !== right.boxArgs.x) return left.boxArgs.x - right.boxArgs.x;
  if (left.boxArgs.y !== right.boxArgs.y) return left.boxArgs.y - right.boxArgs.y;
  return left.renderHandle - right.renderHandle;
}

export function collectTooltipPanelPrewarmCandidates(
  graphData: { nodes: GraphNode[]; edges: GraphEdge[] } | null,
  canOpenSubgraphPreviewForCell: (cell: GraphCell, target: 'key' | 'value' | 'node') => boolean,
  limit = tooltipPanelPrewarmLimit,
): TooltipPanelPrewarmCandidate[] {
  if (!graphData || limit <= 0) return [];
  const { nodes, edges } = graphData;
  if (!nodes.length) return [];
  const nodeById = new Map(nodes.map((node) => [node.renderHandle, node]));
  const incomingCountByNodeId = new Map<number, number>();
  const outgoingEdgesByNodeId = new Map<number, GraphEdge[]>();
  edges.forEach((edge) => {
    incomingCountByNodeId.set(edge.toRenderHandle, (incomingCountByNodeId.get(edge.toRenderHandle) ?? 0) + 1);
    const next = outgoingEdgesByNodeId.get(edge.fromRenderHandle) ?? [];
    next.push(edge);
    outgoingEdgesByNodeId.set(edge.fromRenderHandle, next);
  });
  outgoingEdgesByNodeId.forEach((outgoing) => {
    outgoing.sort((left, right) => {
      if (left.fromRow !== right.fromRow) return left.fromRow - right.fromRow;
      const leftNode = nodeById.get(left.toRenderHandle);
      const rightNode = nodeById.get(right.toRenderHandle);
      if (leftNode && rightNode) return compareGraphNodeForPrewarm(leftNode, rightNode);
      return left.toRenderHandle - right.toRenderHandle;
    });
  });
  const roots = nodes
    .filter((node) => !incomingCountByNodeId.has(node.renderHandle))
    .sort(compareGraphNodeForPrewarm);
  const candidates: TooltipPanelPrewarmCandidate[] = [];
  const seenPathKeys = new Set<string>();
  const visitedNodes = new Set<number>();
  const maybeAppendCandidate = (cell: GraphCell) => {
    if (!isPrewarmableStructuredCell(cell)) return;
    if (!canOpenSubgraphPreviewForCell(cell, 'value')) return;
    const candidateKey = buildPrewarmCandidateKey(cell);
    if (!candidateKey || seenPathKeys.has(candidateKey)) return;
    seenPathKeys.add(candidateKey);
    candidates.push({ cell, target: 'value' });
  };
  const visitNode = (node: GraphNode) => {
    const nodeHandle = node.renderHandle;
    if (visitedNodes.has(nodeHandle) || candidates.length >= limit) return;
    visitedNodes.add(nodeHandle);
    const headerOffset = node.kind === 'table' && node.table && (node.table.headerHeight ?? 0) > 0 ? 1 : 0;
    const outgoing = outgoingEdgesByNodeId.get(nodeHandle) ?? [];
    const childrenByRow = new Map<number, GraphEdge[]>();
    outgoing.forEach((edge) => {
      const bucket = childrenByRow.get(edge.fromRow) ?? [];
      bucket.push(edge);
      childrenByRow.set(edge.fromRow, bucket);
    });
    if (node.kind === 'table' && node.table) {
      const isHeaderlessTable = (node.table.headerHeight ?? 0) === 0;
      const isScrollableTable = isScrollableTableNode(node);
      node.table.rows.forEach((row, rowIndex) => {
        if (candidates.length >= limit) return;
        row.cells.forEach((cell) => {
          if (candidates.length >= limit) return;
          cell.isHeaderlessTable = isHeaderlessTable;
          cell.isScrollableTable = isScrollableTable;
          maybeAppendCandidate(cell);
        });
        const rowEdges = childrenByRow.get(rowIndex + headerOffset) ?? [];
        rowEdges.forEach((edge) => {
          const child = nodeById.get(edge.toRenderHandle);
          if (child) visitNode(child);
        });
      });
      return;
    }
    node.rows.forEach((row, rowIndex) => {
      if (candidates.length >= limit) return;
      row.cells.forEach((cell) => {
        if (candidates.length >= limit) return;
        maybeAppendCandidate(cell);
      });
      const rowEdges = childrenByRow.get(rowIndex) ?? [];
      rowEdges.forEach((edge) => {
        const child = nodeById.get(edge.toRenderHandle);
        if (child) visitNode(child);
      });
    });
  };
  roots.forEach((root) => {
    if (candidates.length >= limit) return;
    visitNode(root);
  });
  return candidates;
}

export function collectTooltipPanelPrewarmCandidatesFromClickTargets(
  clickTargets: GraphViewerClickTarget[],
  canOpenSubgraphPreviewForCell: (cell: GraphCell, target: 'key' | 'value' | 'node') => boolean,
  limit = tooltipPanelPrewarmLimit,
): TooltipPanelPrewarmCandidate[] {
  if (!clickTargets.length || limit <= 0) return [];
  const candidates: TooltipPanelPrewarmCandidate[] = [];
  const seenCandidateKeys = new Set<string>();
  clickTargets.forEach((entry) => {
    if (candidates.length >= limit) return;
    if (entry.target !== 'value') return;
    const cell = entry.cell;
    if (!isPrewarmableStructuredCell(cell)) return;
    if (!canOpenSubgraphPreviewForCell(cell, 'value')) return;
    const candidateKey = buildPrewarmCandidateKey(cell);
    if (!candidateKey || seenCandidateKeys.has(candidateKey)) return;
    seenCandidateKeys.add(candidateKey);
    candidates.push({ cell, target: 'value' });
  });
  return candidates;
}
