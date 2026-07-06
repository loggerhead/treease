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
  let nodeDataMap = new Map<number, GraphNode>();
  let nodeBoxMap = new Map<number, LeaferBox>();
  let pathKeyToRenderHandleMap = new Map<string, number>();
  let cellBoxByPathMap = new Map<string, CellBoxEntry>();
  let tableCellAnchorMap = new Map<string, TableCellAnchor>();

  function beginMainGraphRedraw(nodes: GraphNode[], resetClickTargets?: () => void): Map<number, LeaferBox> {
    nodeDataMap = new Map(nodes.map((node) => [node.renderHandle, node]));
    nodeBoxMap = new Map();
    pathKeyToRenderHandleMap = new Map();
    cellBoxByPathMap = new Map();
    tableCellAnchorMap = rebuildTableCellAnchorIndex(nodes);
    resetClickTargets?.();
    nodes.forEach((node) => {
      const pathKey = buildPathKey(node.path ?? []);
      if (!pathKey) return;
      if (!pathKeyToRenderHandleMap.has(pathKey)) {
        pathKeyToRenderHandleMap.set(pathKey, node.renderHandle);
      }
    });
    return nodeBoxMap;
  }

  return {
    beginMainGraphRedraw,
    getNodeDataMap: () => nodeDataMap,
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
    getNodeDataMap: () => graphRenderState.getNodeDataMap(),
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
    getNodeDataMap: () => graphRenderState.getNodeDataMap(),
    getNodeBoxMap: () => graphRenderState.getNodeBoxMap(),
    getPathKeyToRenderHandleMap: () => graphRenderState.getPathKeyToRenderHandleMap(),
    indexTableCellAnchorsForNode: (node: GraphNode) => graphRenderState.indexTableCellAnchorsForNode(node),
    removeTableCellAnchorsForNode: (nodeId: number) => graphRenderState.removeTableCellAnchorsForNode(nodeId),
  };
}
