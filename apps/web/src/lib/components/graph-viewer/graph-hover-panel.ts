// 职责：Graph hover panel 控制器入口：组合 layout/prewarm/cache/runtime 子模块，管理 tooltip 生命周期
import { tick } from 'svelte';
import type { GraphViewerConfig } from '../../settings/ui-settings';
import { serializePath } from '../../../shared/document-anchor-utils';
import { type PathSeg } from '../../store/tree-path';
import {
  type DrawContext,
  type GraphCell,
  type GraphCellKind,
  type GraphEdge,
  type GraphNode,
} from '../../graph/graph-viewer-render';
import type { SupportedEditorLanguageId } from '../../monaco/language-support';
import { renderGraphEdges, renderGraphNode } from './graph-render-kernel';
import { buildPathKey, buildTooltipContent } from '../../graph/graph-viewer-path';
import { callSharedWasmWorker } from '../../wasm/wasm-worker-singleton';
import type { DocumentProjectionDelta, SnapshotId, SnapshotReadResult } from '@core-wasm/index';
import { TooltipPlugin } from '../../leafer-x-tooltip';
import type {
  CellBoxEntry,
  GraphViewerClickTarget,
  LeaferAppLike,
  LeaferBox,
  LeaferEditor,
  LeaferEditorHost,
  LeaferText,
  TooltipPanelRuntime,
} from './model';
import { createCellEntryBindings, resolveCellPath } from './graph-anchor-index';
import type { GraphRuntimeHoverPanelDebugState, GraphRuntimeHoverPreviewState } from './runtime/scene-types';
import './tooltip-text-editor';
import type {
  GraphHoverPreviewTarget,
  TooltipPanelGraphCacheEntry,
  TooltipPanelGraphData,
  TooltipPanelPrewarmDebugSnapshot,
} from './graph-hover-panel-types';
import {
  buildTooltipPanelAppConfig,
  resolveTooltipPanelViewportSize,
  tooltipPanelContainerSelector,
  tooltipPanelMaxViewportHeight,
  tooltipPanelMaxViewportWidth,
  tooltipPanelViewportMargin,
} from './graph-hover-panel-layout';
import {
  collectTooltipPanelPrewarmCandidatesFromClickTargets,
  isEmptyCompositeCell,
  tooltipPanelPrewarmLimit,
} from './graph-hover-panel-prewarm';
import { isRawGraphDelta } from '../../../shared/worker-protocol/graph-delta-normalize';
import { normalizeGraphDelta } from '../../../shared/worker-protocol/graph-stream-event-codec';

export type { GraphHoverPreviewKind, GraphHoverPreviewTarget } from './graph-hover-panel-types';
export { buildGraphTooltipPanelShellMarkup, buildTooltipPanelAppConfig, resolveTooltipPanelViewportSize } from './graph-hover-panel-layout';
export { collectTooltipPanelPrewarmCandidates } from './graph-hover-panel-prewarm';


type HoverPanelControllerDeps = {
  getCurrentData: () => unknown;
  getLanguageId: () => SupportedEditorLanguageId;
  getActiveSnapshotId: () => SnapshotId | null;
  getSourceText: () => string;
  getDocumentKey: () => string;
  getRevision: () => number;
  getEnableNest: () => boolean;
  getRenderConfig: () => GraphViewerConfig;
  getSettings: () => unknown;
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
  getTooltipEvents: () => {
    LeaferEvent: any;
    PointerEvent: any;
  };
  getValueTypeToSemType: () => Record<string, string>;
  isReadonly?: () => boolean;
  getRootClickTargets: () => GraphViewerClickTarget[];
  bindGraphEditorLifecycle: (editor: LeaferEditor | null) => void;
  canOpenSubgraphPreviewForCell: (cell: GraphCell, target: 'key' | 'value' | 'node') => boolean;
  resolveTreePathByPosition: (row: number, column: number) => Promise<PathSeg[]>;
  resolveInteractiveCellPath: (cell: GraphCell, fallbackPath: PathSeg[]) => Promise<PathSeg[]>;
  inferGraphPaths: (nodes: GraphNode[], edges: GraphEdge[]) => void;
  upsertCellEntry: (map: Map<string, CellBoxEntry>, cell: GraphCell, updater: (entry: CellBoxEntry) => void) => void;
  updateCellEntry: (map: Map<string, CellBoxEntry>, cell: GraphCell, updater: (entry: CellBoxEntry) => void) => void;
  registerPanelClickTarget: (
    store: TooltipPanelRuntime['clickTargetsById'],
    targetIds: WeakMap<object, string>,
    boundTargets: WeakSet<object>,
    box: LeaferBox,
    cell: GraphCell,
    kind: GraphCellKind,
    nodeKind?: GraphNode['kind'],
  ) => string;
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
  refreshTooltipPosition: () => void;
  setRuntimeHoverPreviewState: (preview: GraphRuntimeHoverPreviewState | null) => void;
  setRuntimeHoverPanelDebugState: (state: GraphRuntimeHoverPanelDebugState) => void;
  handleError: (
    error: unknown,
    context: { component: string; operation: string; metadata?: Record<string, unknown> },
  ) => void;
};

const tooltipPanelGraphCacheLimit = 24;

function resolveGraphHoverPanelTarget(kind: GraphCellKind | null): 'key' | 'value' | 'node' | null {
  if (kind === 'key') return 'key';
  if (kind === 'value') return 'value';
  if (kind === 'meta' || kind === 'header') return 'node';
  return null;
}

export function isGraphHoverTargetOverflowing(target: LeaferText | null): boolean {
  return Boolean(target && 'isOverflow' in target && (target as { isOverflow?: boolean }).isOverflow);
}
type GraphHoverPreviewCandidate = Pick<LeaferText, '__graphCell' | '__graphCellKind' | '__graphNodeKind'> & {
  isOverflow?: boolean;
};

export function canOpenSubgraphPreviewForCell(cell: GraphCell, target: 'key' | 'value' | 'node'): boolean {
  if (cell.isHeader) return false;
  if (target === 'key') return true;
  if (target === 'node') return !!cell.path?.length;
  if (cell.valueType !== 'object' && cell.valueType !== 'array') return true;
  if (!cell.isTableCell) return false;
  if (cell.isHeaderlessTable && !cell.isScrollableTable) return false;
  if (isEmptyCompositeCell(cell)) return false;
  return true;
}

export function resolveGraphHoverPreviewRule(
  target: GraphHoverPreviewCandidate | null,
  canOpenSubgraphPreviewForCell: (cell: GraphCell, target: 'key' | 'value' | 'node') => boolean,
): GraphHoverPreviewTarget | null {
  const cell = target?.__graphCell ?? null;
  const kind = target?.__graphCellKind ?? null;
  if (!cell || !kind || kind === 'header') return null;
  if (cell.isHeaderlessTable && !cell.isScrollableTable) return null;
  const panelTarget = resolveGraphHoverPanelTarget(kind);
  const cellWidth = cell.boxArgs?.width ?? 0;
  const textLen = cell.text?.length ?? 0;
  const fontSize = (target as { fontSize?: number })?.fontSize ?? 12;
  // Approximate: if text char count >= cell width / font_size, text must overflow.
  const approxOverflow = cellWidth > 0 && textLen > 0 && textLen >= cellWidth / fontSize;
  const overflowPreview: GraphHoverPreviewTarget | null =
    (target?.isOverflow || approxOverflow)
      ? { cell, target: panelTarget, previewKind: 'pre' }
      : null;
  if (kind === 'key' || kind === 'meta') return overflowPreview;
  if (panelTarget === 'node') return null;
  const isStructuredValue = cell.valueType === 'object' || cell.valueType === 'array';
  if (!isStructuredValue || isEmptyCompositeCell(cell)) {
    return overflowPreview;
  }
  if (!cell.isTableCell) {
    return overflowPreview;
  }
  return canOpenSubgraphPreviewForCell(cell, panelTarget)
    ? { cell, target: panelTarget, previewKind: 'subgraph' }
    : null;
}

export function createGraphHoverPanelController(deps: HoverPanelControllerDeps) {
  let tooltipPanelRuntime: TooltipPanelRuntime | null = null;
  let tooltipPanelRequestToken = 0;
  let tooltipPanelPendingPathKey = '';
  let tooltipPanelGraphCacheSignature = '';
  let tooltipPanelGraphCacheAccessOrder = 0;
  let tooltipPanelPrewarmToken = 0;
  let tooltipPanelPrewarmHandle: number | ReturnType<typeof setTimeout> | null = null;
  const tooltipPanelGraphCache = new Map<string, TooltipPanelGraphCacheEntry>();
  let tooltipPanelPrewarmScheduledPaths: PathSeg[][] = [];
  let tooltipPanelPrewarmCompletedPaths: PathSeg[][] = [];
  let tooltipPanelPrewarmInFlightPath: PathSeg[] | null = null;

  function clonePath(path: PathSeg[]): PathSeg[] {
    return path.map((segment) => ({ ...segment }));
  }


  async function ensureTooltipRuntime() {
    return;
  }

  function disposeTooltipEditor() {
    cancelTooltipPanelPrewarm();
    tooltipPanelGraphCache.clear();
    destroyTooltipPanelRuntime();
    deps.setRuntimeHoverPreviewState(null);
  }

  function applyTheme(settings: unknown) {
    void settings;
    clearTooltipPanelGraphCache();
  }

  function resolveGraphHoverPreviewTarget(target: LeaferText | null): GraphHoverPreviewTarget | null {
    return resolveGraphHoverPreviewRule(
      target as GraphHoverPreviewCandidate | null,
      deps.canOpenSubgraphPreviewForCell,
    );
  }

  function destroyTooltipPanelRuntime() {
    tooltipPanelRequestToken += 1;
    tooltipPanelPendingPathKey = '';
    deps.setRuntimeHoverPreviewState(null);
    const runtime = tooltipPanelRuntime;
    if (runtime) {
      runtime.editor?.closeInnerEditor?.(true);
      runtime.tableRuntimes.forEach((tableRuntime) => tableRuntime.destroy?.());
      runtime.tableRuntimes = [];
      runtime.dispose?.();
      runtime.app.destroy?.();
      runtime.mount.replaceChildren();
      tooltipPanelRuntime = null;
    }
  }

  function disposeTooltipPanelRuntimeForRemount(): void {
    const runtime = tooltipPanelRuntime;
    if (!runtime) return;
    runtime.tableRuntimes.forEach((tableRuntime) => tableRuntime.destroy?.());
    runtime.tableRuntimes = [];
    runtime.dispose?.();
    runtime.app.destroy?.();
    runtime.mount.replaceChildren();
    tooltipPanelRuntime = null;
  }

  function cancelTooltipPanelPrewarm(): void {
    tooltipPanelPrewarmToken += 1;
    tooltipPanelPrewarmInFlightPath = null;
    if (tooltipPanelPrewarmHandle != null) {
      const win = globalThis as typeof globalThis & {
        cancelIdleCallback?: (handle: number) => void;
        clearTimeout?: (handle: number) => void;
      };
      if (typeof win.cancelIdleCallback === 'function') {
        win.cancelIdleCallback(tooltipPanelPrewarmHandle as number);
      } else {
        win.clearTimeout?.(tooltipPanelPrewarmHandle);
      }
      tooltipPanelPrewarmHandle = null;
    }
  }

  function getTooltipPanelGraphCacheSignature(): string {
    const renderConfig = deps.getRenderConfig();
    const snapshotId = deps.getActiveSnapshotId();
    return [
      deps.getDocumentKey(),
      snapshotId ?? 'no-snapshot',
      deps.getLanguageId(),
      deps.getRevision(),
      deps.getEnableNest() ? 'nest' : 'flat',
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

  function clearTooltipPanelGraphCache(): void {
    cancelTooltipPanelPrewarm();
    tooltipPanelGraphCache.clear();
    tooltipPanelGraphCacheSignature = getTooltipPanelGraphCacheSignature();
  }

  function ensureTooltipPanelGraphCacheSignature(): void {
    const signature = getTooltipPanelGraphCacheSignature();
    if (tooltipPanelGraphCacheSignature === signature) return;
    tooltipPanelGraphCache.clear();
    tooltipPanelGraphCacheSignature = signature;
    cancelTooltipPanelPrewarm();
  }

  function touchTooltipPanelGraphCacheEntry(entry: TooltipPanelGraphCacheEntry): void {
    tooltipPanelGraphCacheAccessOrder += 1;
    entry.accessOrder = tooltipPanelGraphCacheAccessOrder;
  }

  function trimTooltipPanelGraphCache(): void {
    if (tooltipPanelGraphCache.size <= tooltipPanelGraphCacheLimit) return;
    const removable = [...tooltipPanelGraphCache.entries()]
      .filter(([, entry]) => !entry.promise)
      .sort((left, right) => left[1].accessOrder - right[1].accessOrder);
    while (tooltipPanelGraphCache.size > tooltipPanelGraphCacheLimit && removable.length) {
      const next = removable.shift();
      if (!next) break;
      tooltipPanelGraphCache.delete(next[0]);
    }
  }

  async function resolveTooltipPanelPath(
    cell: GraphCell,
    targetKind: 'key' | 'value' | 'node',
  ): Promise<{ path: PathSeg[]; pathKey: string } | null> {
    const path = await resolveCellPath(cell, deps.resolveTreePathByPosition, cell.path ?? []);
    const interactivePath = await deps.resolveInteractiveCellPath(cell, path);
    if (!interactivePath.length || !deps.canOpenSubgraphPreviewForCell(cell, targetKind)) {
      return null;
    }
    const pathKey = buildPathKey(interactivePath);
    return pathKey ? { path: interactivePath, pathKey } : null;
  }

  function buildTooltipPanelGraphData(path: PathSeg[], pathKey: string, result: { nodes?: GraphNode[]; edges?: GraphEdge[] }): TooltipPanelGraphData | null {
    const nodes = result.nodes ?? [];
    const edges = result.edges ?? [];
    if (!nodes.length) return null;
    deps.inferGraphPaths(nodes, edges);
    const minX = Math.min(...nodes.map((node) => node.boxArgs.x));
    const minY = Math.min(...nodes.map((node) => node.boxArgs.y));
    const maxX = Math.max(...nodes.map((node) => node.boxArgs.x + node.boxArgs.width));
    const maxY = Math.max(...nodes.map((node) => node.boxArgs.y + node.boxArgs.height));
    return {
      pathKey,
      path,
      nodes,
      edges,
      minX,
      minY,
      width: Math.max(0, maxX - minX),
      height: Math.max(0, maxY - minY),
    };
  }

  async function prepareTooltipPanelGraph(path: PathSeg[], pathKey: string): Promise<TooltipPanelGraphData | null> {
    ensureTooltipPanelGraphCacheSignature();
    const signature = tooltipPanelGraphCacheSignature;
    const current = tooltipPanelGraphCache.get(pathKey);
    if (current && current.signature === signature) {
      touchTooltipPanelGraphCacheEntry(current);
      if (current.graph !== undefined) return current.graph;
      if (current.promise) return current.promise;
    }

    const entry: TooltipPanelGraphCacheEntry = {
      signature,
      accessOrder: 0,
    };
    touchTooltipPanelGraphCacheEntry(entry);
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
        throw new Error('hover subgraph projection decode failed');
      }
      const normalized = normalizeGraphDelta(delta);
      return buildTooltipPanelGraphData(path, pathKey, {
        nodes: normalized.nodesAdded as unknown as GraphNode[],
        edges: normalized.edgesAdded as unknown as GraphEdge[],
      });
    })
      .then((graph) => {
        if (tooltipPanelGraphCacheSignature !== signature) return graph;
        const cached = tooltipPanelGraphCache.get(pathKey);
        if (!cached || cached.signature !== signature) return graph;
        cached.graph = graph;
        cached.promise = undefined;
        touchTooltipPanelGraphCacheEntry(cached);
        trimTooltipPanelGraphCache();
        return graph;
      })
      .catch((error) => {
        tooltipPanelGraphCache.delete(pathKey);
        throw error;
      });
    tooltipPanelGraphCache.set(pathKey, entry);
    trimTooltipPanelGraphCache();
    return entry.promise;
  }

  function scheduleTooltipPanelPrewarm(): void {
    ensureTooltipPanelGraphCacheSignature();
    cancelTooltipPanelPrewarm();
    const candidates = collectTooltipPanelPrewarmCandidatesFromClickTargets(
      deps.getRootClickTargets(),
      deps.canOpenSubgraphPreviewForCell,
      tooltipPanelPrewarmLimit,
    );
    tooltipPanelPrewarmScheduledPaths = [];
    tooltipPanelPrewarmCompletedPaths = [];
    if (!candidates.length) return;
    const queue = [...candidates];
    const token = ++tooltipPanelPrewarmToken;

    const scheduleNext = () => {
      if (token !== tooltipPanelPrewarmToken || !queue.length) return;
      const run = async () => {
        tooltipPanelPrewarmHandle = null;
        if (token !== tooltipPanelPrewarmToken) return;
        const next = queue.shift();
        if (!next) return;
        try {
          const resolved = await resolveTooltipPanelPath(next.cell, next.target);
          if (resolved) {
            const pathKey = buildPathKey(resolved.path);
            if (!tooltipPanelPrewarmScheduledPaths.some((path) => buildPathKey(path) === pathKey)) {
              tooltipPanelPrewarmScheduledPaths.push(clonePath(resolved.path));
            }
            tooltipPanelPrewarmInFlightPath = clonePath(resolved.path);
            await prepareTooltipPanelGraph(resolved.path, resolved.pathKey);
            if (token === tooltipPanelPrewarmToken) {
              if (!tooltipPanelPrewarmCompletedPaths.some((path) => buildPathKey(path) === pathKey)) {
                tooltipPanelPrewarmCompletedPaths.push(clonePath(resolved.path));
              }
            }
          }
        } catch (error) {
          deps.handleError(error, {
            component: 'GraphViewer',
            operation: 'prewarmGraphHoverPanelGraph',
            metadata: { documentKey: deps.getDocumentKey(), language: deps.getLanguageId() },
          });
        } finally {
          if (token === tooltipPanelPrewarmToken) {
            tooltipPanelPrewarmInFlightPath = null;
          }
        }
        scheduleNext();
      };
      const win = globalThis as typeof globalThis & {
        requestIdleCallback?: (callback: () => void) => number;
        setTimeout?: (callback: () => void, delay?: number) => number;
      };
      tooltipPanelPrewarmHandle = typeof win.requestIdleCallback === 'function'
        ? win.requestIdleCallback(run)
        : (win.setTimeout?.(run, 0) ?? null);
    };

    scheduleNext();
  }

  function clearTooltipPreviewHost(host: HTMLElement) {
    void host;
    deps.setRuntimeHoverPreviewState(null);
  }

  function createTooltipPanelApp(view: HTMLDivElement): LeaferAppLike {
    const { LeaferCtor, PlainLeaferCtor } = deps.getConstructors();
    const TooltipLeaferCtor = LeaferCtor ?? PlainLeaferCtor;
    return new TooltipLeaferCtor!(buildTooltipPanelAppConfig(view));
  }

  function getLeaferContentRoot(target: LeaferAppLike | null): LeaferBox | null {
    if (!target) return null;
    const zoomLayer = target.zoomLayer as LeaferBox | undefined;
    if (zoomLayer) return zoomLayer;
    return target as unknown as LeaferBox;
  }

  function ensureTooltipPanelRuntime(host: HTMLElement): TooltipPanelRuntime | null {
    const { LeaferCtor, PlainLeaferCtor, BoxCtor } = deps.getConstructors();
    const TextCtor = deps.getConstructors().TextCtor;
    const PenCtor = deps.getConstructors().PenCtor;
    const mount = host.querySelector(tooltipPanelContainerSelector) as HTMLDivElement | null;
    if (!mount || (!LeaferCtor && !PlainLeaferCtor) || !BoxCtor || !TextCtor || !PenCtor) return null;
    if (tooltipPanelRuntime && tooltipPanelRuntime.host === host && tooltipPanelRuntime.mount === mount) {
      return tooltipPanelRuntime;
    }
    disposeTooltipPanelRuntimeForRemount();
    const view = document.createElement('div');
    view.className = 'graph-tooltip-panel-view';
    mount.replaceChildren(view);
    const app = createTooltipPanelApp(view);
    const root = getLeaferContentRoot(app);
    if (!root) return null;
    const edgeLayer = new BoxCtor({ x: 0, y: 0, width: 0, height: 0, fill: 'transparent' });
    const nodeLayer = new BoxCtor({ x: 0, y: 0, width: 0, height: 0, fill: 'transparent' });
    root.add?.(edgeLayer);
    root.add?.(nodeLayer);
    const editor = (app as LeaferEditorHost).editor ?? null;
    const textEditor = editor?.getInnerEditor?.('TextEditor') ?? editor?.innerEditor;
    if (textEditor?.config) {
      textEditor.config.selectAll = true;
    }
    deps.bindGraphEditorLifecycle(editor);
    const nestedTooltipPlugin = new TooltipPlugin(app as never, {
      className: 'leafer-x-tooltip',
      includeTypes: ['Text'],
      closeDelay: 320,
      interactive: (node) => resolveGraphHoverPreviewTarget((node as LeaferText) ?? null)?.previewKind === 'pre',
      shouldBegin: (event: unknown) => resolveGraphHoverPreviewTarget(((event as { target?: LeaferText })?.target ?? null) as LeaferText)?.previewKind === 'pre',
      getContent: (node) => buildTooltipContent(deps.getCurrentData(), node as LeaferText, deps.getLanguageId()),
      shouldKeepOpen: () => false,
      events: deps.getTooltipEvents(),
    } as never);
    const runtime: TooltipPanelRuntime = {
      host,
      mount,
      view,
      app,
      edgeLayer,
      nodeLayer,
      pathKey: '',
      path: [],
      clickTargetsById: Object.create(null),
      clickTargetIdByTarget: new WeakMap(),
      clickBoundTargets: new WeakSet(),
      cellBoxByPathMap: new Map(),
      nodeBoxMap: new Map(),
      tableRuntimes: [],
      editor,
    };
    const handleDocumentPointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (target instanceof Element && target.closest('.leafer-text-editor')) return;
      if (runtime.host.contains(target) || runtime.view.contains(target)) return;
      runtime.editor?.closeInnerEditor?.();
    };
    document.addEventListener('pointerdown', handleDocumentPointerDown, true);
    runtime.dispose = () => {
      nestedTooltipPlugin.destroy?.();
      document.removeEventListener('pointerdown', handleDocumentPointerDown, true);
    };
    tooltipPanelRuntime = runtime;
    return tooltipPanelRuntime;
  }

  function setTooltipPanelLoadingState(host: HTMLElement, loading: boolean): void {
    const mount = host.querySelector(tooltipPanelContainerSelector) as HTMLDivElement | null;
    if (!mount) return;
    mount.classList.toggle('graph-tooltip-panel--loading', loading);
  }

  async function renderPreparedTooltipPanelGraph(
    host: HTMLElement,
    requestToken: number,
    graph: TooltipPanelGraphData,
  ): Promise<void> {
    const runtime = ensureTooltipPanelRuntime(host);
    if (!runtime) return;
    runtime.pathKey = graph.pathKey;
    runtime.path = graph.path;
    runtime.clickTargetsById = Object.create(null);
    runtime.clickTargetIdByTarget = new WeakMap();
    runtime.clickBoundTargets = new WeakSet();
    runtime.cellBoxByPathMap = new Map();
    runtime.nodeBoxMap = new Map();
    runtime.tableRuntimes.forEach((tableRuntime) => tableRuntime.destroy?.());
    runtime.tableRuntimes = [];
    (runtime.edgeLayer as LeaferBox & { removeAll?: (deep?: boolean) => void }).removeAll?.(true);
    (runtime.nodeLayer as LeaferBox & { removeAll?: (deep?: boolean) => void }).removeAll?.(true);

    const panelPadding = 16;
    const panelWidth = graph.width + panelPadding * 2;
    const panelHeight = graph.height + panelPadding * 2;
    const hostWindow = host.ownerDocument.defaultView;
    const panelViewport = resolveTooltipPanelViewportSize(
      panelWidth,
      panelHeight,
      hostWindow?.innerWidth ?? tooltipPanelMaxViewportWidth + tooltipPanelViewportMargin,
      hostWindow?.innerHeight ?? tooltipPanelMaxViewportHeight + tooltipPanelViewportMargin,
    );
    runtime.mount.style.width = `${panelViewport.width}px`;
    runtime.mount.style.height = `${panelViewport.height}px`;
    runtime.edgeLayer.x = panelPadding - graph.minX;
    runtime.edgeLayer.y = panelPadding - graph.minY;
    runtime.nodeLayer.x = panelPadding - graph.minX;
    runtime.nodeLayer.y = panelPadding - graph.minY;
    runtime.app.resize?.({ width: panelViewport.width, height: panelViewport.height });
    await tick();
    runtime.app.updateClientBounds?.();
    runtime.app.update?.();
    const cellEntryBindings = createCellEntryBindings(runtime.cellBoxByPathMap);
    const registerTooltipPanelTarget = (
      targetBox: LeaferBox,
      targetCell: GraphCell,
      kind: GraphCellKind,
      nodeKind?: GraphNode['kind'],
    ): string => {
      if (!targetBox || typeof targetBox.on !== 'function') return '';
      return deps.registerPanelClickTarget(
        runtime.clickTargetsById,
        runtime.clickTargetIdByTarget!,
        runtime.clickBoundTargets!,
        targetBox,
        targetCell,
        kind,
        nodeKind,
      );
    };
    const { BoxCtor, TextCtor, PenCtor } = deps.getConstructors();
    const drawContext: DrawContext = {
      nodeLayer: runtime.nodeLayer,
      styleConfig: deps.getRenderConfig(),
      languageIdValue: deps.getLanguageId(),
      fontSize: deps.getRenderConfig().layout.baseFontSize,
      textEditInnerName: 'TooltipTextEditor',
      BoxCtor,
      TextCtor,
      PenCtor,
      valueTypeToSemType: deps.getValueTypeToSemType(),
      editable: deps.isReadonly?.() ? false : true,
      registerCellBox: cellEntryBindings.registerCellBox,
      unregisterCellBox: cellEntryBindings.unregisterCellBox,
      registerRowBox: cellEntryBindings.registerRowBox,
      unregisterRowBox: cellEntryBindings.unregisterRowBox,
      registerClickTarget: registerTooltipPanelTarget,
      requestRender: () => runtime.app.update?.(),
      bindVerticalScrollGesture: deps.bindVerticalScrollGesture,
      bindPointerDown: deps.bindPointerDown,
      getPointFromEvent: (hostApp, target, event, space) => {
        const activeApp = (hostApp as LeaferAppLike | null) ?? runtime.app;
        const view = (activeApp as { view?: { parentElement?: Element | null } | null })?.view;
        if (!view || !view.parentElement) return null;
        return deps.getPointFromEvent?.(activeApp, target as LeaferBox, event, space) ?? null;
      },
    };
    graph.nodes.forEach((node) => {
      const result = renderGraphNode({
        node,
        drawContext,
        registerMetaClickTarget: (target, targetCell, kind) => {
          registerTooltipPanelTarget(target as LeaferBox, targetCell, kind);
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
      maxPerSource: null,
    });
    await tick();
    runtime.app.updateClientBounds?.();
    runtime.app.update?.();
    if (requestToken !== tooltipPanelRequestToken) return;
    setTooltipPanelLoadingState(host, false);
    tooltipPanelPendingPathKey = '';
    deps.setRuntimeHoverPreviewState({ kind: 'subgraph', visible: true });
    deps.setRuntimeHoverPanelDebugState({ phase: 'panel-ready', error: '' });
    await refreshTooltipPanelPlacement(runtime);
  }

  async function openTooltipPanelForCell(host: HTMLElement, cell: GraphCell, targetKind: 'key' | 'value' | 'node'): Promise<void> {
    const resolved = await resolveTooltipPanelPath(cell, targetKind);
    if (!resolved) {
      deps.setRuntimeHoverPanelDebugState({ phase: 'panel-skipped', error: '' });
      destroyTooltipPanelRuntime();
      return;
    }
    const { path: interactivePath, pathKey } = resolved;
    if (tooltipPanelRuntime?.host === host && tooltipPanelRuntime.pathKey === pathKey) {
      deps.setRuntimeHoverPanelDebugState({ phase: 'panel-reused', error: '' });
      return;
    }
    if (tooltipPanelPendingPathKey === pathKey) {
      return;
    }
    tooltipPanelPendingPathKey = pathKey;
    deps.setRuntimeHoverPanelDebugState({ phase: 'panel-requested', error: '' });
    clearTooltipPreviewHost(host);
    deps.setRuntimeHoverPreviewState({ kind: 'subgraph', visible: true });
    setTooltipPanelLoadingState(host, true);
    const requestToken = ++tooltipPanelRequestToken;
    try {
      const graph = await prepareTooltipPanelGraph(interactivePath, pathKey);
      if (requestToken !== tooltipPanelRequestToken) {
        return;
      }
      if (!graph) {
        deps.setRuntimeHoverPanelDebugState({ phase: 'panel-empty', error: '' });
        destroyTooltipPanelRuntime();
        return;
      }
      await renderPreparedTooltipPanelGraph(host, requestToken, graph);
    } catch (error) {
      if (requestToken !== tooltipPanelRequestToken) {
        return;
      }
      deps.handleError(error, {
        component: 'GraphViewer',
        operation: 'buildHoverSubgraphProjection',
        metadata: { documentKey: deps.getDocumentKey(), language: deps.getLanguageId(), source: 'tooltip' },
      });
      deps.setRuntimeHoverPanelDebugState({ phase: 'worker-error', error: error instanceof Error ? error.message : String(error) });
      destroyTooltipPanelRuntime();
    } finally {
      if (tooltipPanelPendingPathKey === pathKey) {
        tooltipPanelPendingPathKey = '';
      }
    }
  }

  async function refreshTooltipPanelPlacement(runtime: TooltipPanelRuntime | null = tooltipPanelRuntime): Promise<void> {
    if (!runtime || tooltipPanelRuntime !== runtime) return;
    deps.refreshTooltipPosition();
    await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    if (!tooltipPanelRuntime || tooltipPanelRuntime !== runtime) return;
    runtime.app.updateClientBounds?.();
    runtime.app.update?.();
  }

  async function renderTooltipContent(host: HTMLElement, target: LeaferText | null): Promise<void> {
    const previewTarget = resolveGraphHoverPreviewTarget(target);
    if (previewTarget?.previewKind !== 'subgraph') {
      void host;
      destroyTooltipPanelRuntime();
      deps.setRuntimeHoverPreviewState(
        previewTarget
          ? {
              kind: previewTarget.previewKind,
              text: previewTarget.cell.text,
              language: deps.getLanguageId(),
              visible: true,
            }
          : null,
      );
      deps.setRuntimeHoverPanelDebugState({ phase: previewTarget ? 'preview-ready' : 'preview-empty', error: '' });
      deps.refreshTooltipPosition();
      return;
    }
    await openTooltipPanelForCell(host, previewTarget.cell, previewTarget.target);
    await refreshTooltipPanelPlacement(tooltipPanelRuntime?.host === host ? tooltipPanelRuntime : null);
  }

  function hasTooltipPanelActivity() {
    return !!tooltipPanelPendingPathKey;
  }

  function getTooltipPanelApp() {
    return tooltipPanelRuntime?.app ?? null;
  }

  function getTooltipPanelClickTargets() {
    return tooltipPanelRuntime ? Object.values(tooltipPanelRuntime.clickTargetsById) : [];
  }

  function getTooltipPanelPath() {
    return tooltipPanelRuntime?.path ?? [];
  }

  function getTooltipPanelRuntimeDebugSnapshot() {
    const runtime = tooltipPanelRuntime;
    if (!runtime) return null;
    const edgeChildren = Array.isArray((runtime.edgeLayer as { children?: unknown[] }).children)
      ? ((runtime.edgeLayer as { children?: unknown[] }).children?.length ?? 0)
      : null;
    const nodeChildren = Array.isArray((runtime.nodeLayer as { children?: unknown[] }).children)
      ? ((runtime.nodeLayer as { children?: unknown[] }).children?.length ?? 0)
      : null;
    return {
      pathKey: runtime.pathKey,
      clickTargetCount: Object.keys(runtime.clickTargetsById).length,
      tableRuntimeCount: runtime.tableRuntimes.length,
      mountWidth: runtime.mount.style.width,
      mountHeight: runtime.mount.style.height,
      edgeChildren,
      nodeChildren,
    };
  }

  function getTooltipPanelPrewarmDebugSnapshot(): TooltipPanelPrewarmDebugSnapshot {
    return {
      scheduledPaths: tooltipPanelPrewarmScheduledPaths.map((path) => clonePath(path)),
      completedPaths: tooltipPanelPrewarmCompletedPaths.map((path) => clonePath(path)),
      inFlightPath: tooltipPanelPrewarmInFlightPath ? clonePath(tooltipPanelPrewarmInFlightPath) : null,
    };
  }

  return {
    applyTheme,
    clearTooltipPreviewHost,
    destroyTooltipPanelRuntime,
    disposeTooltipEditor,
    ensureTooltipRuntime,
    getTooltipPanelApp,
    getTooltipPanelClickTargets,
    getTooltipPanelPath,
    getTooltipPanelPrewarmDebugSnapshot,
    getTooltipPanelRuntimeDebugSnapshot,
    hasTooltipPanelActivity,
    openTooltipPanelForCell,
    refreshTooltipPanelPlacement,
    renderTooltipContent,
    resolveGraphHoverPreviewTarget,
    scheduleTooltipPanelPrewarm,
  };
}
