import { tick } from 'svelte';
import { serializePath } from '../../../shared/document-anchor-utils';
import { callSharedWasmWorker } from '../../wasm/wasm-worker-singleton';
import type { DocumentProjectionDelta, SnapshotId, SnapshotReadResult } from '@core-wasm/index';
import type { GraphViewerConfig } from '../../settings/ui-settings';
import type { SupportedEditorLanguageId } from '../../monaco/language-support';
import { buildReadablePath, isPathSegKey, pathSegKeyValue, type PathSeg } from '../../store/tree-path';
import type { DrawContext, GraphCell, GraphCellKind, GraphEdge, GraphNode } from '@treease/graph-viewer-runtime';
import { renderGraphEdges, renderGraphNode } from '@treease/graph-viewer-runtime';
import {
  GRAPH_PAN_CONSTRAINT_PADDING,
  clampPanOffsetToGraphBounds,
  getZoomScale,
  type GraphWorldBounds,
} from '@treease/graph-viewer-runtime';
import type {
  LeaferAppLike,
  LeaferBox,
  LeaferEditor,
  LeaferEditorHost,
  LeaferText,
  SubgraphWorkspaceRuntime,
} from './model';
import { createCellEntryBindings } from './graph-anchor-index';
import type { SubgraphWorkspaceGraphData } from './graph-subgraph-workspace-types';
import { isRawGraphDelta } from '../../../shared/worker-protocol/graph-delta-normalize';
import { normalizeGraphDelta } from '../../../shared/worker-protocol/graph-stream-event-codec';

type GraphCacheEntry = {
  signature: string;
  accessOrder: number;
  graph?: SubgraphWorkspaceGraphData | null;
  promise?: Promise<SubgraphWorkspaceGraphData | null>;
};

type GraphCacheDeps = {
  getActiveSnapshotId: () => SnapshotId | null;
  getDocumentKey: () => string;
  getLanguageId: () => SupportedEditorLanguageId;
  getRevision: () => number;
  getEnableNest: () => boolean;
  getRenderConfig: () => GraphViewerConfig;
  inferGraphPaths: (nodes: GraphNode[], edges: GraphEdge[]) => void;
};

type WorkspaceRuntimeDeps = {
  getConstructors: () => {
    LeaferCtor?: new (...args: any[]) => LeaferAppLike;
    PlainLeaferCtor?: new (...args: any[]) => LeaferAppLike;
    BoxCtor?: new (...args: any[]) => LeaferBox;
    TextCtor?: new (...args: any[]) => LeaferText;
    PenCtor?: new () => {
      setStyle: (style: Record<string, unknown>) => void;
      moveTo: (x: number, y: number) => void;
      bezierCurveTo: (c1x: number, c1y: number, c2x: number, c2y: number, toX: number, toY: number) => void;
    };
  };
  getRenderConfig: () => GraphViewerConfig;
  getLanguageId: () => SupportedEditorLanguageId;
  /** @deprecated Workspace rendering reads GraphCell.semType. */
  getValueTypeToSemType?: () => Record<string, string>;
  isReadonly?: () => boolean;
  bindGraphEditorLifecycle: (editor: LeaferEditor | null) => void;
  bindPointerClick: (target: LeaferBox, handler: (event: unknown) => void | Promise<void>) => void;
  getMoveEventName?: () => string | undefined;
  bindVerticalScrollGesture?: (
    target: LeaferBox,
    handler: (gesture: { event: unknown; deltaY: number; moveType?: string; stop: () => void; stopNow: () => void }) => void,
  ) => (() => void) | void;
  bindPointerDown?: (target: LeaferBox, handler: (event: unknown) => void | Promise<void>) => (() => void) | void;
  getPointFromEvent?: (
    hostApp: LeaferAppLike | null,
    target: LeaferBox,
    event: unknown,
    space: 'client' | 'box' | 'local' | 'world',
  ) => { x: number; y: number } | null;
  resolveInteractiveCellPath: (cell: GraphCell, fallbackPath: PathSeg[]) => Promise<PathSeg[]>;
  onActivateCell: (payload: { path: PathSeg[]; target: 'key' | 'value' | 'node'; cell: GraphCell }) => void | Promise<void>;
};

const graphCacheLimit = 24;

function buildMetaPathText(path: PathSeg[]): string {
  const readable = buildReadablePath(path);
  if (readable === '$') return readable;
  if (readable.startsWith('$.')) return readable.slice(2);
  if (readable.startsWith('$[')) return readable.slice(1);
  return readable;
}

export function rebaseSubgraphWorkspacePath(basePath: PathSeg[], path: PathSeg[]): PathSeg[] {
  if (!basePath.length) return path;
  if (!path.length) return basePath;
  const pathKey = buildWorkspacePathKey(path);
  const basePathKey = buildWorkspacePathKey(basePath);
  return pathKey === basePathKey || pathKey.startsWith(`${basePathKey}|`) ? path : [...basePath, ...path];
}

export function formatSubgraphWorkspacePath(path: PathSeg[]): string {
  return buildMetaPathText(path);
}

export function shouldIgnoreSubgraphOpenCell(cell: GraphCell | null | undefined): boolean {
  return cell?.isMissing === true;
}

export function shouldOpenSubgraphWorkspaceContent(value: {
  valueType?: string | null;
  displayText?: string | null;
}): boolean {
  if (value.valueType !== 'object' && value.valueType !== 'array') return true;
  return value.displayText === '{}' || value.displayText === '[]';
}

function buildWorkspacePathKey(path: PathSeg[]): string {
  if (!path.length) return '';
  return path.map((segment) => (isPathSegKey(segment) ? `k:${pathSegKeyValue(segment)}` : `i:${segment.index}`)).join('|');
}

export function buildSubgraphWorkspaceRenderSignature(renderConfig: GraphViewerConfig): string {
  return [
    renderConfig.columns.keyColumnMaxWidth,
    renderConfig.columns.valueColumnMaxWidth,
    renderConfig.layout.rowHeight,
    renderConfig.layout.layerGapX,
    renderConfig.layout.layerGapY,
    renderConfig.layout.tableMaxHeight,
    renderConfig.layout.tableRowHeight,
    renderConfig.layout.tableHeaderHeight,
  ].join('|');
}

function buildGraphCacheSignature(deps: GraphCacheDeps): string {
  const snapshotId = deps.getActiveSnapshotId();
  return [
    deps.getDocumentKey(),
    snapshotId ?? 'no-snapshot',
    deps.getLanguageId(),
    deps.getRevision(),
    deps.getEnableNest() ? 'nest' : 'flat',
    buildSubgraphWorkspaceRenderSignature(deps.getRenderConfig()),
  ].join('|');
}

export function normalizeWorkspaceGraphEdgeRows(nodes: GraphNode[], edges: GraphEdge[]): GraphEdge[] {
  if (!nodes.length || !edges.length) return edges;
  const headeredTableHandles = new Set(
    nodes
      .filter((node) => node.kind === 'table' && node.table && (node.table.headerHeight ?? 0) > 0 && (node.table.rows?.length ?? 0) > 0)
      .map((node) => node.renderHandle),
  );
  if (!headeredTableHandles.size) return edges;

  const collectHandlesNeedingOffset = (
    readHandle: (edge: GraphEdge) => number,
    readRow: (edge: GraphEdge) => number,
  ): Set<number> =>
    new Set(
      edges
        .filter((edge) => readRow(edge) === 0 && headeredTableHandles.has(readHandle(edge)))
        .map((edge) => readHandle(edge)),
    );

  const normalizeSourceHandles = collectHandlesNeedingOffset(
    (edge) => edge.fromRenderHandle,
    (edge) => edge.fromRow,
  );
  const normalizeTargetHandles = collectHandlesNeedingOffset(
    (edge) => edge.toRenderHandle,
    (edge) => edge.toRow,
  );
  if (!normalizeSourceHandles.size && !normalizeTargetHandles.size) return edges;

  return edges.map((edge) => ({
    ...edge,
    fromRow: normalizeSourceHandles.has(edge.fromRenderHandle) ? edge.fromRow + 1 : edge.fromRow,
    toRow: normalizeTargetHandles.has(edge.toRenderHandle) ? edge.toRow + 1 : edge.toRow,
  }));
}

function buildWorkspaceGraphData(
  deps: GraphCacheDeps,
  path: PathSeg[],
  pathKey: string,
  result: { nodes?: GraphNode[]; edges?: GraphEdge[] },
): SubgraphWorkspaceGraphData | null {
  const nodes = result.nodes ?? [];
  const edges = normalizeWorkspaceGraphEdgeRows(nodes, result.edges ?? []);
  if (!nodes.length) return null;
  deps.inferGraphPaths(nodes, edges);
  const minX = Math.min(...nodes.map((node) => node.boxArgs.x));
  const minY = Math.min(...nodes.map((node) => node.boxArgs.y));
  const maxX = Math.max(...nodes.map((node) => node.boxArgs.x + node.boxArgs.width));
  const maxY = Math.max(...nodes.map((node) => node.boxArgs.y + node.boxArgs.height));
  return {
    path,
    pathKey,
    nodes,
    edges,
    minX,
    minY,
    width: Math.max(0, maxX - minX),
    height: Math.max(0, maxY - minY),
  };
}

export function createSubgraphWorkspaceGraphCache(deps: GraphCacheDeps) {
  const graphCache = new Map<string, GraphCacheEntry>();
  let graphCacheSignature = '';
  let graphCacheAccessOrder = 0;

  function ensureGraphCacheSignature(): void {
    const signature = buildGraphCacheSignature(deps);
    if (graphCacheSignature === signature) return;
    graphCache.clear();
    graphCacheSignature = signature;
  }

  function touchGraphCacheEntry(entry: GraphCacheEntry): void {
    graphCacheAccessOrder += 1;
    entry.accessOrder = graphCacheAccessOrder;
  }

  function trimGraphCache(): void {
    if (graphCache.size <= graphCacheLimit) return;
    const removable = [...graphCache.entries()]
      .filter(([, entry]) => !entry.promise)
      .sort((left, right) => left[1].accessOrder - right[1].accessOrder);
    while (graphCache.size > graphCacheLimit && removable.length) {
      const next = removable.shift();
      if (!next) break;
      graphCache.delete(next[0]);
    }
  }

  async function prepareGraph(path: PathSeg[]): Promise<SubgraphWorkspaceGraphData | null> {
    ensureGraphCacheSignature();
    const pathKey = buildWorkspacePathKey(path);
    const signature = graphCacheSignature;
    const current = graphCache.get(pathKey);
    if (current && current.signature === signature) {
      touchGraphCacheEntry(current);
      if (current.graph !== undefined) return current.graph;
      if (current.promise) return current.promise;
    }
    const entry: GraphCacheEntry = { signature, accessOrder: 0 };
    touchGraphCacheEntry(entry);
    entry.promise = Promise.resolve().then(async () => {
      const snapshotId = deps.getActiveSnapshotId();
      if (snapshotId == null) return null;
      const projection = await callSharedWasmWorker<SnapshotReadResult<DocumentProjectionDelta>>('buildHoverSubgraphProjection', {
        documentKey: deps.getDocumentKey(),
        snapshotId,
        path: serializePath(path),
      });
      if (projection.status !== 'ready' || !projection.data.graphData) return null;
      const delta = { ...projection.data.graphData, clear: projection.data.clear ? 1 : 0 };
      if (!isRawGraphDelta(delta)) {
        throw new Error('subgraph workspace projection decode failed');
      }
      const normalized = normalizeGraphDelta(delta);
      return buildWorkspaceGraphData(deps, path, pathKey, {
        nodes: normalized.nodesAdded as unknown as GraphNode[],
        edges: normalized.edgesAdded as unknown as GraphEdge[],
      });
    }).then((graph) => {
      const cached = graphCache.get(pathKey);
      if (!cached || cached.signature !== signature) return graph;
      cached.graph = graph;
      cached.promise = undefined;
      touchGraphCacheEntry(cached);
      trimGraphCache();
      return graph;
    }).catch((error) => {
      graphCache.delete(pathKey);
      throw error;
    });
    graphCache.set(pathKey, entry);
    trimGraphCache();
    return entry.promise;
  }

  return {
    clear: () => {
      graphCache.clear();
      graphCacheSignature = '';
    },
    prepareGraph,
  };
}

function createWorkspaceApp(view: HTMLDivElement, deps: WorkspaceRuntimeDeps): LeaferAppLike {
  const { LeaferCtor, PlainLeaferCtor } = deps.getConstructors();
  const AppCtor = LeaferCtor ?? PlainLeaferCtor;
  return new AppCtor!({
    view,
    type: 'viewport',
    editor: {
      visible: true,
      hittable: true,
      hover: false,
      moveable: false,
      resizeable: false,
      rotateable: false,
      skewable: false,
      flipable: false,
    },
    move: { drag: false, holdSpaceKey: true, holdRightKey: true, scroll: true },
    zoom: { disabled: false },
    wheel: { zoomMode: false },
    multiTouch: { disabled: false },
  });
}

function getLeaferContentRoot(target: LeaferAppLike | null): LeaferBox | null {
  if (!target) return null;
  const zoomLayer = target.zoomLayer as LeaferBox | undefined;
  if (zoomLayer) return zoomLayer;
  return target as unknown as LeaferBox;
}

export function destroySubgraphWorkspaceRuntime(runtime: SubgraphWorkspaceRuntime | null | undefined): void {
  if (!runtime) return;
  runtime.editor?.closeInnerEditor?.(true);
  runtime.tableRuntimes.forEach((tableRuntime) => tableRuntime.destroy?.());
  runtime.tableRuntimes = [];
  runtime.dispose?.();
  runtime.app.destroy?.();
  runtime.mount.replaceChildren();
}

function buildWorkspacePanConstraintBounds(graph: SubgraphWorkspaceGraphData): GraphWorldBounds {
  return {
    left: graph.minX,
    top: graph.minY,
    right: graph.minX + graph.width,
    bottom: graph.minY + graph.height,
  };
}

function clampWorkspaceViewportPan(app: LeaferAppLike, mount: HTMLDivElement, graph: SubgraphWorkspaceGraphData): void {
  const layer = app.zoomLayer as LeaferBox | undefined;
  if (!layer) return;
  const { scaleX, scaleY } = getZoomScale(layer as never);
  const clamped = clampPanOffsetToGraphBounds(
    {
      viewportWidth: Math.max(1, mount.clientWidth),
      viewportHeight: Math.max(1, mount.clientHeight),
      scaleX,
      scaleY,
      offsetX: Number(layer.x ?? 0),
      offsetY: Number(layer.y ?? 0),
    },
    buildWorkspacePanConstraintBounds(graph),
    GRAPH_PAN_CONSTRAINT_PADDING,
  );
  layer.x = clamped.x;
  layer.y = clamped.y;
}

function resizeWorkspaceViewport(
  app: LeaferAppLike,
  mount: HTMLDivElement,
  graph: SubgraphWorkspaceGraphData,
  contentWidth: number,
  contentHeight: number,
  options?: { resetTransform?: boolean },
): void {
  const viewportWidth = Math.max(1, Math.floor(mount.clientWidth));
  const viewportHeight = Math.max(1, Math.floor(mount.clientHeight));
  app.resize?.({ width: viewportWidth, height: viewportHeight });

  const rootViewport = getLeaferContentRoot(app) as
    | (LeaferBox & {
        x?: number;
        y?: number;
        scaleX?: number;
        scaleY?: number;
      })
    | null;
  if (rootViewport) {
    if (options?.resetTransform) {
      rootViewport.scaleX = 1;
      rootViewport.scaleY = 1;
      rootViewport.x = contentWidth < viewportWidth ? Math.max(0, (viewportWidth - contentWidth) / 2) : 0;
      rootViewport.y = contentHeight < viewportHeight ? Math.max(0, (viewportHeight - contentHeight) / 2) : 0;
      mount.dataset.viewportScale = '1';
    } else {
      clampWorkspaceViewportPan(app, mount, graph);
      if (contentWidth < viewportWidth) {
        rootViewport.x = Math.max(0, (viewportWidth - contentWidth) / 2);
      }
      if (contentHeight < viewportHeight) {
        rootViewport.y = Math.max(0, (viewportHeight - contentHeight) / 2);
      }
    }
  }

  app.updateClientBounds?.();
  app.update?.();
}

export async function renderSubgraphWorkspaceGraph(
  mount: HTMLDivElement,
  graph: SubgraphWorkspaceGraphData,
  deps: WorkspaceRuntimeDeps,
): Promise<SubgraphWorkspaceRuntime | null> {
  const { BoxCtor, TextCtor, PenCtor } = deps.getConstructors();
  if (!BoxCtor || !TextCtor || !PenCtor) return null;
  const view = document.createElement('div');
  view.className = 'graph-subgraph-pane-view';
  view.style.width = '100%';
  view.style.height = '100%';
  mount.replaceChildren(view);
  const app = createWorkspaceApp(view, deps);
  const root = getLeaferContentRoot(app);
  if (!root) {
    app.destroy?.();
    return null;
  }
  const edgeLayer = new BoxCtor({ x: 0, y: 0, width: 0, height: 0, fill: 'transparent' });
  const nodeLayer = new BoxCtor({ x: 0, y: 0, width: 0, height: 0, fill: 'transparent' });
  root.add?.(edgeLayer);
  root.add?.(nodeLayer);
  const runtime: SubgraphWorkspaceRuntime = {
    host: mount,
    mount,
    view,
    app,
    edgeLayer,
    nodeLayer,
    pathKey: graph.pathKey,
    path: graph.path,
    clickTargetsById: Object.create(null),
    clickTargetIdByTarget: new WeakMap(),
    clickBoundTargets: new WeakSet(),
    cellBoxByPathMap: new Map(),
    nodeBoxMap: new Map(),
    tableRuntimes: [],
    editor: (app as LeaferEditorHost).editor ?? null,
  };
  deps.bindGraphEditorLifecycle(runtime.editor);
  const cellEntryBindings = createCellEntryBindings(runtime.cellBoxByPathMap);
  let nextClickTargetSeq = 0;
  const registerWorkspaceProbe = (targetBox: LeaferBox, targetCell: GraphCell, kind: GraphCellKind): string => {
    const existingId = runtime.clickTargetIdByTarget?.get(targetBox);
    const id = existingId ?? `${targetBox.tag === 'Text' ? 'text' : 'node'}-${nextClickTargetSeq++}`;
    runtime.clickTargetIdByTarget?.set(targetBox, id);
    runtime.clickTargetsById[id] = {
      id,
      box: targetBox,
      cell: targetCell,
      target: kind === 'key' ? 'key' : kind === 'value' ? 'value' : 'node',
    };
    return id;
  };
  const bindWorkspaceTarget = (targetBox: LeaferBox, targetCell: GraphCell, kind: GraphCellKind): string => {
    targetBox.__graphCell = targetCell;
    targetBox.__graphCellKind = kind;
    const targetId = registerWorkspaceProbe(targetBox, targetCell, kind);
    if (runtime.clickBoundTargets?.has(targetBox)) return targetId;
    runtime.clickBoundTargets?.add(targetBox);
    deps.bindPointerClick(targetBox, async () => {
      const cell = targetBox.__graphCell as GraphCell | undefined;
      const cellKind = (targetBox.__graphCellKind as GraphCellKind | undefined) ?? kind;
      if (!cell) return;
      const target = cellKind === 'key' ? 'key' : cellKind === 'value' ? 'value' : 'node';
      const fallbackPath = cell.path ?? [];
      const resolvedPath = target === 'node' ? fallbackPath : await deps.resolveInteractiveCellPath(cell, fallbackPath);
      const path = rebaseSubgraphWorkspacePath(graph.path, resolvedPath);
      if (!path.length) return;
      await deps.onActivateCell({ path, target, cell });
    });
    return targetId;
  };
  const drawContext: DrawContext = {
    nodeLayer: runtime.nodeLayer,
    styleConfig: deps.getRenderConfig(),
    languageIdValue: deps.getLanguageId(),
    fontSize: deps.getRenderConfig().layout.baseFontSize,
    BoxCtor,
    TextCtor,
    PenCtor,
    editable: deps.isReadonly?.() ? false : true,
    registerCellBox: cellEntryBindings.registerCellBox,
    unregisterCellBox: cellEntryBindings.unregisterCellBox,
    registerRowBox: cellEntryBindings.registerRowBox,
    unregisterRowBox: cellEntryBindings.unregisterRowBox,
    registerClickTarget: (targetBox, targetCell, kind) => bindWorkspaceTarget(targetBox as LeaferBox, targetCell, kind),
    requestRender: () => runtime.app.update?.(),
    bindVerticalScrollGesture: deps.bindVerticalScrollGesture,
    bindPointerDown: deps.bindPointerDown,
    getPointFromEvent: (hostApp, target, event, space) =>
      deps.getPointFromEvent?.((hostApp as LeaferAppLike | null) ?? runtime.app, target as LeaferBox, event, space) ?? null,
  };
  const padding = 16;
  const contentWidth = Math.max(1, graph.width + padding * 2);
  const contentHeight = Math.max(1, graph.height + padding * 2);
  runtime.edgeLayer.x = padding - graph.minX;
  runtime.edgeLayer.y = padding - graph.minY;
  runtime.nodeLayer.x = padding - graph.minX;
  runtime.nodeLayer.y = padding - graph.minY;
  graph.nodes.forEach((node) => {
    const result = renderGraphNode({
      node,
      drawContext,
      showMeta: false,
      registerMetaClickTarget: (target, targetCell, kind) => {
        bindWorkspaceTarget(target as LeaferBox, targetCell, kind);
      },
    });
    if (result.nodeBox) runtime.nodeBoxMap.set(node.renderHandle, result.nodeBox);
    if (result.tableRuntime) runtime.tableRuntimes.push(result.tableRuntime as { destroy: () => void });
  });
  renderGraphEdges({
    nodes: graph.nodes,
    edges: graph.edges,
    layer: runtime.edgeLayer,
    PenCtor,
    renderConfig: deps.getRenderConfig(),
  });
  const resizeObserver =
    typeof ResizeObserver === 'undefined'
      ? null
      : new ResizeObserver(() => {
          resizeWorkspaceViewport(app, mount, graph, contentWidth, contentHeight);
        });
  resizeObserver?.observe(mount);
  runtime.dispose = () => {
    resizeObserver?.disconnect();
  };
  resizeWorkspaceViewport(app, mount, graph, contentWidth, contentHeight, { resetTransform: true });
  await tick();
  const moveEventName = deps.getMoveEventName?.();
  if (moveEventName && typeof app.on === 'function') {
    app.on(moveEventName, () => {
      clampWorkspaceViewportPan(app, mount, graph);
      app.update?.();
    });
  }
  return runtime;
}
