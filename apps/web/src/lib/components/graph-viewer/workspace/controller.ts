import { tick } from 'svelte';
import type { SnapshotId } from '@core-wasm/index';
import type { SupportedEditorLanguageId } from '../../../monaco/language-support';
import type { GraphViewerConfig } from '../../../settings/ui-settings';
import { queryPathValue } from '../../../services/SnapshotProjectionService';
import { buildPathKey } from '../../../graph/graph-viewer-path';
import type { GraphCell, GraphEdge, GraphNode } from '../../../graph/graph-viewer-render';
import { getClampedPaneSize } from '../../ui/split-layout';
import type { PathSeg } from '../../../store/tree-path';
import type { LeaferAppLike, LeaferBox, LeaferEditor, LeaferText, SubgraphWorkspaceRuntime } from '../model';
import {
  createSubgraphWorkspaceGraphCache,
  destroySubgraphWorkspaceRuntime,
  formatSubgraphWorkspacePath,
  renderSubgraphWorkspaceGraph,
  shouldIgnoreSubgraphOpenCell,
  shouldOpenSubgraphWorkspaceContent,
} from '../graph-subgraph-workspace';
import type {
  SubgraphWorkspaceActivatePayload,
  SubgraphWorkspaceContentState,
  SubgraphWorkspacePaneState,
  SubgraphWorkspaceState,
  VisibleSubgraphWorkspacePaneState,
} from './types';

const SUBGRAPH_WORKSPACE_MIN_HEIGHT = 100;
const SUBGRAPH_WORKSPACE_MAX_HEIGHT_FRACTION = 0.75;
const MAX_VISIBLE_PANES = 3;

type GraphValueEditCell = GraphCell & {
  editable?: boolean;
  boxArgs: { x: number; y: number; width: number; height: number; cornerRadius: number };
  textArgs: {
    x: number;
    y: number;
    width: number;
    height: number;
    text: string;
    textAlign: 'left' | 'center' | 'right';
    verticalAlign: 'top' | 'middle' | 'bottom';
    editable: boolean;
  };
};

type SubgraphWorkspaceControllerDeps = {
  defaultHeightPx: number;
  getActiveSnapshotId: () => SnapshotId | null;
  getWorkspaceSnapshotId: () => SnapshotId | null;
  getDocumentKey: () => string;
  getLanguageId: () => SupportedEditorLanguageId;
  getRevision: () => number;
  getRenderConfig: () => GraphViewerConfig;
  getEnableNest: () => boolean;
  getReadonly: () => boolean;
  getShellHeight: () => number;
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
  getValueTypeToSemType: () => Record<string, string>;
  inferGraphPaths: (nodes: GraphNode[], edges: GraphEdge[]) => void;
  clearSearchHighlight: () => void;
  clearActiveGraphSelection: () => void;
  emitReveal: (path: PathSeg[], target: 'key' | 'value' | 'node', trigger: 'click') => void;
  handleError: (
    error: unknown,
    context: { component: string; operation: string; metadata?: Record<string, unknown> },
  ) => void;
  applyGraphEdit: (cell: GraphValueEditCell, kind: 'key' | 'value', raw: string) => Promise<boolean>;
  markSubgraphRequested: (payload: { requestId: number; pathKey: string; sourceRevision: number }) => void;
  markSubgraphMaterialized: (payload: {
    requestId: number;
    pathKey: string;
    sourceRevision: number;
    materializedRevision: number;
  }) => void;
  bindGraphEditorLifecycle: (editor: LeaferEditor | null) => void;
  bindPointerClick: (target: LeaferBox, handler: (event: unknown) => void | Promise<void>) => void;
  getMoveEventName: () => string | undefined;
  bindVerticalScrollGesture: (
    target: LeaferBox,
    handler: (gesture: { event: unknown; deltaY: number; moveType?: string; stop: () => void; stopNow: () => void }) => void,
  ) => (() => void) | void;
  bindPointerDown: (target: LeaferBox, handler: (event: unknown) => void | Promise<void>) => (() => void) | void;
  getPointFromEvent: (
    hostApp: LeaferAppLike | null,
    target: LeaferBox,
    event: unknown,
    space: 'client' | 'box' | 'local' | 'world',
  ) => { x: number; y: number } | null;
  resolveInteractiveCellPath: (cell: GraphCell, fallbackPath: PathSeg[]) => Promise<PathSeg[]>;
  onState: (state: SubgraphWorkspaceState) => void;
};

function clonePaneState(pane: SubgraphWorkspacePaneState): SubgraphWorkspacePaneState {
  return {
    ...pane,
    path: [...pane.path],
    graph: pane.graph,
    content: pane.content ? { ...pane.content } : null,
  };
}

export function createSubgraphWorkspaceController(deps: SubgraphWorkspaceControllerDeps) {
  const runtimeMap = new Map<string, SubgraphWorkspaceRuntime>();
  const hostMap = new Map<string, HTMLDivElement>();
  const pendingEditKeys = new Set<string>();
  const queuedEditMap = new Map<string, string>();
  const graphCache = createSubgraphWorkspaceGraphCache({
    getActiveSnapshotId: deps.getActiveSnapshotId,
    getDocumentKey: deps.getDocumentKey,
    getLanguageId: deps.getLanguageId,
    getRevision: deps.getRevision,
    getEnableNest: deps.getEnableNest,
    getRenderConfig: deps.getRenderConfig,
    inferGraphPaths: deps.inferGraphPaths,
  });

  let chain: SubgraphWorkspacePaneState[] = [];
  let visiblePanes: VisibleSubgraphWorkspacePaneState[] = [];
  let heightPx = deps.defaultHeightPx;
  let isDraggingDivider = false;
  let resizeState: { startClientY: number; startHeightPx: number } | null = null;
  let renderSignature = '';
  let refreshToken = 0;
  let requestId = 0;

  function emitState(): void {
    deps.onState({
      chain: chain.map(clonePaneState),
      visiblePanes: visiblePanes.map((pane) => ({ ...clonePaneState(pane), visibleIndex: pane.visibleIndex, absoluteIndex: pane.absoluteIndex })),
      heightPx,
      isDraggingDivider,
    });
  }

  function clampHeight(nextHeightPx: number): number {
    return getClampedPaneSize(
      nextHeightPx,
      deps.getShellHeight(),
      SUBGRAPH_WORKSPACE_MIN_HEIGHT,
      SUBGRAPH_WORKSPACE_MAX_HEIGHT_FRACTION,
    );
  }

  function syncTransientState(nextChain: SubgraphWorkspacePaneState[]): void {
    const nextKeys = new Set(nextChain.map((pane) => pane.pathKey));
    for (const pathKey of pendingEditKeys) {
      if (!nextKeys.has(pathKey)) pendingEditKeys.delete(pathKey);
    }
    for (const pathKey of queuedEditMap.keys()) {
      if (!nextKeys.has(pathKey)) queuedEditMap.delete(pathKey);
    }
  }

  function updateVisiblePanes(): void {
    const start = Math.max(0, chain.length - MAX_VISIBLE_PANES);
    visiblePanes = chain.slice(start).map((pane, index) => ({
      ...pane,
      visibleIndex: index,
      absoluteIndex: start + index,
    }));
  }

  function disposeRuntimes(exceptPathKeys: string[] = []): void {
    const preserved = new Set(exceptPathKeys);
    for (const [pathKey, runtime] of runtimeMap.entries()) {
      if (preserved.has(pathKey)) continue;
      destroySubgraphWorkspaceRuntime(runtime);
      runtimeMap.delete(pathKey);
    }
  }

  function setChain(nextChain: SubgraphWorkspacePaneState[]): void {
    const previousRequestIds = new Map(
      chain.filter((pane) => typeof pane.requestId === 'number').map((pane) => [pane.pathKey, pane.requestId as number]),
    );
    chain = nextChain.map((pane) => ({
      ...pane,
      requestId: pane.requestId ?? previousRequestIds.get(pane.pathKey),
    }));
    syncTransientState(chain);
    updateVisiblePanes();
    disposeRuntimes(chain.map((pane) => pane.pathKey));
    emitState();
  }

  async function buildContentState(path: PathSeg[]): Promise<SubgraphWorkspaceContentState | null> {
    const snapshotId = deps.getWorkspaceSnapshotId();
    const pathValue = await queryPathValue({ documentKey: deps.getDocumentKey(), snapshotId, path });
    if (pathValue.status !== 'ready' || !pathValue.data) return null;
    const valueType = pathValue.data.valueType as SubgraphWorkspaceContentState['valueType'];
    if (!shouldOpenSubgraphWorkspaceContent(pathValue.data)) return null;
    return {
      tabId: `subgraph-content:${buildPathKey(path)}`,
      tabName: formatSubgraphWorkspacePath(path, deps.getRenderConfig()),
      sourceText: pathValue.data.displayText,
      valueType,
    };
  }

  async function preparePane(path: PathSeg[]): Promise<SubgraphWorkspacePaneState | null> {
    const pathKey = buildPathKey(path);
    if (!pathKey) return null;
    const title = formatSubgraphWorkspacePath(path, deps.getRenderConfig());
    const content = await buildContentState(path);
    if (content) {
      return {
        path,
        pathKey,
        title,
        kind: 'content',
        graph: null,
        content,
        status: 'ready',
      };
    }
    try {
      const graph = await graphCache.prepareGraph(path);
      return {
        path,
        pathKey,
        title,
        kind: 'graph',
        graph,
        content: null,
        status: graph ? 'ready' : 'empty',
      };
    } catch (error) {
      deps.handleError(error, {
        component: 'GraphViewer',
        operation: 'buildSubgraphWorkspaceProjection',
        metadata: { documentKey: deps.getDocumentKey(), language: deps.getLanguageId(), pathKey },
      });
      return {
        path,
        pathKey,
        title,
        kind: 'graph',
        graph: null,
        content: null,
        status: 'error',
        error: error instanceof Error ? error.message : String(error),
      };
    }
  }

  async function renderPanes(): Promise<void> {
    const visibleSignature = visiblePanes.map((pane) => `${pane.pathKey}:${pane.status}:${pane.absoluteIndex}`).join('|');
    const constructors = deps.getConstructors();
    const nextSignature = [
      visibleSignature,
      deps.getLanguageId(),
      deps.getReadonly() ? 'readonly' : 'editable',
      constructors.LeaferCtor ? 'leafer' : 'no-leafer',
      constructors.BoxCtor ? 'box' : 'no-box',
      constructors.TextCtor ? 'text' : 'no-text',
      constructors.PenCtor ? 'pen' : 'no-pen',
    ].join('|');
    if (nextSignature === renderSignature) return;
    renderSignature = nextSignature;
    const activeKeys = visiblePanes.filter((pane) => pane.status === 'ready' && pane.graph).map((pane) => pane.pathKey);
    disposeRuntimes(activeKeys);
    for (const pane of visiblePanes) {
      if (pane.kind === 'content') continue;
      const mount = hostMap.get(pane.pathKey);
      if (!mount) continue;
      if (pane.status !== 'ready' || !pane.graph) {
        mount.replaceChildren();
        continue;
      }
      const existingRuntime = runtimeMap.get(pane.pathKey) as (SubgraphWorkspaceRuntime & {
        __graphRef?: typeof pane.graph;
      }) | undefined;
      if (existingRuntime && existingRuntime.host === mount && existingRuntime.__graphRef === pane.graph) {
        continue;
      }
      destroySubgraphWorkspaceRuntime(existingRuntime);
      const runtime = await renderSubgraphWorkspaceGraph(mount, pane.graph, {
        getConstructors: deps.getConstructors,
        getRenderConfig: deps.getRenderConfig,
        getLanguageId: deps.getLanguageId,
        getValueTypeToSemType: deps.getValueTypeToSemType,
        isReadonly: deps.getReadonly,
        bindGraphEditorLifecycle: deps.bindGraphEditorLifecycle,
        bindPointerClick: deps.bindPointerClick,
        getMoveEventName: deps.getMoveEventName,
        bindVerticalScrollGesture: deps.bindVerticalScrollGesture,
        bindPointerDown: deps.bindPointerDown,
        getPointFromEvent: deps.getPointFromEvent,
        resolveInteractiveCellPath: deps.resolveInteractiveCellPath,
        onActivateCell: (payload) => handleActivate(payload, pane.absoluteIndex),
      });
      if (runtime) {
        (runtime as SubgraphWorkspaceRuntime & { __graphRef?: typeof pane.graph }).__graphRef = pane.graph;
        runtimeMap.set(pane.pathKey, runtime);
      }
    }
  }

  async function openPath(path: PathSeg[], parentAbsoluteIndex: number): Promise<void> {
    const pathKey = buildPathKey(path);
    if (!pathKey) return;
    const currentChild = chain[parentAbsoluteIndex + 1] ?? null;
    if (currentChild?.pathKey === pathKey) return;
    const nextRequestId = ++requestId;
    deps.markSubgraphRequested({
      requestId: nextRequestId,
      pathKey,
      sourceRevision: deps.getRevision(),
    });
    const base = parentAbsoluteIndex >= 0 ? chain.slice(0, parentAbsoluteIndex + 1) : [];
    setChain([
      ...base,
      {
        requestId: nextRequestId,
        path,
        pathKey,
        title: formatSubgraphWorkspacePath(path, deps.getRenderConfig()),
        kind: 'graph',
        graph: null,
        content: null,
        status: 'loading',
      },
    ]);
    const pane = await preparePane(path);
    if (!pane || nextRequestId !== requestId) return;
    const latestBase = parentAbsoluteIndex >= 0 ? chain.slice(0, parentAbsoluteIndex + 1) : [];
    if (latestBase.some((entry, index) => base[index]?.pathKey !== entry.pathKey)) return;
    const nextPane = { ...pane, requestId: nextRequestId };
    deps.markSubgraphMaterialized({
      requestId: nextRequestId,
      pathKey,
      sourceRevision: deps.getRevision(),
      materializedRevision: deps.getRevision(),
    });
    setChain([...latestBase, nextPane]);
    await tick();
    await renderPanes();
  }

  function closePane(absoluteIndex: number): void {
    if (absoluteIndex < 0 || absoluteIndex >= chain.length) return;
    setChain(chain.slice(0, absoluteIndex));
  }

  async function handleActivate(payload: SubgraphWorkspaceActivatePayload, parentAbsoluteIndex: number): Promise<void> {
    if (shouldIgnoreSubgraphOpenCell(payload.cell)) return;
    deps.emitReveal(payload.path, payload.target, 'click');
    await openPath(payload.path, parentAbsoluteIndex);
  }

  async function commitValueEdit(pane: SubgraphWorkspacePaneState, draft?: string): Promise<void> {
    if (deps.getReadonly() || pane.kind !== 'content' || !pane.content) return;
    const nextText = draft ?? pane.content.sourceText;
    if (nextText === pane.content.sourceText) return;
    if (pendingEditKeys.has(pane.pathKey)) {
      queuedEditMap.set(pane.pathKey, nextText);
      return;
    }
    pendingEditKeys.add(pane.pathKey);
    try {
      let nextDraft = nextText;
      if (pane.content.valueType === 'string') {
        try {
          const parsed = JSON.parse(nextDraft);
          if (typeof parsed === 'string') nextDraft = parsed;
        } catch {}
      }
      await deps.applyGraphEdit(
        {
          text: pane.content.sourceText,
          value: pane.content.sourceText,
          valueType: pane.content.valueType,
          path: pane.path,
          editable: !deps.getReadonly(),
          boxArgs: { x: 0, y: 0, width: 0, height: 0, cornerRadius: 0 },
          textArgs: {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            text: pane.content.sourceText,
            textAlign: 'left',
            verticalAlign: 'top',
            editable: !deps.getReadonly(),
          },
        },
        'value',
        nextDraft,
      );
    } finally {
      pendingEditKeys.delete(pane.pathKey);
    }
    const queuedText = queuedEditMap.get(pane.pathKey);
    if (queuedText == null || queuedText === nextText) return;
    queuedEditMap.delete(pane.pathKey);
    await commitValueEdit(pane, queuedText);
  }

  function bindHost(pathKey: string, host: HTMLDivElement | null): void {
    if (host) {
      hostMap.set(pathKey, host);
    } else {
      hostMap.delete(pathKey);
      const runtime = runtimeMap.get(pathKey);
      destroySubgraphWorkspaceRuntime(runtime);
      runtimeMap.delete(pathKey);
    }
    renderSignature = '';
    void renderPanes();
  }

  function hostAction(node: HTMLDivElement, pathKey: string) {
    let currentPathKey = pathKey;
    bindHost(currentPathKey, node);
    return {
      update(nextPathKey: string) {
        if (nextPathKey === currentPathKey) return;
        bindHost(currentPathKey, null);
        currentPathKey = nextPathKey;
        bindHost(currentPathKey, node);
      },
      destroy() {
        bindHost(currentPathKey, null);
      },
    };
  }

  function startDividerDrag(clientY: number): void {
    isDraggingDivider = true;
    resizeState = {
      startClientY: clientY,
      startHeightPx: heightPx,
    };
    emitState();
  }

  function moveDividerDrag(clientY: number): void {
    if (!resizeState) return;
    const deltaY = resizeState.startClientY - clientY;
    heightPx = clampHeight(resizeState.startHeightPx + deltaY);
    emitState();
  }

  function endDividerDrag(): void {
    isDraggingDivider = false;
    resizeState = null;
    emitState();
  }

  function syncHeightToShell(): void {
    if (!visiblePanes.length) return;
    const nextHeight = clampHeight(heightPx);
    if (nextHeight === heightPx) return;
    heightPx = nextHeight;
    emitState();
  }

  async function refreshPanes(): Promise<void> {
    if (!chain.length) return;
    const token = ++refreshToken;
    const nextChain: SubgraphWorkspacePaneState[] = [];
    for (const pane of chain) {
      const nextPane = await preparePane(pane.path);
      if (token !== refreshToken) return;
      if (nextPane) nextChain.push(nextPane);
    }
    setChain(nextChain);
    await tick();
    await renderPanes();
  }

  function clearCache(): void {
    graphCache.clear();
  }

  function invalidateRender(): void {
    renderSignature = '';
  }

  function reset(): void {
    refreshToken += 1;
    requestId += 1;
    renderSignature = '';
    deps.clearSearchHighlight();
    deps.clearActiveGraphSelection();
    setChain([]);
    graphCache.clear();
  }

  function dispose(): void {
    disposeRuntimes();
    graphCache.clear();
    hostMap.clear();
    pendingEditKeys.clear();
    queuedEditMap.clear();
  }

  emitState();

  return {
    getChain: () => chain,
    getVisiblePanes: () => visiblePanes,
    getRuntime: (pathKey: string) => runtimeMap.get(pathKey),
    hasRuntime: (pathKey: string) => runtimeMap.has(pathKey),
    hostAction,
    openPath,
    closePane,
    commitValueEdit,
    renderPanes,
    refreshPanes,
    clearCache,
    invalidateRender,
    reset,
    dispose,
    syncHeightToShell,
    startDividerDrag,
    moveDividerDrag,
    endDividerDrag,
  };
}
