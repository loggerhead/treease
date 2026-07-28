import { tick } from 'svelte';
import type { SnapshotId } from '@core-wasm/index';
import type { SupportedEditorLanguageId } from '../../../monaco/language-support';
import type { GraphViewerConfig } from '../../../settings/ui-settings';
import { queryNodePreview, queryPathValue } from '../../../services/SnapshotProjectionService';
import { buildPathKey } from '../../../graph/graph-viewer-path';
import type { GraphCell, GraphEdge, GraphNode } from '@treease/graph-viewer-runtime';
import { getClampedPaneSize } from '../../ui/split-layout';
import { createViewRuntimeOperation, type ViewRuntimeOperation } from '../../../guards/view-runtime-operation';
import type { PathSeg } from '../../../store/tree-path';
import type { LeaferAppLike, LeaferBox, LeaferEditor, LeaferText, SubgraphWorkspaceRuntime } from '../model';
import {
  createSubgraphWorkspaceGraphCache,
  buildSubgraphWorkspaceRenderSignature,
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

export type SubgraphWorkspaceProjectionInput = {
  documentKey: string;
  languageId: SupportedEditorLanguageId;
  revision: number;
  graphAppliedRevision: number;
  snapshotId: SnapshotId | null;
  enableNest: boolean;
  renderConfig: GraphViewerConfig;
};

export type SubgraphWorkspaceControllerDeps = {
  defaultHeightPx: number;
  getActiveSnapshotId: () => SnapshotId | null;
  getWorkspaceSnapshotId: () => SnapshotId | null;
  getDocumentKey: () => string;
  getLanguageId: () => SupportedEditorLanguageId;
  /** @deprecated Workspace rendering reads GraphCell.semType. */
  getValueTypeToSemType?: () => Record<string, string>;
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
  inferGraphPaths: (nodes: GraphNode[], edges: GraphEdge[]) => void;
  clearSearchHighlight: () => void;
  clearActiveGraphSelection: () => void;
  emitReveal: (path: PathSeg[], target: 'key' | 'value' | 'node', trigger: 'click') => void;
  handleError: (
    error: unknown,
    context: { component: string; operation: string; metadata?: Record<string, unknown> },
  ) => void;
  applyGraphEdit: (cell: GraphValueEditCell, kind: 'key' | 'value', raw: string) => Promise<boolean>;
  waitForCommittedDocument: (documentKey: string, afterRevision: number) => Promise<boolean>;
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
  onPaneReady?: (pane: SubgraphWorkspacePaneState) => void;
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
  let projectionSignature = '';
  let requestId = 0;
  let disposed = false;
  let projectionEpoch = 0;
  let openPathOperation: ViewRuntimeOperation | null = null;
  let refreshOperation: ViewRuntimeOperation | null = null;
  let renderOperation: ViewRuntimeOperation | null = null;

  function createWorkspaceOperation(): ViewRuntimeOperation {
    const context = () => ({
      documentKey: deps.getDocumentKey(),
      languageId: deps.getLanguageId(),
      revision: deps.getRevision(),
      sessionId: `${projectionEpoch}|${deps.getWorkspaceSnapshotId() ?? 'no-snapshot'}|${deps.getEnableNest() ? 'nest' : 'flat'}`,
    });
    return createViewRuntimeOperation({ captured: context(), getCurrent: context });
  }

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

  function notifyVisiblePaneReadiness(): void {
    for (const pane of visiblePanes) {
      deps.onPaneReady?.(pane);
    }
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
    const [pathValue, nodePreview] = await Promise.all([
      queryPathValue({ documentKey: deps.getDocumentKey(), snapshotId, path }),
      queryNodePreview({ documentKey: deps.getDocumentKey(), snapshotId, path }),
    ]);
    if (pathValue.status !== 'ready' || !pathValue.data) return null;
    const valueType = pathValue.data.valueType as SubgraphWorkspaceContentState['valueType'];
    if (!shouldOpenSubgraphWorkspaceContent(pathValue.data)) return null;
    return {
      tabId: `subgraph-content:${buildPathKey(path)}`,
      tabName: formatSubgraphWorkspacePath(path, deps.getRenderConfig()),
      sourceText: pathValue.data.displayText,
      valueType,
      rootSemType: nodePreview.status === 'ready' ? (nodePreview.data?.semType ?? null) : null,
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
    if (disposed) return;
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
    if (nextSignature === renderSignature) {
      notifyVisiblePaneReadiness();
      return;
    }

    void renderOperation?.cancel();
    const pendingRuntimes = new Set<SubgraphWorkspaceRuntime>();
    const operation = createViewRuntimeOperation({
      captured: {
        documentKey: deps.getDocumentKey(),
        languageId: deps.getLanguageId(),
        revision: deps.getRevision(),
        sessionId: `${projectionEpoch}|${visibleSignature}`,
      },
      getCurrent: () => ({
        documentKey: deps.getDocumentKey(),
        languageId: deps.getLanguageId(),
        revision: deps.getRevision(),
        sessionId: `${projectionEpoch}|${visiblePanes.map((pane) => `${pane.pathKey}:${pane.status}:${pane.absoluteIndex}`).join('|')}`,
      }),
      onStale: () => {
        for (const runtime of pendingRuntimes) destroySubgraphWorkspaceRuntime(runtime);
        pendingRuntimes.clear();
      },
    });
    renderOperation = operation;
    renderSignature = nextSignature;
    const activeKeys = visiblePanes.filter((pane) => pane.status === 'ready' && pane.graph).map((pane) => pane.pathKey);
    await operation.run({
      execute: async ({ step }) => {
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
          const runtime = await step(async () => {
            const created = await renderSubgraphWorkspaceGraph(mount, pane.graph!, {
              getConstructors: deps.getConstructors,
              getRenderConfig: deps.getRenderConfig,
              getLanguageId: deps.getLanguageId,
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
            if (created) {
              if (!operation.isCurrent()) {
                destroySubgraphWorkspaceRuntime(created);
                return null;
              }
              pendingRuntimes.add(created);
            }
            return created;
          });
          if (runtime) {
            pendingRuntimes.delete(runtime);
            (runtime as SubgraphWorkspaceRuntime & { __graphRef?: typeof pane.graph }).__graphRef = pane.graph;
            runtimeMap.set(pane.pathKey, runtime);
          }
        }
      },
      land: () => {
        if (!disposed) notifyVisiblePaneReadiness();
      },
    });
  }

  async function openPath(path: PathSeg[], parentAbsoluteIndex: number): Promise<void> {
    if (disposed) return;
    const pathKey = buildPathKey(path);
    if (!pathKey) return;
    const currentChild = chain[parentAbsoluteIndex + 1] ?? null;
    if (currentChild?.pathKey === pathKey) return;

    void openPathOperation?.cancel();
    const operation = createWorkspaceOperation();
    openPathOperation = operation;
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
    await operation.run({
      execute: ({ step }) => step(() => preparePane(path)),
      land: async (pane) => {
        if (!pane || disposed) return;
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
        await operation.step(() => tick());
        await operation.step(() => renderPanes());
      },
    });
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
    if (disposed || deps.getReadonly() || pane.kind !== 'content' || !pane.content) return;
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
        } catch {
          nextDraft = nextText;
        }
      }
      const revisionBeforeCommit = deps.getRevision();
      const applied = await deps.applyGraphEdit(
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
      if (!applied) {
        queuedEditMap.delete(pane.pathKey);
        return;
      }
      const committed = await deps.waitForCommittedDocument(deps.getDocumentKey(), revisionBeforeCommit);
      if (!committed) {
        queuedEditMap.delete(pane.pathKey);
        return;
      }
    } finally {
      pendingEditKeys.delete(pane.pathKey);
    }
    const queuedText = queuedEditMap.get(pane.pathKey);
    if (queuedText == null || queuedText === nextText) return;
    queuedEditMap.delete(pane.pathKey);
    await commitValueEdit(pane, queuedText);
  }

  function bindHost(pathKey: string, host: HTMLDivElement | null): void {
    if (disposed) return;
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
    if (disposed || !chain.length) return;
    void refreshOperation?.cancel();
    const operation = createWorkspaceOperation();
    refreshOperation = operation;
    const sourceChain = chain;
    await operation.run({
      execute: async ({ step }) => {
        const nextChain: SubgraphWorkspacePaneState[] = [];
        for (const pane of sourceChain) {
          const nextPane = await step(() => preparePane(pane.path));
          if (nextPane) nextChain.push(nextPane);
        }
        return nextChain;
      },
      land: async (nextChain) => {
        if (disposed) return;
        setChain(nextChain);
        await operation.step(() => tick());
        await operation.step(() => renderPanes());
      },
    });
  }

  async function syncProjection(input: SubgraphWorkspaceProjectionInput): Promise<void> {
    const nextSignature = [
      input.documentKey,
      input.languageId,
      input.revision,
      input.graphAppliedRevision,
      input.snapshotId ?? 'no-snapshot',
      input.enableNest ? 'nest' : 'flat',
      buildSubgraphWorkspaceRenderSignature(input.renderConfig),
    ].join('|');
    if (nextSignature === projectionSignature) return;
    projectionSignature = nextSignature;
    projectionEpoch += 1;
    void openPathOperation?.cancel();
    void refreshOperation?.cancel();
    graphCache.clear();
    invalidateRender();
    await refreshPanes();
  }

  function invalidateRender(): void {
    void renderOperation?.cancel();
    renderSignature = '';
  }

  function reset(): void {
    projectionEpoch += 1;
    void openPathOperation?.cancel();
    void refreshOperation?.cancel();
    requestId += 1;
    void renderOperation?.cancel();
    renderSignature = '';
    deps.clearSearchHighlight();
    deps.clearActiveGraphSelection();
    setChain([]);
    graphCache.clear();
  }

  function dispose(): void {
    if (disposed) return;
    disposed = true;
    projectionEpoch += 1;
    void openPathOperation?.cancel();
    void refreshOperation?.cancel();
    requestId += 1;
    void renderOperation?.cancel();
    disposeRuntimes();
    graphCache.clear();
    hostMap.clear();
    pendingEditKeys.clear();
    queuedEditMap.clear();
    chain = [];
    visiblePanes = [];
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
    syncProjection,
    reset,
    dispose,
    syncHeightToShell,
    startDividerDrag,
    moveDividerDrag,
    endDividerDrag,
  };
}
