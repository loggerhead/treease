import { buildPathKey } from '../../../graph/graph-viewer-path';
import type { GraphCell, GraphCellKind, GraphNode } from '../../../graph/graph-viewer-render';
import type { PathSeg } from '../../../store/tree-path';
import {
  getCellEntry,
  registerCellBox,
  registerRowBox,
  unregisterCellBox,
  unregisterRowBox,
  upsertCellEntry,
  updateCellEntry,
} from '../graph-anchor-index';
import { rebuildTableCellAnchorIndex, indexTableCellAnchorsForNode, removeTableCellAnchorsForNode } from '../graph-table-anchor-index';
import type { CellBoxEntry, LeaferBox, ScrollableBox } from '../model';
import type { TableCellAnchor } from '../graph-table-anchor-index';

export function createGraphRenderState() {
  let fullNodeDataMap = new Map<number, GraphNode>();
  let renderedNodeDataMap = new Map<number, GraphNode>();
  let nodeBoxMap = new Map<number, LeaferBox>();
  let pathKeyToRenderHandleMap = new Map<string, number>();
  let cellBoxByPathMap = new Map<string, CellBoxEntry>();
  let tableCellAnchorMap = new Map<string, TableCellAnchor>();

  function setFullGraph(nodes: GraphNode[]): void {
    fullNodeDataMap = new Map(nodes.map((node) => [node.renderHandle, node]));
    pathKeyToRenderHandleMap = new Map();
    tableCellAnchorMap = rebuildTableCellAnchorIndex(nodes);
    nodes.forEach((node) => {
      const pathKey = buildPathKey(node.path ?? []);
      if (!pathKey || pathKeyToRenderHandleMap.has(pathKey)) return;
      pathKeyToRenderHandleMap.set(pathKey, node.renderHandle);
    });
  }

  function beginMainGraphRedraw(
    nodes: GraphNode[],
    resetClickTargets?: () => void,
  ): Map<number, LeaferBox> {
    setFullGraph(nodes);
    renderedNodeDataMap = new Map();
    nodeBoxMap = new Map();
    cellBoxByPathMap = new Map();
    resetClickTargets?.();
    return nodeBoxMap;
  }

  return {
    beginMainGraphRedraw,
    setFullGraph,
    getFullNodeDataMap: () => fullNodeDataMap,
    getRenderedNodeDataMap: () => renderedNodeDataMap,
    getNodeBoxMap: () => nodeBoxMap,
    getPathKeyToRenderHandleMap: () => pathKeyToRenderHandleMap,
    getCellBoxByPathMap: () => cellBoxByPathMap,
    getTableCellAnchorMap: () => tableCellAnchorMap,
    getCellEntry: (path: PathSeg[] | null | undefined) => getCellEntry(cellBoxByPathMap, path),
    upsertCellEntry: (cell: GraphCell, updater: (entry: CellBoxEntry) => void) =>
      upsertCellEntry(cellBoxByPathMap, cell, updater),
    updateCellEntry: (cell: GraphCell, updater: (entry: CellBoxEntry) => void) =>
      updateCellEntry(cellBoxByPathMap, cell, updater),
    registerCellBox: (cell: GraphCell, kind: GraphCellKind, box: LeaferBox) =>
      registerCellBox(cellBoxByPathMap, cell, kind, box),
    unregisterCellBox: (cell: GraphCell, kind: GraphCellKind, box: LeaferBox) =>
      unregisterCellBox(cellBoxByPathMap, cell, kind, box),
    registerRowBox: (
      cell: GraphCell,
      rowBox: LeaferBox,
      scrollOwner?: ScrollableBox,
      bodyHeight?: number,
      contentHeight?: number,
    ) => registerRowBox(cellBoxByPathMap, cell, rowBox, scrollOwner, bodyHeight, contentHeight),
    unregisterRowBox: (cell: GraphCell, rowBox: LeaferBox) => unregisterRowBox(cellBoxByPathMap, cell, rowBox),
    indexTableCellAnchorsForNode: (node: GraphNode) => indexTableCellAnchorsForNode(tableCellAnchorMap, node),
    removeTableCellAnchorsForNode: (nodeId: number) => removeTableCellAnchorsForNode(tableCellAnchorMap, nodeId),
  };
}

export function createGraphTextLinkageRenderDeps(
  graphRenderState: ReturnType<typeof createGraphRenderState>,
  scrollTableCellAnchorIntoView: (anchor: TableCellAnchor) => boolean,
) {
  return {
    getNodeDataMap: () => graphRenderState.getFullNodeDataMap(),
    getNodeBoxMap: () => graphRenderState.getNodeBoxMap(),
    getCellBoxByPathMap: () => graphRenderState.getCellBoxByPathMap(),
    getTableCellAnchorMap: () => graphRenderState.getTableCellAnchorMap(),
    getPathKeyToRenderHandleMap: () => graphRenderState.getPathKeyToRenderHandleMap(),
    scrollTableCellAnchorIntoView,
  };
}

export function createGraphSceneRenderDeps(
  graphRenderState: ReturnType<typeof createGraphRenderState>,
  resetRootClickTargets: () => void,
) {
  return {
    beginMainGraphRedraw: (nodes: GraphNode[]) =>
      graphRenderState.beginMainGraphRedraw(nodes, resetRootClickTargets),
    setFullGraph: (nodes: GraphNode[]) => graphRenderState.setFullGraph(nodes),
    getNodeDataMap: () => graphRenderState.getRenderedNodeDataMap(),
    getNodeBoxMap: () => graphRenderState.getNodeBoxMap(),
  };
}
