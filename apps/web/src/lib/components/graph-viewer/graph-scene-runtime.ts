// 职责：GraphViewer Leafer 场景生命周期：create/dispose/remount scene、node/edge layer 管理
import type { GraphViewerConfig } from '../../settings/ui-settings';
import {
  applyGraphDeltaToState,
  applyVersionedProjection,
  clearStreamState,
  createEmptyStreamState,
  replaceStreamState,
  streamStateToArrays,
} from '../../graph/StreamUpdateHandler';
import { buildPathKey } from '../../graph/graph-viewer-path';
import {
  createCellText,
  describeTableRuntime,
  destroyTableRuntime,
  patchTableContent,
  patchTableStructure,
  tableRuntimeOps,
  type DrawContext,
  type GraphEdge,
  type GraphNode,
  type TableRuntime,
} from '../../graph/graph-viewer-render';
import type { InternalTableRuntime } from './runtime/table-runtime';
import { createGraphDirtyRegion, type GraphDirtyRegionRect } from './graph-dirty-region';
import { renderGraphEdges, renderGraphNode } from './graph-render-kernel';
import type { NormalizedGraphDelta, RawGraphDelta } from '../../../shared/worker-protocol/protocol';
import type { GraphSceneLayers, GraphViewerClickTargetStore } from './model';

export type GraphSceneViewData = {
  nodes: GraphNode[];
  edges: GraphEdge[];
};

export type GraphSceneInteractionState = {
  hasGraphData: boolean;
  nodeCount: number;
  rootProbeCount: number;
  pendingRenderWork: boolean;
};

type GraphSceneDelta = NormalizedGraphDelta | RawGraphDelta;

type TrackedCellBinding = {
  cell: any;
  kind: any;
  box: any;
};

type TrackedRowBinding = {
  cell: any;
  rowBox: any;
};

type TablePatchMode = 'append' | 'content' | 'structure';

type PendingStreamPatch = {
  clear: boolean;
  nodeIds: Set<number>;
  tablePatchNodeIds: Set<number>;
  tablePatchModes: Map<number, TablePatchMode>;
  edgeChange: boolean;
};

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === 'object' ? (value as Record<string, unknown>) : null;
}

function rawNodeRenderHandle(node: unknown): number | null {
  const handle = asRecord(node)?.renderHandle;
  return typeof handle === 'number' ? handle : null;
}

function layoutPatchRenderHandle(patch: unknown): number | null {
  const record = asRecord(patch);
  if (!record) return null;
  if (record.kind === 'nodeBoundsUpdated') {
    const handle = record.render_handle ?? record.renderHandle;
    return typeof handle === 'number' ? handle : null;
  }
  if (record.kind === 'groupLayoutUpdated') {
    const handle = record.group_handle ?? record.groupHandle;
    return typeof handle === 'number' ? handle : null;
  }
  return null;
}

function arrayValue(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

function mergeTablePatchMode(current: TablePatchMode | undefined, next: TablePatchMode): TablePatchMode {
  if (current === 'structure' || next === 'structure') return 'structure';
  if (current === 'content' || next === 'content') return 'content';
  return 'append';
}

function tablePatchHandle(tablePatch: unknown): number | null {
  const record = asRecord(tablePatch);
  const handle = record?.tableRenderHandle ?? record?.table_handle ?? record?.tableHandle;
  return typeof handle === 'number' ? handle : null;
}

function tablePatchMode(tablePatch: unknown): TablePatchMode {
  const kind = asRecord(tablePatch)?.kind;
  if (kind === 'rowsAppended') return 'append';
  if (kind === 'cellsUpdated') return 'content';
  return 'structure';
}

function markTablePatch(patch: PendingStreamPatch, handle: number, mode: TablePatchMode): void {
  patch.nodeIds.add(handle);
  patch.tablePatchNodeIds.add(handle);
  patch.tablePatchModes.set(handle, mergeTablePatchMode(patch.tablePatchModes.get(handle), mode));
}

function createPendingStreamPatch(delta: GraphSceneDelta): PendingStreamPatch {
  const patch: PendingStreamPatch = {
    clear: delta.clear === 1,
    nodeIds: new Set<number>(),
    tablePatchNodeIds: new Set<number>(),
    tablePatchModes: new Map<number, TablePatchMode>(),
    edgeChange: (delta.edgesAdded?.length ?? 0) > 0 || (delta.edgesRemoved?.length ?? 0) > 0,
  };
  if (!patch.clear) {
    delta.nodesRemoved.forEach((nodeId) => {
      if (typeof nodeId === 'number') patch.nodeIds.add(nodeId);
    });
    delta.nodesAdded.forEach((node) => {
      const handle = rawNodeRenderHandle(node);
      if (handle != null) patch.nodeIds.add(handle);
    });
    delta.nodesUpdated.forEach((node) => {
      const handle = rawNodeRenderHandle(node);
      if (handle != null) patch.nodeIds.add(handle);
    });
    const deltaRecord = asRecord(delta);
    arrayValue(deltaRecord?.tableCellPatches).forEach((tablePatch) => {
      const handle = tablePatchHandle(tablePatch);
      if (handle != null) markTablePatch(patch, handle, 'content');
    });
    arrayValue(deltaRecord?.tablePatches).forEach((tablePatch) => {
      const handle = tablePatchHandle(tablePatch);
      if (handle != null) markTablePatch(patch, handle, tablePatchMode(tablePatch));
    });
    arrayValue(deltaRecord?.layoutPatches).forEach((layoutPatch) => {
      const handle = layoutPatchRenderHandle(layoutPatch);
      if (handle != null) {
        patch.nodeIds.add(handle);
        patch.edgeChange = true;
      }
    });
  }
  return patch;
}

function mergePendingStreamPatch(target: PendingStreamPatch, next: PendingStreamPatch): void {
  if (target.clear || next.clear) {
    target.clear = true;
    target.nodeIds.clear();
    target.tablePatchNodeIds.clear();
    target.tablePatchModes.clear();
    target.edgeChange = true;
    return;
  }
  next.nodeIds.forEach((nodeId) => target.nodeIds.add(nodeId));
  next.tablePatchNodeIds.forEach((nodeId) => target.tablePatchNodeIds.add(nodeId));
  next.tablePatchModes.forEach((mode, nodeId) => {
    target.tablePatchModes.set(nodeId, mergeTablePatchMode(target.tablePatchModes.get(nodeId), mode));
  });
  target.edgeChange ||= next.edgeChange;
}

type GraphSceneRuntimeDeps = {
  getContainer: () => HTMLElement | null;
  getLeafer: () => any;
  getRenderRoot: () => any | null;
  getBoxCtor: () => any;
  getTextCtor: () => any;
  getPenCtor: () => any;
  getRenderConfig: () => GraphViewerConfig;
  getLanguageId: () => string;
  getValueTypeToSemType: () => Record<string, string>;
  isReadonly?: () => boolean;
  getLastAutoOffset: () => { x: number; y: number } | null;
  setLastAutoOffset: (value: { x: number; y: number } | null) => void;
  getLayers: () => GraphSceneLayers;
  setLayers: (layers: Partial<GraphSceneLayers>) => void;
  buildPathSegFromCell: (cell: any, rowIndex: number) => any;
  clearSearchHighlight: () => void;
  beginMainGraphRedraw: (nodes: GraphNode[]) => Map<number, any>;
  getNodeDataMap: () => Map<number, GraphNode>;
  getNodeBoxMap: () => Map<number, any>;
  getPathKeyToRenderHandleMap: () => Map<string, number>;
  indexTableCellAnchorsForNode?: (node: GraphNode) => void;
  removeTableCellAnchorsForNode?: (nodeId: number) => void;
  getClickTargetProbes: () => any[];
  getClickTargetProbeStore: () => GraphViewerClickTargetStore;
  registerCellBox: (cell: any, kind: any, box: any) => void;
  unregisterCellBox: (cell: any, kind: any, box: any) => void;
  registerRowBox: (cell: any, rowBox: any, scrollOwner?: any, bodyHeight?: number, contentHeight?: number) => void;
  unregisterRowBox: (cell: any, rowBox: any) => void;
  registerClickTarget: (target: any, cell: any, kind: any, nodeKind?: GraphNode['kind']) => string;
  bindVerticalScrollGesture?: (
    target: any,
    handler: (gesture: {
      event: unknown;
      deltaY: number;
      moveType?: string;
      stop: () => void;
      stopNow: () => void;
    }) => void,
  ) => (() => void) | void;
  bindPointerDown?: (target: any, handler: (event: unknown) => void | Promise<void>) => (() => void) | void;
  getPointFromEvent?: (
    hostApp: any,
    target: any,
    event: unknown,
    space: 'client' | 'box' | 'local' | 'world',
  ) => { x: number; y: number } | null;
  refreshActiveHighlight?: () => void;
  updateLeafer: () => void;
};

export function createGraphSceneRuntime(deps: GraphSceneRuntimeDeps) {
  let lastGraphData: GraphSceneViewData | null = null;
  const streamState = createEmptyStreamState();
  const dirtyRegion = createGraphDirtyRegion();
  const nodeMetaTextById = new Map<number, any>();
  const tableRuntimeByNodeId = new Map<number, TableRuntime>();
  const nodeCellBindingsById = new Map<number, Map<object, TrackedCellBinding>>();
  const nodeRowBindingsById = new Map<number, Map<object, TrackedRowBinding>>();
  const nodeClickTargetsById = new Map<number, Set<string>>();
  let pendingStreamPatch: PendingStreamPatch | null = null;
  let pendingStreamFrame: number | null = null;
  let pendingStreamRedrawDone: Promise<void> | null = null;
  let resolvePendingStreamRedraw: (() => void) | null = null;
  let pendingViewportRedrawFrame: number | null = null;
  let pendingViewportRedrawDone: Promise<void> | null = null;
  let pendingNodeBuffer: GraphNode[] = [];
  let pendingBufferTimer: ReturnType<typeof setTimeout> | null = null;
  let resolvePendingViewportRedraw: (() => void) | null = null;

  function flushLeaferSceneLayout(): void {
    deps.updateLeafer();
  }

  function inferGraphPaths(nodes: GraphNode[], edges: GraphEdge[]): void {
    if (!nodes.length) return;
    const tableHeaderOffset = (node: GraphNode): number =>
      node.kind === 'table' && node.table && (node.table.headerHeight ?? 0) > 0 ? 1 : 0;
    const rowsForNode = (node: GraphNode) => (node.kind === 'table' ? (node.table?.rows ?? []) : node.rows);
    const nodeById = new Map(nodes.map((node) => [node.renderHandle, node]));
    const incomingEdgeByNodeId = new Map<number, GraphEdge>();
    const outgoingEdgesByNodeId = new Map<number, GraphEdge[]>();
    edges.forEach((edge) => {
      if (!incomingEdgeByNodeId.has(edge.toRenderHandle)) incomingEdgeByNodeId.set(edge.toRenderHandle, edge);
      const list = outgoingEdgesByNodeId.get(edge.fromRenderHandle) ?? [];
      list.push(edge);
      outgoingEdgesByNodeId.set(edge.fromRenderHandle, list);
    });
    const rowPathByNodeId = new Map<number, Map<number, any[]>>();
    const queue = nodes.filter((node) => !incomingEdgeByNodeId.has(node.renderHandle));
    const visited = new Set<number>();
    while (queue.length) {
      const node = queue.shift();
      if (!node) continue;
      const nodeHandle = node.renderHandle;
      if (visited.has(nodeHandle)) continue;
      visited.add(nodeHandle);
      if (!Array.isArray(node.path) || node.path.length === 0) {
        const incoming = incomingEdgeByNodeId.get(nodeHandle);
        if (incoming) {
          const parentRows = rowPathByNodeId.get(incoming.fromRenderHandle);
          const inferredPath = parentRows?.get(incoming.fromRow) ?? [];
          node.path = inferredPath;
        } else {
          node.path = [];
        }
      }
      const nodePath = Array.isArray(node.path) ? node.path : [];
      const rowPaths = new Map<number, any[]>();
      rowsForNode(node).forEach((row, rowIndex) => {
        const seg = row.cells[0] ? deps.buildPathSegFromCell(row.cells[0], rowIndex) : null;
        const nextPath = seg ? [...nodePath, seg] : nodePath;
        rowPaths.set(rowIndex + tableHeaderOffset(node), nextPath);
        row.cells.forEach((cell) => {
          if (!Array.isArray(cell.path) || cell.path.length === 0) {
            cell.path = nextPath;
          }
        });
      });
      rowPathByNodeId.set(nodeHandle, rowPaths);
      const outgoing = outgoingEdgesByNodeId.get(nodeHandle) ?? [];
      outgoing.forEach((edge) => {
        const child = nodeById.get(edge.toRenderHandle);
        if (!child) return;
        const inferredPath = rowPaths.get(edge.fromRow);
        if (inferredPath && (!Array.isArray(child.path) || child.path.length === 0)) {
          child.path = inferredPath;
        }
        queue.push(child);
      });
    }
  }

  function ensureLayers(): void {
    const BoxCtor = deps.getBoxCtor();
    if (!BoxCtor) return;
    const root = deps.getRenderRoot();
    if (!root) return;
    const layers = deps.getLayers();
    const nextLayers: Partial<GraphSceneLayers> = {};
    if (!layers.edgeLayer) {
      nextLayers.edgeLayer = new BoxCtor({ x: 0, y: 0, width: 0, height: 0, fill: 'transparent' });
      root.add(nextLayers.edgeLayer);
    }
    if (!layers.nodeLayer) {
      nextLayers.nodeLayer = new BoxCtor({ x: 0, y: 0, width: 0, height: 0, fill: 'transparent' });
      root.add(nextLayers.nodeLayer);
    }
    if (!layers.overlayLayer) {
      nextLayers.overlayLayer = new BoxCtor({
        x: 0,
        y: 0,
        width: 0,
        height: 0,
        fill: 'transparent',
        hittable: false,
        hitChildren: false,
      });
      root.add(nextLayers.overlayLayer);
    }
    if (Object.keys(nextLayers).length > 0) {
      deps.setLayers(nextLayers);
    }
  }

  function drawEdges(nodes: GraphNode[], edges: GraphEdge[], options?: { layer?: any; maxPerSource?: number | null }) {
    ensureLayers();
    const tableVisibleRanges =
      tableRuntimeByNodeId.size === 0
        ? undefined
        : new Map(Array.from(tableRuntimeByNodeId.entries(), ([nodeId, runtime]) => [nodeId, runtime.visibleRange]));
    return renderGraphEdges({
      nodes,
      edges,
      layer: options?.layer ?? deps.getLayers().edgeLayer,
      PenCtor: deps.getPenCtor(),
      renderConfig: deps.getRenderConfig(),
      container: deps.getContainer(),
      leafer: deps.getLeafer(),
      maxPerSource: options?.maxPerSource === undefined ? 10 : options.maxPerSource,
      tableVisibleRanges,
    });
  }

  function updateAutoPosition(nodes: GraphNode[]): void {
    const renderConfig = deps.getRenderConfig();
    const padding = renderConfig.layout.canvasPadding ?? 0;
    const leafer = deps.getLeafer();
    if (!leafer?.zoomLayer) return;
    const layer = leafer.zoomLayer as { x?: number; y?: number };
    const currentX = layer.x ?? 0;
    const currentY = layer.y ?? 0;
    const lastAutoOffset = deps.getLastAutoOffset();
    const canAutoPosition =
      !lastAutoOffset || (Math.abs(currentX - lastAutoOffset.x) < 0.5 && Math.abs(currentY - lastAutoOffset.y) < 0.5);
    let minX = 0;
    let minY = 0;
    if (nodes.length > 0) {
      minX = Number.POSITIVE_INFINITY;
      minY = Number.POSITIVE_INFINITY;
      for (const node of nodes) {
        const metaX = node.meta?.boxArgs?.x ?? node.boxArgs.x;
        const metaY = node.meta?.boxArgs?.y ?? node.boxArgs.y;
        minX = Math.min(minX, node.boxArgs.x, metaX);
        minY = Math.min(minY, node.boxArgs.y, metaY);
      }
      if (!Number.isFinite(minX)) minX = 0;
      if (!Number.isFinite(minY)) minY = 0;
    }
    if (!canAutoPosition) return;
    layer.x = padding - minX;
    layer.y = padding - minY;
    deps.setLastAutoOffset({ x: layer.x ?? 0, y: layer.y ?? 0 });
  }

  function removeRenderable(target: any): void {
    if (!target) return;
    target.removeAll?.(true);
    target.remove?.();
  }

  function deleteNodePathMapping(renderHandle: number): void {
    const pathKeyToRenderHandleMap = deps.getPathKeyToRenderHandleMap();
    for (const [pathKey, value] of pathKeyToRenderHandleMap.entries()) {
      if (value === renderHandle) pathKeyToRenderHandleMap.delete(pathKey);
    }
  }

  function setNodePathMapping(node: GraphNode): void {
    const nodeHandle = node.renderHandle;
    deleteNodePathMapping(nodeHandle);
    const pathKey = buildPathKey(node.path ?? []);
    if (!pathKey) return;
    const pathKeyToRenderHandleMap = deps.getPathKeyToRenderHandleMap();
    const existing = pathKeyToRenderHandleMap.get(pathKey);
    if (existing == null || existing === nodeHandle) {
      pathKeyToRenderHandleMap.set(pathKey, nodeHandle);
    }
  }

  function removeNodeProbeTargets(nodeId: number): void {
    const targets = nodeClickTargetsById.get(nodeId);
    if (!targets?.size) {
      nodeClickTargetsById.delete(nodeId);
      return;
    }
    const probeStore = deps.getClickTargetProbeStore();
    targets.forEach((targetId) => {
      delete probeStore[targetId];
    });
    nodeClickTargetsById.delete(nodeId);
  }

  function clearNodeInteractiveBindings(nodeId: number): void {
    const cellBindings = nodeCellBindingsById.get(nodeId);
    const rowBindings = nodeRowBindingsById.get(nodeId);
    if (cellBindings) {
      for (const binding of cellBindings.values()) {
        deps.unregisterCellBox(binding.cell, binding.kind, binding.box);
      }
      nodeCellBindingsById.delete(nodeId);
    }
    if (rowBindings) {
      for (const binding of rowBindings.values()) {
        deps.unregisterRowBox(binding.cell, binding.rowBox);
      }
      nodeRowBindingsById.delete(nodeId);
    }
    removeNodeProbeTargets(nodeId);
  }

  function removeNodeRender(nodeId: number): void {
    clearNodeInteractiveBindings(nodeId);
    removeRenderable(nodeMetaTextById.get(nodeId));
    nodeMetaTextById.delete(nodeId);
    const drawContext = createTrackedDrawContext(nodeId);
    if (drawContext) {
      destroyTableRuntime(drawContext, tableRuntimeByNodeId.get(nodeId), tableRuntimeOps);
    }
    tableRuntimeByNodeId.delete(nodeId);
    const nodeBoxMap = deps.getNodeBoxMap();
    const nodeBox = nodeBoxMap.get(nodeId);
    removeRenderable(nodeBox);
    nodeBoxMap.delete(nodeId);
    deleteNodePathMapping(nodeId);
    deps.removeTableCellAnchorsForNode?.(nodeId);
    deps.getNodeDataMap().delete(nodeId);
  }

  function clearRenderedMainGraph(): void {
    for (const nodeId of deps.getNodeDataMap().keys()) {
      removeNodeRender(nodeId);
    }
    deps.getNodeDataMap().clear();
    deps.getNodeBoxMap().clear();
    deps.getPathKeyToRenderHandleMap().clear();
    const clickTargetProbeStore = deps.getClickTargetProbeStore();
    Object.keys(clickTargetProbeStore).forEach((targetId) => {
      delete clickTargetProbeStore[targetId];
    });
    nodeMetaTextById.clear();
    tableRuntimeByNodeId.clear();
    nodeCellBindingsById.clear();
    nodeRowBindingsById.clear();
    nodeClickTargetsById.clear();
    deps.getLayers().edgeLayer?.removeAll(true);
    deps.getLayers().overlayLayer?.removeAll(true);
  }

  function toRenderHandleDirtyRect(node: GraphNode | null | undefined): GraphDirtyRegionRect | null {
    if (!node) return null;
    return {
      left: Number(node.boxArgs.x ?? 0),
      top: Number(node.boxArgs.y ?? 0),
      width: Math.max(0, Number(node.boxArgs.width ?? 0)),
      height: Math.max(0, Number(node.boxArgs.height ?? 0)),
    };
  }

  function markNodeDirty(node: GraphNode | null | undefined): void {
    dirtyRegion.mark(toRenderHandleDirtyRect(node));
  }

  function markViewDirty(graphData: GraphSceneViewData | null | undefined): void {
    if (!graphData?.nodes?.length) return;
    graphData.nodes.forEach((node) => {
      markNodeDirty(node);
    });
  }

  function beginFullSceneReplace(graphData: GraphSceneViewData): void {
    cancelPendingNodeBuffer();
    deps.clearSearchHighlight();
    clearRenderedMainGraph();
    deps.beginMainGraphRedraw(graphData.nodes);
    updateAutoPosition(graphData.nodes);
  }

  function createTrackedDrawContext(nodeId: number): DrawContext | null {
    ensureLayers();
    const { nodeLayer } = deps.getLayers();
    const BoxCtor = deps.getBoxCtor();
    const TextCtor = deps.getTextCtor();
    const PenCtor = deps.getPenCtor();
    if (!nodeLayer || !BoxCtor || !TextCtor || !PenCtor) return null;
    const cellBindings = new Map<object, TrackedCellBinding>();
    const rowBindings = new Map<object, TrackedRowBinding>();
    const clickTargets = new Set<string>();
    nodeCellBindingsById.set(nodeId, cellBindings);
    nodeRowBindingsById.set(nodeId, rowBindings);
    nodeClickTargetsById.set(nodeId, clickTargets);
    return {
      nodeLayer,
      styleConfig: deps.getRenderConfig(),
      languageIdValue: deps.getLanguageId(),
      fontSize: deps.getRenderConfig().layout.baseFontSize,
      BoxCtor,
      TextCtor,
      PenCtor,
      valueTypeToSemType: deps.getValueTypeToSemType(),
      editable: deps.isReadonly?.() ? false : undefined,
      registerCellBox: (cell, kind, box) => {
        if (box && typeof box === 'object') {
          cellBindings.set(box, { cell, kind, box });
        }
        deps.registerCellBox(cell, kind, box);
      },
      unregisterCellBox: (cell, kind, box) => {
        if (box && typeof box === 'object') {
          cellBindings.delete(box);
        }
        deps.unregisterCellBox(cell, kind, box);
      },
      registerRowBox: (cell, rowBox, scrollOwner, bodyHeight, contentHeight) => {
        if (rowBox && typeof rowBox === 'object') {
          rowBindings.set(rowBox, { cell, rowBox });
        }
        deps.registerRowBox(cell, rowBox, scrollOwner, bodyHeight, contentHeight);
      },
      unregisterRowBox: (cell, rowBox) => {
        if (rowBox && typeof rowBox === 'object') {
          rowBindings.delete(rowBox);
        }
        deps.unregisterRowBox(cell, rowBox);
      },
      registerClickTarget: (target, cell, kind, nodeKind) => {
        const clickTargetId = deps.registerClickTarget(target, cell, kind, nodeKind);
        if (clickTargetId) clickTargets.add(clickTargetId);
        return clickTargetId;
      },
      requestRender: deps.updateLeafer,
      bindVerticalScrollGesture: deps.bindVerticalScrollGesture,
      bindPointerDown: deps.bindPointerDown,
      getPointFromEvent: deps.getPointFromEvent,
      refreshActiveHighlight: deps.refreshActiveHighlight,
    };
  }

  function renderNodeIntoScene(node: GraphNode): void {
    const nodeHandle = node.renderHandle;
    const drawContext = createTrackedDrawContext(nodeHandle);
    if (!drawContext) return;
    deps.getNodeDataMap().set(nodeHandle, node);
    setNodePathMapping(node);
    deps.removeTableCellAnchorsForNode?.(nodeHandle);
    deps.indexTableCellAnchorsForNode?.(node);
    const clickTargets = nodeClickTargetsById.get(nodeHandle) ?? new Set<string>();
    nodeClickTargetsById.set(nodeHandle, clickTargets);
    const result = renderGraphNode({
      node,
      drawContext,
      registerMetaClickTarget: (target, cell, kind) => {
        const clickTargetId = deps.registerClickTarget(target, cell, kind);
        if (clickTargetId) clickTargets.add(clickTargetId);
      },
    });
    nodeMetaTextById.set(nodeHandle, result.metaText);
    if (result.tableRuntime) {
      tableRuntimeByNodeId.set(nodeHandle, result.tableRuntime as TableRuntime);
    } else {
      tableRuntimeByNodeId.delete(nodeHandle);
    }
    if (result.nodeBox) {
      deps.getNodeBoxMap().set(nodeHandle, result.nodeBox);
      return;
    }
    deps.getNodeBoxMap().delete(nodeHandle);
  }

  function patchTableNodeRender(node: GraphNode, mode: TablePatchMode = 'content'): boolean {
    const nodeHandle = node.renderHandle;
    const existingRuntime = tableRuntimeByNodeId.get(nodeHandle);
    const existingNodeBox = deps.getNodeBoxMap().get(nodeHandle);
    if (!existingRuntime || !existingNodeBox || node.kind !== 'table' || !node.table) {
      return false;
    }

    const drawContext = createTrackedDrawContext(nodeHandle);
    if (!drawContext) return false;

    deps.getNodeDataMap().set(nodeHandle, node);
    setNodePathMapping(node);
    deps.removeTableCellAnchorsForNode?.(nodeHandle);
    deps.indexTableCellAnchorsForNode?.(node);
    existingNodeBox.x = node.boxArgs.x;
    existingNodeBox.y = node.boxArgs.y;
    existingNodeBox.width = node.boxArgs.width;
    existingNodeBox.height = node.boxArgs.height;
    existingNodeBox.cornerRadius = node.boxArgs.cornerRadius;

    const nextDescription = describeTableRuntime(drawContext, node);
    const sameLayout = existingRuntime.layoutSignature === nextDescription.layoutSignature;
    if (sameLayout && mode === 'append') {
      const nextRuntime = patchTableContent(drawContext, existingRuntime, node, tableRuntimeOps, false);
      tableRuntimeByNodeId.set(nodeHandle, nextRuntime);
      deps.getNodeBoxMap().set(nodeHandle, existingNodeBox);
      return true;
    }

    clearNodeInteractiveBindings(nodeHandle);
    removeRenderable(nodeMetaTextById.get(nodeHandle));
    nodeMetaTextById.delete(nodeHandle);

    const clickTargets = nodeClickTargetsById.get(nodeHandle) ?? new Set<string>();
    nodeClickTargetsById.set(nodeHandle, clickTargets);
    const metaText = createCellText(drawContext, drawContext.nodeLayer, node.meta, 'meta', node.kind);
    nodeMetaTextById.set(nodeHandle, metaText);
    const metaTargetId = deps.registerClickTarget(metaText, node.meta, 'meta');
    if (metaTargetId) clickTargets.add(metaTargetId);

    const nextRuntime =
      sameLayout && mode !== 'structure'
        ? patchTableContent(drawContext, existingRuntime, node, tableRuntimeOps, true)
        : patchTableStructure(drawContext, existingRuntime, node, tableRuntimeOps);
    tableRuntimeByNodeId.set(nodeHandle, nextRuntime);
    deps.getNodeBoxMap().set(nodeHandle, existingNodeBox);
    return true;
  }

  function upsertNodeRender(node: GraphNode, tablePatchMode?: TablePatchMode): void {
    if (patchTableNodeRender(node, tablePatchMode)) return;
    removeNodeRender(node.renderHandle);
    renderNodeIntoScene(node);
  }

  function finalizeSceneFrame(
    graphData: GraphSceneViewData,
    skipEdgeRedraw?: boolean,
    skipLeaferRender?: boolean,
  ): GraphSceneViewData {
    if (skipEdgeRedraw && lastGraphData) {
      const edges = lastGraphData.edges;
      dirtyRegion.flush(deps.getLeafer(), false);
      const renderedView = { nodes: graphData.nodes, edges };
      if (!skipLeaferRender) deps.updateLeafer();
      lastGraphData = renderedView;
      const container = deps.getContainer();
      if (container) {
        container.setAttribute('data-graph-node-count', String(graphData.nodes.length));
      }
      return renderedView;
    }
    const edges = drawEdges(graphData.nodes, graphData.edges, { maxPerSource: 10 });
    dirtyRegion.flush(deps.getLeafer(), false);
    const renderedView = { nodes: graphData.nodes, edges };
    if (!skipLeaferRender) flushLeaferSceneLayout();
    lastGraphData = renderedView;
    const container = deps.getContainer();
    if (container) {
      container.setAttribute('data-graph-node-count', String(graphData.nodes.length));
    }
    return renderedView;
  }

  function cancelPendingNodeBuffer(): void {
    if (pendingBufferTimer) {
      clearTimeout(pendingBufferTimer);
      pendingBufferTimer = null;
    }
    pendingNodeBuffer = [];
  }

  function flushNodeBuffer(forceLeafer?: boolean): void {
    if (pendingBufferTimer) {
      clearTimeout(pendingBufferTimer);
      pendingBufferTimer = null;
    }
    const batch = pendingNodeBuffer;
    pendingNodeBuffer = [];
    if (batch.length === 0) return;
    batch.forEach((node) => upsertNodeRender(node));
    dirtyRegion.flush(deps.getLeafer(), false);
    if (forceLeafer) deps.updateLeafer();
  }

  async function flushPendingRenderWork(): Promise<void> {
    await (pendingStreamRedrawDone ?? Promise.resolve());
    if (pendingNodeBuffer.length > 0 || pendingBufferTimer) {
      flushNodeBuffer(true);
    }
    await (pendingViewportRedrawDone ?? Promise.resolve());
  }

  function hasPendingRenderWork(): boolean {
    return Boolean(
      pendingStreamPatch ||
        pendingStreamFrame ||
        pendingStreamRedrawDone ||
        pendingViewportRedrawFrame ||
        pendingViewportRedrawDone ||
        pendingNodeBuffer.length > 0 ||
        pendingBufferTimer,
    );
  }

  function getInteractionState(): GraphSceneInteractionState {
    return {
      hasGraphData: !!lastGraphData,
      nodeCount: lastGraphData?.nodes.length ?? 0,
      rootProbeCount: deps.getClickTargetProbes().length,
      pendingRenderWork: hasPendingRenderWork(),
    };
  }

  function scheduleNodeBufferFlush(): void {
    if (pendingBufferTimer) return;
    pendingBufferTimer = setTimeout(() => {
      pendingBufferTimer = null;
      flushNodeBuffer(true);
    }, 200);
  }


  function flushStreamPatch(patch: PendingStreamPatch): void {
    performance.mark('pipeline:flush-stream-patch:start');
    const previousViewData = lastGraphData;
    const graphData = streamStateToArrays(streamState);
    dirtyRegion.reset();
    if (patch.clear) {
      inferGraphPaths(graphData.nodes, graphData.edges);
      markViewDirty(previousViewData);
      beginFullSceneReplace(graphData);
      graphData.nodes.forEach((node) => {
        renderNodeIntoScene(node);
      });
      markViewDirty(graphData);
      finalizeSceneFrame(graphData);
      performance.mark('pipeline:flush-stream-patch:end');
      performance.measure('pipeline:flush-stream-patch', 'pipeline:flush-stream-patch:start', 'pipeline:flush-stream-patch:end');
      return;
    }
    // Incremental: nodes already have paths from Core delta, skip O(G+E) inferGraphPaths.
    const previousNodeById = new Map(
      (previousViewData?.nodes ?? []).map((node) => [node.renderHandle, node]),
    );
    const nodeById = new Map(graphData.nodes.map((node) => [node.renderHandle, node]));
    patch.nodeIds.forEach((nodeId) => {
      markNodeDirty(previousNodeById.get(nodeId));
      const node = nodeById.get(nodeId);
      if (node) {
        if (patch.tablePatchNodeIds.has(nodeId)) {
          upsertNodeRender(node, patch.tablePatchModes.get(nodeId));
          return;
        }
        // Buffer node for deferred Leafer scene creation (200ms batch).
        removeNodeRender(node.renderHandle);
        pendingNodeBuffer.push(node);
        return;
      }
      removeNodeRender(nodeId);
    });
    if (patch.edgeChange) {
      markViewDirty(previousViewData);
      markViewDirty(graphData);
    }
    finalizeSceneFrame(graphData, !patch.edgeChange, true);
    scheduleNodeBufferFlush();
    performance.mark('pipeline:flush-stream-patch:end');
    performance.measure('pipeline:flush-stream-patch', 'pipeline:flush-stream-patch:start', 'pipeline:flush-stream-patch:end');
  }

  function scheduleStreamFrame(): Promise<void> {
    if (pendingStreamFrame) {
      return pendingStreamRedrawDone ?? Promise.resolve();
    }
    pendingStreamRedrawDone = new Promise<void>((resolve) => {
      resolvePendingStreamRedraw = resolve;
    });
    pendingStreamFrame = requestAnimationFrame(() => {
      pendingStreamFrame = null;
      const patch = pendingStreamPatch;
      pendingStreamPatch = null;
      if (patch) {
        flushStreamPatch(patch);
      }
      resolvePendingStreamRedraw?.();
      resolvePendingStreamRedraw = null;
      pendingStreamRedrawDone = null;
    });
    return pendingStreamRedrawDone;
  }

  function scheduleViewportRedraw(): Promise<void> {
    if (pendingViewportRedrawFrame) return pendingViewportRedrawDone ?? Promise.resolve();
    pendingViewportRedrawDone = new Promise<void>((resolve) => {
      resolvePendingViewportRedraw = resolve;
    });
    pendingViewportRedrawFrame = requestAnimationFrame(() => {
      pendingViewportRedrawFrame = null;
      const graphData = lastGraphData;
      if (graphData) {
        lastGraphData = {
          nodes: graphData.nodes,
          edges: drawEdges(graphData.nodes, graphData.edges),
        };
      }
      resolvePendingViewportRedraw?.();
      resolvePendingViewportRedraw = null;
      pendingViewportRedrawDone = null;
    });
    return pendingViewportRedrawDone;
  }

  function replaceAll(graphData: GraphSceneViewData): GraphSceneViewData {
    replaceStreamState(streamState, graphData);
    dirtyRegion.reset();
    ensureLayers();
    inferGraphPaths(graphData.nodes, graphData.edges);
    markViewDirty(lastGraphData);
    beginFullSceneReplace(graphData);
    graphData.nodes.forEach((node) => {
      renderNodeIntoScene(node);
    });
    markViewDirty(graphData);
    lastGraphData = finalizeSceneFrame(graphData);
    return lastGraphData;
  }

  function applyGraphDelta(
    delta: GraphSceneDelta,
    version?: { baseGraphVersion: number; graphVersion: number },
  ): Promise<void> {
    if (version) {
      applyVersionedProjection(streamState, delta, version);
    } else {
      applyGraphDeltaToState(delta, streamState);
    }
    const nextPatch = createPendingStreamPatch(delta);
    if (!pendingStreamPatch) {
      pendingStreamPatch = nextPatch;
      return scheduleStreamFrame();
    }
    mergePendingStreamPatch(pendingStreamPatch, nextPatch);
    return scheduleStreamFrame();
  }
  function updateViewport(): Promise<void> {
    return scheduleViewportRedraw();
  }

  function clear(): void {
    cancelActiveRenderWork();
    clearStreamState(streamState);
    dirtyRegion.reset();
    pendingStreamPatch = null;
    lastGraphData = null;
    clearRenderedMainGraph();
    deps.updateLeafer();
  }

  function cancelActiveRenderWork(): void {
    cancelPendingNodeBuffer();
    if (pendingStreamFrame) cancelAnimationFrame(pendingStreamFrame);
    pendingStreamFrame = null;
    resolvePendingStreamRedraw?.();
    resolvePendingStreamRedraw = null;
    pendingStreamRedrawDone = null;
    if (pendingViewportRedrawFrame) cancelAnimationFrame(pendingViewportRedrawFrame);
    pendingViewportRedrawFrame = null;
    resolvePendingViewportRedraw?.();
    resolvePendingViewportRedraw = null;
    pendingViewportRedrawDone = null;
  }

  function getLastGraphData(): GraphSceneViewData | null {
    return lastGraphData;
  }

  function setLastViewData(value: GraphSceneViewData | null): void {
    lastGraphData = value;
    if (value) {
      replaceStreamState(streamState, value);
      return;
    }
    clearStreamState(streamState);
  }

  function dispose(): void {
    cancelActiveRenderWork();
  }

  function scrollTableToRow(nodeIdOrRowIndex: number, rowIndex?: number): boolean {
    if (rowIndex == null) {
      let scrolled = false;
      for (const [, runtime] of tableRuntimeByNodeId) {
        const virtualList = (runtime as InternalTableRuntime).virtualList;
        if (!virtualList) continue;
        virtualList.scrollToIndex(nodeIdOrRowIndex);
        runtime.updateWindow?.({ forceRebindVisibleRows: true });
        scrolled = true;
      }
      if (scrolled) deps.updateLeafer();
      return scrolled;
    }
    const runtime = tableRuntimeByNodeId.get(nodeIdOrRowIndex) as InternalTableRuntime | undefined;
    const virtualList = runtime?.virtualList;
    if (!virtualList) return false;
    virtualList.scrollToIndex(rowIndex);
    runtime.updateWindow?.({ forceRebindVisibleRows: true });
    deps.updateLeafer();
    return true;
  }



  return {
    clear,
    inferGraphPaths,
    ensureLayers,
    drawEdges,
    replaceAll,
    applyGraphDelta,
    updateViewport,
    flushPendingRenderWork,
    getInteractionState,
    scheduleViewportRedraw,
    cancelActiveRenderWork,
    getLastGraphData,
    setLastViewData,
    dispose,
    scrollTableToRow,
  };
}
