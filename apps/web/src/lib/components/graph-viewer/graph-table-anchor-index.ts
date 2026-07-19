// Responsibility: index table-cell canonical paths by their table-runtime row and column positions.
import { buildPathKey } from '../../graph/graph-viewer-path';
import type { GraphCell, GraphCellKind, GraphNode } from '../../graph/graph-viewer-render';

export type TableCellAnchor = {
  nodeId: number;
  rowIndex: number;
  cellIndex: number;
  target: GraphCellKind;
};

export function indexTableCellAnchorsForNode(map: Map<string, TableCellAnchor>, node: GraphNode): void {
  if (node.kind !== 'table' || !node.table) return;
  node.table.rows.forEach((row, rowIndex) => {
    row.cells?.forEach((cell: GraphCell, cellIndex: number) => {
      const pathKey = buildPathKey(cell.path ?? []);
      if (!pathKey) return;
      map.set(pathKey, {
        nodeId: node.renderHandle,
        rowIndex,
        cellIndex,
        target: cellIndex === 0 ? 'key' : 'value',
      });
    });
  });
}

export function removeTableCellAnchorsForNode(map: Map<string, TableCellAnchor>, nodeId: number): void {
  for (const [pathKey, anchor] of map.entries()) {
    if (anchor.nodeId === nodeId) map.delete(pathKey);
  }
}

export function rebuildTableCellAnchorIndex(nodes: GraphNode[]): Map<string, TableCellAnchor> {
  const map = new Map<string, TableCellAnchor>();
  nodes.forEach((node) => indexTableCellAnchorsForNode(map, node));
  return map;
}
