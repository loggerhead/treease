<!-- 职责：GraphViewer 稳定入口组件：Leafer 生命周期、controller 装配、跨域编排、DOM 模板 -->
<script lang="ts">
  import { onDestroy, tick, createEventDispatcher } from 'svelte';
  import { cubicOut } from 'svelte/easing';
  import { fly } from 'svelte/transition';
  import { X } from 'lucide-svelte';
  import type { SnapshotId } from '@core-wasm/index';
  import {
    sourceText,
    documentKey as documentKeyStore,
    languageId as languageIdStore,
    editorRevision,
    graphAppliedRevision,
    editorIO,
    activeTempModel,
    treeState,
    fullEditUiState,
    jsonBlockSelection,
    editorStore,
  } from '../store/editor-store';

  import { type GraphViewerConfig } from '../settings/ui-settings';
  import { settings, settingsStore } from '../settings/settings-store';
  import { shouldShowGraphRuntimeLoading, type RuntimeStateEventDetail } from '../runtime-loading';
  import { callSharedWasmWorker } from '../wasm/wasm-worker-singleton';
  import { getWorkspaceSnapshotId } from '../store/workspace-snapshot-bindings';
  import { queryPathValue } from '../services/SnapshotProjectionService';
  import { getFullEditDocumentJobSession } from '../graph-stream/full-edit-document-job-session';
  import { buildReadablePath, type PathSeg } from '../store/tree-path';
  import { type MinimapViewData } from '../leafer-minimap';
  import GraphRuntimeHost from './graph-viewer/GraphRuntimeHost.svelte';
  import GraphRuntimeLoading from './graph-viewer/GraphRuntimeLoading.svelte';
  import GraphStreamProgressOverlay from './graph-viewer/GraphStreamProgressOverlay.svelte';
  import SidecarEditor from './Editor/SidecarEditor.svelte';
  import { getClampedPaneSize, splitLayoutDrag } from './ui/split-layout';
  import { createGraphPointerController, type LeaferEventTarget } from './graph-viewer/graph-pointer-controller';
  import { createGraphRuntimeProbeController } from './graph-viewer/graph-runtime-probe-controller';
  import {
    createGraphStreamProgressController,
    type GraphStreamProgressState,
  } from './graph-viewer/graph-stream-progress';
  import { createGraphTextLinkageController } from './graph-viewer/graph-text-linkage';
  import { createGraphMinimapRuntimeController } from './graph-viewer/graph-minimap-runtime-controller';
  import { createGraphValueEditController } from './graph-viewer/graph-value-edit';
  import { createGraphViewportController, type LeaferZoomLayer } from './graph-viewer/graph-viewport-controller';
  import { getZoomScale } from './graph-viewer/graph-viewport-geometry';
  import {
    buildSubgraphWorkspaceRenderSignature,
    createSubgraphWorkspaceGraphCache,
    destroySubgraphWorkspaceRuntime,
    formatSubgraphWorkspacePath,
    rebaseSubgraphWorkspacePath,
    renderSubgraphWorkspaceGraph,
    shouldOpenSubgraphWorkspaceContent,
    shouldIgnoreSubgraphOpenCell,
  } from './graph-viewer/graph-subgraph-workspace';
  import {
    getCellEntry,
    registerCellBox as registerCellBoxEntry,
    registerRowBox as registerRowBoxEntry,
    resolveInteractiveCellPath as resolveInteractiveCellPathWithFallback,
    unregisterCellBox as unregisterCellBoxEntry,
    unregisterRowBox as unregisterRowBoxEntry,
    upsertCellEntry as upsertCellEntryInIndex,
    updateCellEntry as updateCellEntryInIndex,
  } from './graph-viewer/graph-anchor-index';
  import {
    getClientRectFromBoxLike,
    getClientProbeCoordFromBoxLike,
    getWorldRectFromBoxLike,
  } from './graph-viewer/graph-geometry';
  import { createGraphRenderSession, type GraphRenderGuard } from './graph-viewer/graph-render-session';
  import { createGraphSceneController } from './graph-viewer/graph-scene';
  import type { GraphSceneViewData } from './graph-viewer/graph-scene-runtime';
  import { createGraphMeasurementController } from './graph-viewer/graph-measurement-controller';
  import {
    buildGraphHighlightSignature,
    shouldApplyGraphHighlight,
  } from './graph-viewer/graph-viewer-highlight-effects';
  import { createGraphViewerRenderEffects } from './graph-viewer/graph-viewer-render-effects';
  import {
    clearGraphViewerTestHooks as clearGraphViewerTestHookState,
    shouldAttachGraphViewerTestHooks,
  } from './graph-viewer/graph-viewer-test-hooks';
  import type {
    CellBoxEntry,
    LeaferAppLike,
    LeaferBox,
    ScrollableBox,
    SubgraphWorkspaceRuntime,
  } from './graph-viewer/model';
  import type {
    GraphRuntimeProbeTarget,
  } from './graph-viewer/runtime/scene-types';
  import { editorLanguageFallback, type SupportedEditorLanguageId } from '../monaco/language-support';
  import type { EditorIO, GraphHighlightTarget } from '../store/editor-store';
  import { handleError } from '../utils/error-handler';
  import { GRAPH_CONFIG } from '../config/constants';
  import { resolveGraphCellDisplayText } from '../graph/literal-display';
  import { type GraphCell, type GraphCellKind, type GraphNode, type ValueType } from '../graph/graph-viewer-render';
  import { buildPathKey } from '../graph/graph-viewer-path';
  import type { SubgraphWorkspaceGraphData } from './graph-viewer/graph-subgraph-workspace-types';
  import { isDocumentRevisionGuardCurrent } from '../guards/document-revision-guard';
  import {
    clearGraphBridge,
    installGraphBridge,
    replaceGraphStreamState,
    resetGraphStreamState,
  } from '../test-bridge/register-graph-bridge';
  import {
    markGraphRequested,
    markSubgraphMaterialized,
    markSubgraphRequested,
    readRuntimeReadiness,
    syncGraphInteractionReadiness,
    syncSubgraphInteractionReadiness,
  } from '../test-bridge/runtime-readiness';
  import { PathSegTag, type TreeNode } from '@core-wasm/index';
  import type {
    App as LeaferApp,
    Box,
    Text,
    Pen,
    MoveEvent,
    ZoomEvent,
    DragEvent,
    LeaferEvent,
    PointerEvent as LeaferPointerEvent,
    Leafer,
  } from 'leafer-ui';

  type LeaferAppOrLeafer = LeaferApp | Leafer;
  export let enableRevealSync = true;
  export let synchronizedRuntimeLoading = false;
  export let readonly = false;

  const MINIMAP_WIDTH = 220;
  const MINIMAP_HEIGHT = 150;
  const SUBGRAPH_WORKSPACE_MIN_HEIGHT = 100;
  const SUBGRAPH_WORKSPACE_MAX_HEIGHT_FRACTION = 0.75;
  const SUBGRAPH_WORKSPACE_DEFAULT_HEIGHT = 220;

  let graphViewerShell: HTMLDivElement;
  let graphViewerShellHeight = 0;
  let container: HTMLDivElement;
  let minimapHost: HTMLDivElement;
  let leafer: LeaferAppOrLeafer | null = null;
  let LeaferCtor: typeof LeaferApp | typeof Leafer | undefined;
  let PlainLeaferCtor: typeof Leafer | undefined;
  let BoxCtor: typeof Box | undefined;
  let TextCtor: typeof Text | undefined;
  let PenCtor: typeof Pen | undefined;
  let errorMessage = '';
  let graphRuntimeReady = false;
  let showRuntimeLoading = true;
  let renderRuntimeReady = false;
  let documentKeyValue = '';
  let languageIdValue: SupportedEditorLanguageId = editorLanguageFallback;
  let editorRevisionValue = 0;
  let graphRenderGuard: GraphRenderGuard | null = null;
  let editorIOValue: EditorIO | null = null;
  let lastAutoOffset: { x: number; y: number } | null = null;
  let edgeLayer: Box | null = null;
  let nodeLayer: Box | null = null;
  let overlayLayer: Box | null = null;
  let canvasHintText: Text | null = null;
  let suppressGraphPointerUntil = 0;
  let MoveEventCtor: typeof MoveEvent | undefined;
  let ZoomEventCtor: typeof ZoomEvent | undefined;
  let DragEventCtor: typeof DragEvent | undefined;
  let LeaferEventCtor: typeof LeaferEvent | undefined;
  let PointerEventCtor: typeof LeaferPointerEvent | undefined;
  const dispatch = createEventDispatcher<{ reveal: unknown; 'runtime-state': RuntimeStateEventDetail }>();
  let nodeDataMap = new Map<number, GraphNode>();
  let lastRuntimeStateSignature = '';

  $: documentKeyValue = $documentKeyStore;
  $: languageIdValue = $languageIdStore;
  $: editorRevisionValue = $editorRevision;
  $: editorIOValue = $editorIO;
  $: showRuntimeLoading = shouldShowGraphRuntimeLoading({
    graphRuntimeReady,
    synchronizedRuntimeLoading,
    errorMessage,
  });
  $: renderRuntimeReady = Boolean(leafer && BoxCtor && TextCtor && PenCtor);
  let nodeBoxMap = new Map<number, LeaferBox>();
  let pathKeyToRenderHandleMap = new Map<string, number>();
  let cellBoxByPathMap = new Map<string, CellBoxEntry>();
  let lastAppliedGraphHighlightSignature = '';
  let lastAppliedGraphHighlightRevision = -1;
  let treeStateToken = 0;
  let streamProgressState: GraphStreamProgressState = {
    visible: false,
    streamRunId: '',
    label: '',
    detail: '',
    value: 0,
    phase: 'idle',
    startedAt: null,
    completedAt: null,
  };
  const graphStreamProgressController = createGraphStreamProgressController();
  const unsubscribeGraphStreamProgress = graphStreamProgressController.subscribe((value) => {
    streamProgressState = value;
  });
  let subgraphWorkspaceChain: SubgraphWorkspacePaneState[] = [];
  let subgraphWorkspaceVisiblePanes: Array<SubgraphWorkspacePaneState & { visibleIndex: number; absoluteIndex: number }> = [];
  let subgraphWorkspaceHostMap = new Map<string, HTMLDivElement>();
  let subgraphWorkspaceRuntimeMap = new Map<string, SubgraphWorkspaceRuntime>();
  let subgraphWorkspacePendingEditKeys = new Set<string>();
  let subgraphWorkspaceQueuedEditMap = new Map<string, string>();
  let subgraphWorkspaceRenderSignature = '';
  let subgraphWorkspaceRefreshSignature = '';
  let subgraphWorkspaceRefreshRevision = -1;
  let subgraphWorkspaceRefreshToken = 0;
  let subgraphWorkspaceRequestId = 0;
  let subgraphWorkspaceHeightPx = SUBGRAPH_WORKSPACE_DEFAULT_HEIGHT;
  let isDraggingSubgraphWorkspaceDivider = false;
  let subgraphWorkspaceResizeState: { startClientY: number; startHeightPx: number } | null = null;

  let graphSceneController: ReturnType<typeof createGraphSceneController>;
  let graphRuntimeProbeController: ReturnType<typeof createGraphRuntimeProbeController>;
  let lastFullEditProgressActive = false;
  let lastFullEditIncrementalActive = false;
  let fullEditSettledCleanupHandle: number | null = null;
  let fullEditIdleCleanupHandle: number | null = null;
  let lastFullEditHandledDocumentKey = '';
  let lastFullEditHandledRevision = -1;
  $: {
    const fullEditProgressActive = isFullEditProgressActive();
    if (lastFullEditProgressActive && !fullEditProgressActive) {
      completeStreamProgress();
    }
    lastFullEditProgressActive = fullEditProgressActive;
  }

  const valueTypeToSemType = {
    string: 'str',
    number: 'int',
    boolean: 'boolean',
    null: 'nil',
    object: 'map',
    array: 'seq',
  } as const;

  let renderConfig: GraphViewerConfig = $settings.viewer.graphViewer;
  $: renderConfig = $settings.viewer.graphViewer;
  $: if (subgraphWorkspaceVisiblePanes.length) {
    const nextSubgraphWorkspaceHeightPx = clampSubgraphWorkspaceHeight(subgraphWorkspaceHeightPx);
    if (nextSubgraphWorkspaceHeightPx !== subgraphWorkspaceHeightPx) {
      subgraphWorkspaceHeightPx = nextSubgraphWorkspaceHeightPx;
    }
  }
  const fullBuildReasonSet = new Set([
    'import-file',
    'drop-file',
    'language-switch',
    'whole-document-replacement',
  ]);
  const measureTextSample = GRAPH_CONFIG.measureTextSample;
  let measureRoot: HTMLDivElement;
  let measureRow: HTMLDivElement;
  let measureRowText: HTMLSpanElement;
  let measureHeader: HTMLDivElement;
  let measureHeaderText: HTMLSpanElement;
  let measureSignature = '';

  type GraphSearchTarget = 'node' | 'key' | 'value';

  type GraphSearchResult = {
    nodeId?: number;
    target: GraphSearchTarget;
    label: string;
    path: PathSeg[];
    pathText: string;
  };

  type SubgraphWorkspaceContentState = {
    tabId: string;
    tabName: string;
    sourceText: string;
    valueType: ValueType;
  };

  type SubgraphWorkspacePaneState = {
    requestId?: number;
    path: PathSeg[];
    pathKey: string;
    title: string;
    kind: 'graph' | 'content';
    graph: SubgraphWorkspaceGraphData | null;
    content: SubgraphWorkspaceContentState | null;
    status: 'loading' | 'ready' | 'empty' | 'error';
    error?: string;
  };

  const graphPointerController = createGraphPointerController({
    getPointerEventCtor: () => PointerEventCtor,
    getMoveEventCtor: () => MoveEventCtor,
    getActiveApp: () => leafer as LeaferAppLike | null,
  });

  const graphViewportController = createGraphViewportController({
    getContainer: () => container,
    getLeafer: () =>
      leafer as (LeaferAppLike & { zoomLayer?: LeaferZoomLayer; getValidScale?: (scale: number) => number }) | null,
    getSuppressGraphPointerUntil: () => suppressGraphPointerUntil,
    getMoveEventName: () => (MoveEventCtor?.BEFORE_MOVE ?? MoveEventCtor?.MOVE) as string | undefined,
    getZoomEventName: () => (ZoomEventCtor?.BEFORE_ZOOM ?? ZoomEventCtor?.ZOOM) as string | undefined,
    bindPointerClick: (target, handler) => graphPointerController.bindPointerClick(target, handler),
    updateViewport: (options) => {
      graphMinimapRuntimeController.updateViewport();
      if (options?.redrawEdges) {
        void graphSceneController?.updateViewport();
      }
    },
    getLastAutoOffset: () => lastAutoOffset,
    setLastAutoOffset: (value) => {
      lastAutoOffset = value;
    },
    getPanConstraintBounds: () => {
      const graphData = graphSceneController?.getLastGraphData?.() ?? null;
      const nodes = graphData?.nodes ?? [];
      if (!nodes.length) return null;
      let left = Number.POSITIVE_INFINITY;
      let top = Number.POSITIVE_INFINITY;
      let right = Number.NEGATIVE_INFINITY;
      let bottom = Number.NEGATIVE_INFINITY;
      for (const node of nodes) {
        left = Math.min(left, Number(node.boxArgs.x ?? 0));
        top = Math.min(top, Number(node.boxArgs.y ?? 0));
        right = Math.max(right, Number(node.boxArgs.x ?? 0) + Math.max(0, Number(node.boxArgs.width ?? 0)));
        bottom = Math.max(bottom, Number(node.boxArgs.y ?? 0) + Math.max(0, Number(node.boxArgs.height ?? 0)));
      }
      if (!Number.isFinite(left) || !Number.isFinite(top) || !Number.isFinite(right) || !Number.isFinite(bottom)) {
        return null;
      }
      return { left, top, right, bottom };
    },
  });

  const listClickTargetProbes = () => graphRuntimeProbeController?.listRootClickTargets() ?? [];
  const setGraphHighlightTestState = (path: PathSeg[] | null, target?: GraphHighlightTarget, box?: LeaferBox | null) =>
    graphRuntimeProbeController?.setGraphHighlightTestState(path, target, box);
  const setGraphRevealTestState = (path: PathSeg[] | null, target?: GraphHighlightTarget) =>
    graphRuntimeProbeController?.setGraphRevealTestState(path, target);
  const setGraphRowScrollTestState = (path: PathSeg[] | null, scrollY?: number) =>
    graphRuntimeProbeController?.setGraphRowScrollTestState(path, scrollY);
  const centerOnBox = (box: LeaferBox) => graphViewportController.centerOnBox(box);
  const centerOnNode = (node: GraphNode) => graphViewportController.centerOnNode(node);
  const registerClickTarget = (target: LeaferBox, cell: GraphCell, kind: GraphCellKind, nodeKind?: GraphNode['kind']) =>
    graphRuntimeProbeController?.registerRootClickTarget(target, cell, kind, nodeKind) ?? '';
  const getRuntimeProbeTargets = (scope: 'root' = 'root') =>
    graphRuntimeProbeController?.getRuntimeProbeTargets(scope) ?? [];
  const getSubgraphWorkspaceProbeTargets = (): GraphRuntimeProbeTarget[] => {
    const workspaceHost = document.querySelector("[data-testid='graph-subgraph-workspace']") as HTMLElement | null;
    const workspaceRect = workspaceHost?.getBoundingClientRect() ?? null;
    if (!workspaceRect) return [];
    return subgraphWorkspaceVisiblePanes.flatMap((pane) => {
      const runtime = subgraphWorkspaceRuntimeMap.get(pane.pathKey);
      if (!runtime) return [];
      const app = runtime.app as LeaferAppLike | null;
      return Object.values(runtime.clickTargetsById ?? {}).map((entry) => {
        const path = rebaseSubgraphWorkspacePath(pane.path, entry.cell?.path ?? []);
        const point = getClientProbeCoordFromBoxLike(entry.box, app);
        return {
          scope: 'workspace',
          id: entry.id,
          target: entry.target,
          nodeType: String((entry.box as { tag?: string }).tag ?? ''),
          coord:
            point && workspaceRect
              ? {
                  x: Math.round(point.x - workspaceRect.left),
                  y: Math.round(point.y - workspaceRect.top),
                }
              : null,
          rect: getClientRectFromBoxLike(entry.box, app),
          worldRect: getWorldRectFromBoxLike(entry.box),
          cell: entry.cell
            ? {
                text: resolveGraphCellDisplayText(
                  entry.cell.text,
                  entry.cell.value,
                  String(entry.cell.valueType ?? ''),
                  languageIdValue,
                ),
                valueType: String(entry.cell.valueType ?? ''),
                isTableCell: !!entry.cell.isTableCell,
                isHeader: !!entry.cell.isHeader,
                path,
              }
            : null,
        };
      });
    });
  };
  const getRuntimeHighlightTarget = () => graphRuntimeProbeController?.getRuntimeHighlightTarget() ?? null;
  const getRuntimeRowScrollState = (path?: PathSeg[] | null) =>
    graphRuntimeProbeController?.getRuntimeRowScrollState(path) ?? null;
  const getRuntimeHitResult = (point: { x: number; y: number }) =>
    graphRuntimeProbeController?.getRuntimeHitResult(point) ?? null;
  const clearLastReveal = () => graphRuntimeProbeController?.clearLastReveal();
  const getLastReveal = () => graphRuntimeProbeController?.getLastReveal() ?? null;
  const activateRuntimeProbe = async (probeId: string) => {
    await graphRuntimeProbeController?.activateRuntimeProbe(probeId);
  };
  const commitRuntimeProbe = async (probeId: string, text: string) =>
    (await graphRuntimeProbeController?.commitRuntimeProbe(probeId, text)) ?? false;
  const updateSize = () => graphViewportController.updateSize();
  function requestLeaferRender(): void {
    const target = leafer as ({ update?: () => void; forceRender?: () => void; updateClientBounds?: () => void } & object) | null;
    target?.update?.();
    target?.forceRender?.();
    // Update client bounds for hit-testing (needed for pointer events to find correct elements)
    target?.updateClientBounds?.();
    if (typeof requestAnimationFrame === 'function') {
      requestAnimationFrame(() => {
        if (target === leafer) target?.forceRender?.();
      });
    }
  }
  const registerViewportEvents = (target: LeaferEventTarget) => graphViewportController.registerViewportEvents(target);
  const applyZoom = (changeScale: number) => graphViewportController.applyZoom(changeScale);

  $: if (renderRuntimeReady) {
    graphSceneController.ensureLayers();
    ensureCanvasHint();
  } else {
    resetCanvasHint();
  }

  const graphTextLinkageController = createGraphTextLinkageController({
    getDocumentKey: () => documentKeyValue,
    getSourceText: () => $sourceText ?? '',
    getLanguageId: () => languageIdValue,
    getActiveSnapshotId: () => graphRenderCoordinator.getActiveSnapshotId(),
    getEnableNest: () => $settings.parser.enableNest,
    getRenderConfig: () => renderConfig,
    getNodeDataMap: () => nodeDataMap,
    getNodeBoxMap: () => nodeBoxMap,
    getCellBoxByPathMap: () => cellBoxByPathMap,
    getPathKeyToRenderHandleMap: () => pathKeyToRenderHandleMap,
    getClickTargetProbes: () => listClickTargetProbes(),
    setGraphHighlightTestState,
    setGraphRevealTestState,
    setGraphRowScrollTestState,
    buildPathSegFromCell,
    upsertCellEntry,
    centerOnBox,
    centerOnNode,
    updateLeafer: requestLeaferRender,
    updateActiveTempModel: (updater) => activeTempModel.update(updater),
    getEditorRevision: () => editorRevisionValue,
    getGraphAppliedRevision: () => $graphAppliedRevision,
    getEnableRevealSync: () => enableRevealSync,
    dispatchReveal: (path, target, trigger) => dispatch('reveal', { path, target, trigger }),
    handleError,
  });
  const clearSearchHighlight = graphTextLinkageController.clearSearchHighlight;
  const resolveTreePathByPosition = graphTextLinkageController.resolveTreePathByPosition;
  const ensurePathIndex = graphTextLinkageController.ensurePathIndex;
  const emitReveal = graphTextLinkageController.emitReveal;

  const graphValueEditController = createGraphValueEditController({
    getCurrentData: () => null,
    getSourceText: () => $sourceText ?? '',
    getDocumentKey: () => documentKeyValue,
    getLanguageId: () => languageIdValue,
    getEnableNest: () => $settings.parser.enableNest,
    isReadonly: () => readonly,
    getEditorIO: () => editorIOValue,
    getEditorRevision: () => editorRevisionValue,
    getActiveSnapshotId: () => getWorkspaceSnapshotId(documentKeyValue),
    resolveTreePathByPosition,
    nextTreeStateToken: () => ++treeStateToken,
    publishTreeState,
    emitEditorMutation: editorStore.actions.emitMutation,
    updateActiveTempModel: (updater) => activeTempModel.update(updater),
    dispatchGraphEditEvent: (type, detail) => emitGraphEditEvent(type, detail),
    handleError,
  });
  const hasActiveEdit = graphValueEditController.hasActiveEdit;
  const applyGraphEdit = graphValueEditController.applyGraphEdit;
  const bindGraphEditorLifecycle = graphValueEditController.bindGraphEditorLifecycle;
  const resetActiveEditState = graphValueEditController.resetActiveEditState;
  const subgraphWorkspaceGraphCache = createSubgraphWorkspaceGraphCache({
    getActiveSnapshotId: () => graphRenderCoordinator.getActiveSnapshotId(),
    getDocumentKey: () => documentKeyValue,
    getLanguageId: () => languageIdValue,
    getRevision: () => editorRevisionValue,
    getEnableNest: () => $settings.parser.enableNest,
    getRenderConfig: () => renderConfig,
    inferGraphPaths: (nodes, edges) => graphSceneController.inferGraphPaths(nodes, edges),
  });

  function clearGraphViewerTestHooks(): void {
    clearGraphViewerTestHookState({
      clearRuntimeProbeState: () => graphRuntimeProbeController?.clearTestState(),
    });
  }

  function isFullEditInteractionBlocked(): boolean {
    const state = $fullEditUiState;
    return state?.active === true && Boolean(state.sessionId) && state.phase !== 'idle';
  }

  function isGraphRenderGuardCurrent(guard: GraphRenderGuard | null): boolean {
    if (!guard) return false;
    if (guard.mode === 'json-block') {
      const selection = $jsonBlockSelection;
      return selection?.blockDocumentKey === guard.documentKey && selection.revision === guard.revision;
    }
    return guard.documentKey === documentKeyValue && guard.revision === editorRevisionValue;
  }

  function getGraphInteractionState() {
    const sceneState = graphSceneController.getInteractionState();
    const current = isGraphRenderGuardCurrent(graphRenderGuard);
    const interactiveReady =
      current &&
      sceneState.hasGraphData &&
      !sceneState.pendingRenderWork &&
      sceneState.rootProbeCount > 0;
    return {
      ...graphRenderGuard,
      current,
      ...sceneState,
      interactiveReady,
    };
  }

  function syncGraphReadinessFromInteraction() {
    const interaction = getGraphInteractionState();
    if (!interaction?.documentKey || typeof interaction.revision !== 'number') return interaction;
    syncGraphInteractionReadiness({
      documentKey: interaction.documentKey,
      revision: interaction.revision,
      mode: interaction.mode ?? 'committed',
      hasGraphData: interaction.hasGraphData === true,
      nodeCount: interaction.nodeCount ?? 0,
      pendingRenderWork: interaction.pendingRenderWork ?? true,
      interactiveReady: interaction.interactiveReady === true,
    });
    return interaction;
  }

  function syncSubgraphReadinessForPane(pane: SubgraphWorkspacePaneState | null | undefined): void {
    if (!pane?.pathKey || !pane.requestId) return;
    const interactiveReady =
      pane.kind === 'content'
        ? pane.status === 'ready'
        : pane.status === 'ready' && subgraphWorkspaceRuntimeMap.has(pane.pathKey);
    syncSubgraphInteractionReadiness({
      requestId: pane.requestId,
      pathKey: pane.pathKey,
      sourceRevision: editorRevisionValue,
      interactiveRevision: editorRevisionValue,
      interactiveReady,
    });
  }

  function getRuntimeReadiness() {
    const interaction = syncGraphReadinessFromInteraction();
    const base = readRuntimeReadiness();
    if (base.subgraph.pathKey) {
      syncSubgraphReadinessForPane(subgraphWorkspaceChain.find((pane) => pane.pathKey === base.subgraph.pathKey));
    }
    const next = readRuntimeReadiness();
    return {
      ...next,
      graph: {
        ...next.graph,
        mode: interaction?.mode ?? next.graph.mode,
        pendingRenderWork: interaction?.pendingRenderWork ?? next.graph.pendingRenderWork,
      },
    };
  }


  function isTextClickTarget(target: object): boolean {
    if (TextCtor && target instanceof TextCtor) return true;
    const tag = String((target as { __tag?: string; tag?: string }).__tag ?? (target as { tag?: string }).tag ?? '');
    return tag === 'Text';
  }

  let graphRenderEffects: ReturnType<typeof createGraphViewerRenderEffects>;

  const graphRenderCoordinator = createGraphRenderSession({
    getDocumentKey: () => documentKeyValue,
    getEnableNest: () => $settings.parser.enableNest,
    getRenderConfig: () => renderConfig,
    getJsonBlockSelection: () => $jsonBlockSelection,
    hasRenderTarget: () => renderRuntimeReady,
    shouldAttachGraphViewerTestHooks,
    getGraphStreamState: () => window._treease?.graph.getStreamState() ?? null,
    replaceGraphStreamState,
    nextTreeToken: () => ++treeStateToken,
    publishTreeState,
    clearTreeState,
    resetJsonBlockViewport: () => {
      const layer = leafer?.zoomLayer as LeaferZoomLayer | undefined;
      lastAutoOffset = null;
      if (!layer) return;
      layer.x = 0;
      layer.y = 0;
      layer.scaleX = 1;
      layer.scaleY = 1;
      if (layer.__) {
        layer.__.scaleX = 1;
        layer.__.scaleY = 1;
      }
    },
    callWorker: (method, input) => callSharedWasmWorker(method as any, input),
    onStreamFinalAnalysis: (documentKey, language, revision, analysis) => {
      if (
        !isDocumentRevisionGuardCurrent(
          { documentKey, revision },
          { documentKey: documentKeyValue, revision: editorRevisionValue },
        )
      ) {
        return;
      }
      const requestId = ++treeStateToken;
      void analysis;
      clearTreeState(requestId, 'graph', revision);
    },
    onStreamFinalRedraw: (mode, revision, guard) => {
      if (
        isGraphRenderGuardCurrent(guard) &&
        (mode === 'committed' || mode === 'streaming' || mode === 'json-block')
      ) {
        graphRenderGuard = guard;
        const renderedDocumentKey =
          mode === 'json-block' ? ($jsonBlockSelection?.blockDocumentKey ?? documentKeyValue) : documentKeyValue;
        const renderedText = mode === 'json-block' ? ($jsonBlockSelection?.text ?? '') : $sourceText;
        const renderedLanguage = mode === 'json-block' ? 'json' : languageIdValue;
        graphRenderEffects?.markRendered(renderedDocumentKey, revision, renderedText, renderedLanguage);
        graphAppliedRevision.set(revision);
        replaceGraphStreamState({
          ...(window._treease?.graph.getStreamState() ?? { partialSeen: false, finalSeen: false }),
          appliedAtMs:
            typeof performance !== 'undefined' && typeof performance.now === 'function' ? performance.now() : Date.now(),
        });
      }
    },
    updateStreamProgress: (event) => {
      if ($fullEditUiState?.active && !isFullEditProgressActive()) {
        return;
      }
      graphStreamProgressController.handleEvent(event as any);
    },
    resetStreamProgress: () => {
      graphStreamProgressController.reset();
    },
    completeStreamProgress: () => {
      completeStreamProgress();
    },
    setErrorMessage: (message) => setError(message),
    clearErrorMessage: () => {
      errorMessage = '';
    },
    handleError,
  });

  graphRenderEffects = createGraphViewerRenderEffects({
    shouldAttachGraphViewerTestHooks,
    getGraphStreamState: () => (graphViewerRuntimeApi ? (window._treease?.graph.getStreamState() ?? null) : null),
    replaceGraphStreamState,
    renderDocumentGraph: (input) => graphRenderCoordinator.renderDocumentGraph(input),
    attachFullEditDocumentJobSession: async (input) => {
      const session = getFullEditDocumentJobSession(input);
      if (!session) return null;
      return graphRenderCoordinator.attachExternalDocumentJobSession(session);
    },
    renderJsonBlockSelection: (selection) => graphRenderCoordinator.renderJsonBlockSelection(selection),
    markGraphRequested,
    resetStreamProgress: () => graphStreamProgressController.reset(),
    onStreamingRenderError: (error) => {
      console.error('[GraphViewer] streaming render failed', error);
    },
  });

  graphSceneController = createGraphSceneController({
    getContainer: () => container,
    getLeafer: () => leafer,
    getRenderRoot,
    getBoxCtor: () => BoxCtor,
    getTextCtor: () => TextCtor,
    getPenCtor: () => PenCtor,
    getRenderConfig: () => renderConfig,
    getLanguageId: () => languageIdValue,
    getValueTypeToSemType: () => valueTypeToSemType as Record<string, string>,
    isReadonly: () => readonly,
    getLastAutoOffset: () => lastAutoOffset,
    setLastAutoOffset: (value) => {
      lastAutoOffset = value;
    },
    getLayers: () => ({
      edgeLayer,
      nodeLayer,
      overlayLayer,
    }),
    setLayers: (layers) => {
      if ('edgeLayer' in layers) edgeLayer = (layers.edgeLayer ?? null) as Box | null;
      if ('nodeLayer' in layers) nodeLayer = (layers.nodeLayer ?? null) as Box | null;
      if ('overlayLayer' in layers) overlayLayer = (layers.overlayLayer ?? null) as Box | null;
    },
    buildPathSegFromCell,
    clearSearchHighlight,
    beginMainGraphRedraw: (nodes) => {
      nodeDataMap = new Map(nodes.map((node) => [node.renderHandle, node]));
      nodeBoxMap = new Map();
      pathKeyToRenderHandleMap = new Map();
      cellBoxByPathMap = new Map();
      graphRuntimeProbeController?.resetRootClickTargets();
      nodes.forEach((node) => {
        const pathKey = buildPathKey(node.path ?? []);
        if (!pathKey) return;
        if (!pathKeyToRenderHandleMap.has(pathKey))
          pathKeyToRenderHandleMap.set(pathKey, node.renderHandle);
      });
      return nodeBoxMap;
    },
    getNodeDataMap: () => nodeDataMap,
    getNodeBoxMap: () => nodeBoxMap,
    getPathKeyToRenderHandleMap: () => pathKeyToRenderHandleMap,
    getClickTargetProbes: () => listClickTargetProbes(),
    getClickTargetProbeStore: () => graphRuntimeProbeController?.getRootStore() ?? (Object.create(null) as any),
    upsertCellEntry,
    updateCellEntry,
    upsertClickTargetProbe: (store, targetIds, box, cell, kind) =>
      graphRuntimeProbeController.upsertProbe(store, targetIds, box, cell, kind),
    bindPointerClick: (target, handler) => graphPointerController.bindPointerClick(target, handler),
    toGraphClickTarget,
    getSuppressGraphPointerUntil: () => suppressGraphPointerUntil,
    resolveTreePathByPosition,
    resolveInteractiveCellPath,
    emitReveal,
    registerCellBox,
    unregisterCellBox,
    registerRowBox,
    unregisterRowBox,
    registerClickTarget,
    bindVerticalScrollGesture: (target, handler) => graphPointerController.bindVerticalScrollGesture(target, handler),
    bindPointerDown: (target, handler) => graphPointerController.bindPointerDown(target, handler),
    getPointFromEvent: (hostApp, target, event, space) =>
      graphPointerController.getPointFromEvent(hostApp, target, event, space),
    refreshActiveHighlight: () => graphTextLinkageController.refreshActiveHighlight(),
    updateLeafer: requestLeaferRender,
    handleError,
  });
  graphRenderCoordinator.attachSceneBridge({
    applyGraphDelta: async (delta, version) => {
      const result = await graphSceneController.applyGraphDelta(delta, version);
      ensureCanvasHint();
      if (!isFullEditInteractionBlocked()) graphMinimapRuntimeController.update();
      return result;
    },
    flushPendingRenderWork: () => graphSceneController.flushPendingRenderWork(),
    cancelActiveRenderWork: () => graphSceneController.cancelActiveRenderWork(),
    replaceRenderedGraph: (value) => {
      const result = graphSceneController.replaceAll(value);
      ensureCanvasHint();
      if (!isFullEditInteractionBlocked()) graphMinimapRuntimeController.update();
      return result;
    },
    getLastRenderedGraph: () => graphSceneController.getLastGraphData(),
  });

  const graphMinimapRuntimeController = createGraphMinimapRuntimeController({
    getViewData: () => toMinimapViewData(graphSceneController.getLastGraphData()),
    onViewportChange: () => {
      void graphSceneController.updateViewport();
    },
  });
  let lastSyncedReadonly = readonly;

  function closeGraphInnerEditor(): void {
    const editor = (leafer as ({ editor?: { closeInnerEditor?: (skipUpdate?: boolean) => void } } & object) | null)
      ?.editor;
    editor?.closeInnerEditor?.(true);
  }

  function syncReadonlyEditability(): void {
    resetActiveEditState();
    closeGraphInnerEditor();
    const graphData = graphSceneController.getLastGraphData();
    if (!graphData) return;
    graphSceneController.replaceAll(graphData);
    if (!isFullEditInteractionBlocked()) graphMinimapRuntimeController.update();
  }

  $: if (readonly !== lastSyncedReadonly) {
    lastSyncedReadonly = readonly;
    syncReadonlyEditability();
  }

  graphRuntimeProbeController = createGraphRuntimeProbeController({
    shouldAttachGraphViewerTestHooks,
    isTextClickTarget,
    isFullEditStreaming: () => isFullEditInteractionBlocked(),
    bindPointerClick: (target, handler) => graphPointerController.bindPointerClick(target, handler),
    getContainerRect: () => container?.getBoundingClientRect() ?? null,
    getRootClickTargets: () => listClickTargetProbes(),
    getRootApp: () => leafer as LeaferAppLike | null,
    getLanguageId: () => languageIdValue,
    getCellBoxByPathMap: () => cellBoxByPathMap,
    buildPathKey,
    getClientProbeCoordFromBox: (box, app) => getClientProbeCoordFromBoxLike(box, app),
    getClientRectFromBox: (box, app) => getClientRectFromBoxLike(box, app),
    getWorldRectFromBox: (box) => getWorldRectFromBoxLike(box),
    getClientPointFromWorld: (point) => {
      const worldLeafer = leafer as LeaferAppLike | null;
      if (!point || typeof worldLeafer?.getClientPointByWorld !== 'function') return null;
      const clientPoint = worldLeafer.getClientPointByWorld(point);
      const x = Number(clientPoint?.x);
      const y = Number(clientPoint?.y);
      if (!Number.isFinite(x) || !Number.isFinite(y)) return null;
      return { x, y };
    },
    getViewportWorldCenter: () => {
      const worldLeafer = leafer as LeaferAppOrLeafer & {
        updateClientBounds?: () => void;
        clientBounds?: { x: number; y: number; width: number; height: number };
      };
      worldLeafer.updateClientBounds?.();
      const clientBounds = worldLeafer.clientBounds;
      if (!clientBounds) return null;
      return {
        x: Number(clientBounds.x) + Number(clientBounds.width) / 2,
        y: Number(clientBounds.y) + Number(clientBounds.height) / 2,
      };
    },
    ensurePathIndex: (path) => ensurePathIndex(path),
    resolveTreePathByPosition,
    resolveInteractiveCellPath,
    emitReveal: (path, target, source) => {
      if (source === 'runtime-query') {
        emitReveal(path, target, 'click');
        return;
      }
      emitReveal(path, target, source);
    },
    onRegisteredTargetClick: async ({ path, target, cell, scope }) => {
      if (scope !== 'root') return;
      if (shouldIgnoreSubgraphOpenCell(cell)) return;
      void cell;
      void target;
      await openSubgraphWorkspacePath(path, -1);
    },
    commitProbe: async ({ cell, kind }, text) => {
      if (kind !== 'key' && kind !== 'value') return false;
      emitGraphEditEvent('graph-edit-open', {
        path: cell.path,
        kind,
        valueType: cell.valueType,
      });
      return applyGraphEdit(cell, kind, text, null);
    },
  });

  function getLeaferContentRoot(target: LeaferAppOrLeafer | null): Box | null {
    if (!target) return null;
    const zoomLayer = (target as LeaferAppOrLeafer & { zoomLayer?: Box }).zoomLayer;
    if (zoomLayer) return zoomLayer;
    return 'add' in target ? (target as unknown as Box) : null;
  }

  function upsertCellEntry(
    map: Map<string, CellBoxEntry>,
    cell: GraphCell,
    updater: (entry: CellBoxEntry) => void,
  ): void {
    upsertCellEntryInIndex(map, cell, updater);
  }

  function updateCellEntry(
    map: Map<string, CellBoxEntry>,
    cell: GraphCell,
    updater: (entry: CellBoxEntry) => void,
  ): void {
    updateCellEntryInIndex(map, cell, updater);
  }

  function toGraphClickTarget(kind: GraphCellKind): 'key' | 'value' | 'node' {
    if (kind === 'key') return 'key';
    if (kind === 'value') return 'value';
    return 'node';
  }

  function buildPathSegFromCell(cell: GraphCell | undefined, rowIndex: number): PathSeg | null {
    const raw = String(cell?.text ?? '').trim();
    if (!raw) return cell?.isIndex ? ({ tag: PathSegTag.INDEX, key: '' as any, index: rowIndex } as PathSeg) : null;
    const bracketMatch = raw.match(/^\[(\d+)\]$/);
    if (bracketMatch) {
      return { tag: PathSegTag.INDEX, key: '' as any, index: Number.parseInt(bracketMatch[1], 10) } as PathSeg;
    }
    if (cell?.isIndex && /^\d+$/.test(raw)) {
      return { tag: PathSegTag.INDEX, key: '' as any, index: Number.parseInt(raw, 10) } as PathSeg;
    }
    return { tag: PathSegTag.KEY, key: raw as any, index: 0 } as PathSeg;
  }

  function resolveInteractiveCellPath(cell: GraphCell, fallbackPath: PathSeg[]): Promise<PathSeg[]> {
    return resolveInteractiveCellPathWithFallback(cell, fallbackPath, resolveTreePathByPosition);
  }

  function publishTreeState(
    requestId: number,
    tree: TreeNode | null,
    value: unknown,
    source: 'editor' | 'graph',
    revision: number,
    snapshotId?: SnapshotId | null,
  ) {
    void tree;
    void value;
    void snapshotId;
    const accepted = requestId === treeStateToken;
    if (!accepted) return false;
    treeState.set({ tree: null, value: null, source, revision });
    return true;
  }

  function clearTreeState(
    requestId: number,
    source: 'editor' | 'graph',
    revision: number,
    snapshotId?: SnapshotId | null,
  ) {
    void snapshotId;
    const accepted = requestId === treeStateToken;
    if (!accepted) return false;
    treeState.set({ tree: null, value: null, source, revision });
    return true;
  }

  function getRenderRoot(): Box | null {
    return getLeaferContentRoot(leafer);
  }

  function isFullEditProgressActive(): boolean {
    return $fullEditUiState?.active === true && $fullEditUiState.phase !== 'idle';
  }

  function completeStreamProgress(): void {
    graphStreamProgressController.completeIfActive();
  }

  function scheduleFullEditCleanup(kind: 'settled' | 'idle', task: () => void): void {
    if (kind === 'settled') {
      if (fullEditSettledCleanupHandle != null) return;
      fullEditSettledCleanupHandle = requestAnimationFrame(() => {
        fullEditSettledCleanupHandle = null;
        task();
      });
      return;
    }
    if (fullEditIdleCleanupHandle != null) return;
    fullEditIdleCleanupHandle = requestAnimationFrame(() => {
      fullEditIdleCleanupHandle = null;
      task();
    });
  }

  function ensureCanvasHint(): void {
    if (!container || !leafer || !overlayLayer || !TextCtor) return;
    if (canvasHintText && canvasHintText.parent === overlayLayer) return;
    const zoomLayer = (leafer as LeaferAppLike & { zoomLayer?: LeaferZoomLayer }).zoomLayer;
    if (!zoomLayer) return;
    const { scaleX, scaleY } = getZoomScale(zoomLayer);
    if (!Number.isFinite(scaleX) || !Number.isFinite(scaleY) || scaleX <= 0 || scaleY <= 0) return;
    const hintWidth = 320;
    const offsetX = zoomLayer.x ?? 0;
    const offsetY = zoomLayer.y ?? 0;
    const worldX = (container.clientWidth / 2 - offsetX) / scaleX - hintWidth / 2;
    const worldY = (16 - offsetY) / scaleY;
    const hint = new TextCtor({
      text: 'Hold Space and drag to move the canvas',
      width: hintWidth,
      fontSize: 12,
      fontWeight: '500',
      textAlign: 'center',
      fill: '#94a3b8',
      hittable: false,
      hitSelf: false,
      hitChildren: false,
    });
    hint.x = worldX;
    hint.y = worldY;
    overlayLayer.add(hint);
    canvasHintText = hint;
  }

  function resetCanvasHint(): void {
    canvasHintText?.remove?.();
    canvasHintText = null;
  }


  function toMinimapViewData(graphData: GraphSceneViewData | null): MinimapViewData | null {
    if (!graphData) return null;

    // minimap 纵向间距系数
    const SPREAD = 1.1;
    const nodes = graphData.nodes;

    const centerY = nodes.length > 0 ? nodes.reduce((sum, n) => sum + n.boxArgs.y, 0) / nodes.length : 0;

    const yDeltaByHandle = new Map<number, number>();
    const spreadNodes = nodes.map((node) => {
      const handle = node.renderHandle;
      const origY = node.boxArgs.y;
      const spreadY = centerY + (origY - centerY) * SPREAD;
      yDeltaByHandle.set(handle, spreadY - origY);
      return {
        id: handle,
        kind: node.kind,
        x: node.boxArgs.x,
        y: spreadY,
        width: node.boxArgs.width,
        height: node.boxArgs.height,
      };
    });

    return {
      nodes: spreadNodes,
      edges: graphData.edges.map((edge) => {
        const fromDelta = yDeltaByHandle.get(edge.fromRenderHandle) ?? 0;
        const toDelta = yDeltaByHandle.get(edge.toRenderHandle) ?? 0;
        return {
          fromX: edge.bezierArgs.fromX,
          fromY: edge.bezierArgs.fromY + fromDelta,
          c1x: edge.bezierArgs.c1x,
          c1y: edge.bezierArgs.c1y + fromDelta,
          c2x: edge.bezierArgs.c2x,
          c2y: edge.bezierArgs.c2y + toDelta,
          toX: edge.bezierArgs.toX,
          toY: edge.bezierArgs.toY + toDelta,
        };
      }),
    };
  }

  const graphMeasurementController = createGraphMeasurementController({
    getElements: () => ({ measureRoot, measureRow, measureRowText, measureHeader, measureHeaderText }),
    getRenderConfig: () => renderConfig,
    getMeasureTextSample: () => measureTextSample,
    tick,
    saveGraphViewerConfig: (graphViewer) => settingsStore.save({ viewer: { graphViewer } }),
  });

  function scheduleMeasure() {
    graphMeasurementController.scheduleMeasure();
  }

  function emitGraphEditEvent(
    type: 'graph-edit-open' | 'graph-edit-commit' | 'graph-edit-replace-fallback' | 'graph-edit-probes',
    detail: unknown,
  ) {
    if (!container) return;
    container.dispatchEvent(new CustomEvent(type, { detail, bubbles: true }));
  }

  /**
   * 注册可高亮的单元格。
   * @param cell 单元格数据
   * @param kind 单元格类型
   * @param box 单元格容器
   * @returns void
   */
  function registerCellBox(cell: GraphCell, kind: GraphCellKind, box: LeaferBox): void {
    registerCellBoxEntry(cellBoxByPathMap, cell, kind, box);
  }

  function unregisterCellBox(cell: GraphCell, kind: GraphCellKind, box: LeaferBox): void {
    unregisterCellBoxEntry(cellBoxByPathMap, cell, kind, box);
  }

  function registerRowBox(
    cell: GraphCell,
    rowBox: LeaferBox,
    scrollOwner?: ScrollableBox,
    bodyHeight?: number,
    contentHeight?: number,
  ): void {
    registerRowBoxEntry(cellBoxByPathMap, cell, rowBox, scrollOwner, bodyHeight, contentHeight);
  }

  function unregisterRowBox(cell: GraphCell, rowBox: LeaferBox): void {
    unregisterRowBoxEntry(cellBoxByPathMap, cell, rowBox);
  }


  function getClientProbeCoord(box: LeaferBox): { x: number; y: number } | null {
    const absoluteProbe = getClientProbeCoordFromBoxLike(box, leafer as LeaferAppLike | null);
    const containerRect = container?.getBoundingClientRect();
    if (!absoluteProbe || !containerRect) return null;
    return {
      x: Math.round(absoluteProbe.x - containerRect.left),
      y: Math.round(absoluteProbe.y - containerRect.top),
    };
  }

  async function buildSubgraphWorkspaceContentState(path: PathSeg[]): Promise<SubgraphWorkspaceContentState | null> {
    const snapshotId = getWorkspaceSnapshotId(documentKeyValue);
    const pathValue = await queryPathValue({ documentKey: documentKeyValue, snapshotId, path });
    if (pathValue.status !== 'ready' || !pathValue.data) return null;
    const valueType = pathValue.data.valueType as ValueType;
    if (!shouldOpenSubgraphWorkspaceContent(pathValue.data)) return null;
    return {
      tabId: `subgraph-content:${buildPathKey(path)}`,
      tabName: formatSubgraphWorkspacePath(path, renderConfig),
      sourceText: pathValue.data.displayText,
      valueType,
    };
  }

  function syncSubgraphWorkspaceTransientState(nextChain: SubgraphWorkspacePaneState[]): void {
    const nextKeys = new Set(nextChain.map((pane) => pane.pathKey));
    for (const pathKey of subgraphWorkspacePendingEditKeys) {
      if (!nextKeys.has(pathKey)) subgraphWorkspacePendingEditKeys.delete(pathKey);
    }
    for (const pathKey of subgraphWorkspaceQueuedEditMap.keys()) {
      if (!nextKeys.has(pathKey)) subgraphWorkspaceQueuedEditMap.delete(pathKey);
    }
  }

  function updateVisibleSubgraphWorkspacePanes(): void {
    const start = Math.max(0, subgraphWorkspaceChain.length - 3);
    subgraphWorkspaceVisiblePanes = subgraphWorkspaceChain.slice(start).map((pane, index) => ({
      ...pane,
      visibleIndex: index,
      absoluteIndex: start + index,
    }));
  }

  function disposeSubgraphWorkspaceRuntimes(exceptPathKeys: string[] = []): void {
    const preserved = new Set(exceptPathKeys);
    for (const [pathKey, runtime] of subgraphWorkspaceRuntimeMap.entries()) {
      if (preserved.has(pathKey)) continue;
      destroySubgraphWorkspaceRuntime(runtime);
      subgraphWorkspaceRuntimeMap.delete(pathKey);
    }
  }

  function setSubgraphWorkspaceChain(nextChain: SubgraphWorkspacePaneState[]): void {
    const previousRequestIds = new Map(
      subgraphWorkspaceChain
        .filter((pane) => typeof pane.requestId === 'number')
        .map((pane) => [pane.pathKey, pane.requestId as number]),
    );
    const normalizedChain = nextChain.map((pane) => ({
      ...pane,
      requestId: pane.requestId ?? previousRequestIds.get(pane.pathKey),
    }));
    syncSubgraphWorkspaceTransientState(normalizedChain);
    subgraphWorkspaceChain = normalizedChain;
    updateVisibleSubgraphWorkspacePanes();
    disposeSubgraphWorkspaceRuntimes(normalizedChain.map((pane) => pane.pathKey));
  }

  async function prepareSubgraphWorkspacePane(path: PathSeg[]): Promise<SubgraphWorkspacePaneState | null> {
    const pathKey = buildPathKey(path);
    if (!pathKey) return null;
    const title = formatSubgraphWorkspacePath(path, renderConfig);
    const content = await buildSubgraphWorkspaceContentState(path);
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
      const graph = await subgraphWorkspaceGraphCache.prepareGraph(path);
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
      handleError(error, {
        component: 'GraphViewer',
        operation: 'buildSubgraphWorkspaceProjection',
        metadata: { documentKey: documentKeyValue, language: languageIdValue, pathKey },
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

  async function openSubgraphWorkspacePath(path: PathSeg[], parentAbsoluteIndex: number): Promise<void> {
    const pathKey = buildPathKey(path);
    if (!pathKey) return;
    const currentChild = subgraphWorkspaceChain[parentAbsoluteIndex + 1] ?? null;
    if (currentChild?.pathKey === pathKey) return;
    const requestId = ++subgraphWorkspaceRequestId;
    markSubgraphRequested({
      requestId,
      pathKey,
      sourceRevision: editorRevisionValue,
    });
    const base = parentAbsoluteIndex >= 0 ? subgraphWorkspaceChain.slice(0, parentAbsoluteIndex + 1) : [];
    const loadingPane: SubgraphWorkspacePaneState = {
      requestId,
      path,
      pathKey,
      title: formatSubgraphWorkspacePath(path, renderConfig),
      kind: 'graph',
      graph: null,
      content: null,
      status: 'loading',
    };
    setSubgraphWorkspaceChain([...base, loadingPane]);
    const pane = await prepareSubgraphWorkspacePane(path);
    if (!pane) return;
    const latestBase = parentAbsoluteIndex >= 0 ? subgraphWorkspaceChain.slice(0, parentAbsoluteIndex + 1) : [];
    if (latestBase.some((entry, index) => base[index]?.pathKey !== entry.pathKey)) return;
    const nextPane = { ...pane, requestId };
    markSubgraphMaterialized({
      requestId,
      pathKey,
      sourceRevision: editorRevisionValue,
      materializedRevision: editorRevisionValue,
    });
    setSubgraphWorkspaceChain([...latestBase, nextPane]);
    await tick();
    await renderSubgraphWorkspacePanes();
    syncSubgraphReadinessForPane(nextPane);
  }

  function closeSubgraphWorkspacePane(absoluteIndex: number): void {
    if (absoluteIndex < 0 || absoluteIndex >= subgraphWorkspaceChain.length) return;
    setSubgraphWorkspaceChain(subgraphWorkspaceChain.slice(0, absoluteIndex));
  }

  function clampSubgraphWorkspaceHeight(nextHeightPx: number): number {
    return getClampedPaneSize(
      nextHeightPx,
      graphViewerShellHeight,
      SUBGRAPH_WORKSPACE_MIN_HEIGHT,
      SUBGRAPH_WORKSPACE_MAX_HEIGHT_FRACTION,
    );
  }

  async function handleSubgraphWorkspaceActivate(
    payload: { path: PathSeg[]; target: 'key' | 'value' | 'node'; cell: GraphCell },
    parentAbsoluteIndex: number,
  ): Promise<void> {
    if (shouldIgnoreSubgraphOpenCell(payload.cell)) return;
    emitReveal(payload.path, payload.target, 'click');
    void payload.cell;
    await openSubgraphWorkspacePath(payload.path, parentAbsoluteIndex);
  }

  async function commitSubgraphWorkspaceValueEdit(pane: SubgraphWorkspacePaneState, draft?: string): Promise<void> {
    if (readonly || pane.kind !== 'content' || !pane.content) return;
    const nextText = draft ?? pane.content.sourceText;
    if (nextText === pane.content.sourceText) return;
    if (subgraphWorkspacePendingEditKeys.has(pane.pathKey)) {
      subgraphWorkspaceQueuedEditMap.set(pane.pathKey, nextText);
      return;
    }
    subgraphWorkspacePendingEditKeys.add(pane.pathKey);
    try {
      let nextDraft = nextText;
      if (pane.content.valueType === 'string') {
        try {
          const parsed = JSON.parse(nextDraft);
          if (typeof parsed === 'string') nextDraft = parsed;
        } catch {}
      }
      await applyGraphEdit(
        {
          text: pane.content.sourceText,
          value: pane.content.sourceText,
          valueType: pane.content.valueType,
          path: pane.path,
          editable: !readonly,
          boxArgs: { x: 0, y: 0, width: 0, height: 0, cornerRadius: 0 },
          textArgs: {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            text: pane.content.sourceText,
            textAlign: 'left',
            verticalAlign: 'top',
            editable: !readonly,
          },
        },
        'value',
        nextDraft,
      );
    } finally {
      subgraphWorkspacePendingEditKeys.delete(pane.pathKey);
    }
    const queuedText = subgraphWorkspaceQueuedEditMap.get(pane.pathKey);
    if (queuedText == null || queuedText === nextText) return;
    subgraphWorkspaceQueuedEditMap.delete(pane.pathKey);
    await commitSubgraphWorkspaceValueEdit(pane, queuedText);
  }

  function bindSubgraphWorkspaceHost(pathKey: string, host: HTMLDivElement | null): void {
    if (host) {
      subgraphWorkspaceHostMap.set(pathKey, host);
    } else {
      subgraphWorkspaceHostMap.delete(pathKey);
      const runtime = subgraphWorkspaceRuntimeMap.get(pathKey);
      destroySubgraphWorkspaceRuntime(runtime);
      subgraphWorkspaceRuntimeMap.delete(pathKey);
    }
    subgraphWorkspaceRenderSignature = '';
    void renderSubgraphWorkspacePanes();
  }

  function subgraphWorkspaceHostAction(node: HTMLDivElement, pathKey: string) {
    let currentPathKey = pathKey;
    bindSubgraphWorkspaceHost(currentPathKey, node);
    return {
      update(nextPathKey: string) {
        if (nextPathKey === currentPathKey) return;
        bindSubgraphWorkspaceHost(currentPathKey, null);
        currentPathKey = nextPathKey;
        bindSubgraphWorkspaceHost(currentPathKey, node);
      },
      destroy() {
        bindSubgraphWorkspaceHost(currentPathKey, null);
      },
    };
  }

  function handleSubgraphWorkspaceDividerDragStart(clientY: number) {
    isDraggingSubgraphWorkspaceDivider = true;
    subgraphWorkspaceResizeState = {
      startClientY: clientY,
      startHeightPx: subgraphWorkspaceHeightPx,
    };
  }

  function handleSubgraphWorkspaceDividerDragMove(clientY: number) {
    if (!subgraphWorkspaceResizeState) return;
    const deltaY = subgraphWorkspaceResizeState.startClientY - clientY;
    subgraphWorkspaceHeightPx = clampSubgraphWorkspaceHeight(subgraphWorkspaceResizeState.startHeightPx + deltaY);
  }

  function handleSubgraphWorkspaceDividerDragEnd() {
    isDraggingSubgraphWorkspaceDivider = false;
    subgraphWorkspaceResizeState = null;
  }

  async function renderSubgraphWorkspacePanes(): Promise<void> {
    const visibleSignature = subgraphWorkspaceVisiblePanes
      .map((pane) => `${pane.pathKey}:${pane.status}:${pane.absoluteIndex}`)
      .join('|');
    const nextSignature = `${visibleSignature}|${Boolean(LeaferCtor || PlainLeaferCtor)}|${Boolean(BoxCtor)}|${Boolean(TextCtor)}|${Boolean(PenCtor)}|${readonly}`;
    if (nextSignature === subgraphWorkspaceRenderSignature) return;
    subgraphWorkspaceRenderSignature = nextSignature;
    const activeKeys = subgraphWorkspaceVisiblePanes
      .filter((pane) => pane.status === 'ready' && pane.graph)
      .map((pane) => pane.pathKey);
    disposeSubgraphWorkspaceRuntimes(activeKeys);
    for (const pane of subgraphWorkspaceVisiblePanes) {
      syncSubgraphReadinessForPane(pane);
      if (pane.kind === 'content') continue;
      const mount = subgraphWorkspaceHostMap.get(pane.pathKey);
      if (!mount) continue;
      if (pane.status !== 'ready' || !pane.graph) {
        mount.replaceChildren();
        continue;
      }
      const existingRuntime = subgraphWorkspaceRuntimeMap.get(pane.pathKey) as (SubgraphWorkspaceRuntime & {
        __graphRef?: SubgraphWorkspaceGraphData | null;
      }) | undefined;
      if (existingRuntime && existingRuntime.host === mount && existingRuntime.__graphRef === pane.graph) {
        continue;
      }
      destroySubgraphWorkspaceRuntime(existingRuntime);
      const runtime = await renderSubgraphWorkspaceGraph(mount, pane.graph, {
        getConstructors: () => ({ LeaferCtor, PlainLeaferCtor, BoxCtor, TextCtor, PenCtor }),
        getRenderConfig: () => renderConfig,
        getLanguageId: () => languageIdValue,
        getValueTypeToSemType: () => valueTypeToSemType as Record<string, string>,
        isReadonly: () => readonly,
        bindGraphEditorLifecycle,
        bindPointerClick: (target, handler) => graphPointerController.bindPointerClick(target, handler),
        getMoveEventName: () => (MoveEventCtor?.BEFORE_MOVE ?? MoveEventCtor?.MOVE) as string | undefined,
        bindVerticalScrollGesture: (target, handler) => graphPointerController.bindVerticalScrollGesture(target, handler),
        bindPointerDown: (target, handler) => graphPointerController.bindPointerDown(target, handler),
        getPointFromEvent: (hostApp, target, event, space) =>
          graphPointerController.getPointFromEvent(hostApp, target, event, space),
        resolveInteractiveCellPath,
        onActivateCell: (payload) => handleSubgraphWorkspaceActivate(payload, pane.absoluteIndex),
      });
      if (runtime) {
        (runtime as SubgraphWorkspaceRuntime & { __graphRef?: SubgraphWorkspaceGraphData | null }).__graphRef =
          pane.graph;
        subgraphWorkspaceRuntimeMap.set(pane.pathKey, runtime);
        syncSubgraphReadinessForPane(pane);
      }
    }
  }

  async function refreshSubgraphWorkspacePanes(): Promise<void> {
    if (!subgraphWorkspaceChain.length) return;
    const token = ++subgraphWorkspaceRefreshToken;
    const nextChain: SubgraphWorkspacePaneState[] = [];
    for (const pane of subgraphWorkspaceChain) {
      const nextPane = await prepareSubgraphWorkspacePane(pane.path);
      if (token !== subgraphWorkspaceRefreshToken) return;
      if (nextPane) nextChain.push(nextPane);
    }
    setSubgraphWorkspaceChain(nextChain);
    await tick();
    await renderSubgraphWorkspacePanes();
  }
  
  export function revealSearchResult(result: GraphSearchResult): void {
    if (isFullEditInteractionBlocked()) return;
    graphTextLinkageController.revealSearchResult(result);
  }

  export function revealPath(
    path: PathSeg[],
    options?: { target?: 'key' | 'value' | 'node'; navigate?: boolean },
  ): void {
    if (isFullEditInteractionBlocked()) return;
    graphTextLinkageController.revealPath(path, options);
  }

  const graphViewerRuntimeApi = {
    getClickProbeTargets: (scope?: 'root' | 'workspace') =>
      scope === 'workspace' ? getSubgraphWorkspaceProbeTargets() : getRuntimeProbeTargets(scope ?? 'root'),
    getHighlightTarget: () => getRuntimeHighlightTarget(),
    getLastReveal: () => getLastReveal(),
    clearLastReveal: () => clearLastReveal(),
    getRowScrollState: (path?: PathSeg[] | null) => getRuntimeRowScrollState(path),
    getHitResult: (point: { x: number; y: number }) => getRuntimeHitResult(point),
    getLastGraphData: () => graphSceneController.getLastGraphData(),
    getInteractionState: () => getGraphInteractionState(),
    getRuntimeReadiness: () => getRuntimeReadiness(),
    getStreamProgressState: () => streamProgressState,
    revealPath: (path: PathSeg[], options?: { target?: 'key' | 'value' | 'node'; navigate?: boolean }) =>
      revealPath(path, options),
    activateProbe: (probeId: string) => activateRuntimeProbe(probeId),
    commitProbe: (probeId: string, text: string) => commitRuntimeProbe(probeId, text),
    scrollTableToRow: (rowIndex: number) => graphSceneController.scrollTableToRow(rowIndex),
    refs: {
      get leafer() {
        return leafer;
      },
      get layers() {
        return {
          edgeLayer,
          nodeLayer,
          overlayLayer,
        };
      },
    },
  };

  $: if (container) {
    installGraphBridge(graphViewerRuntimeApi);
  }

  export async function exportImage() {
    if (!leafer) return;
    const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
    await leafer.export(`treease-${timestamp}.png`, { trim: true, padding: [5, 5, 5, 5] });
  }

  function setError(message: string) {
    errorMessage = message;
    graphSceneController.clear();
    ensureCanvasHint();
    clearSearchHighlight();
  }

  export function zoomIn() {
    applyZoom(1.1);
  }

  export function zoomOut() {
    applyZoom(1 / 1.1);
  }

  onDestroy(() => {
    if (fullEditSettledCleanupHandle != null) cancelAnimationFrame(fullEditSettledCleanupHandle);
    if (fullEditIdleCleanupHandle != null) cancelAnimationFrame(fullEditIdleCleanupHandle);
    resetCanvasHint();
    graphMeasurementController.dispose();
    graphRenderEffects.dispose();
    void graphRenderCoordinator.dispose();
    graphSceneController.dispose();
    resetActiveEditState();
    disposeSubgraphWorkspaceRuntimes();
    subgraphWorkspaceGraphCache.clear();
    unsubscribeGraphStreamProgress();
    graphStreamProgressController.dispose();
    clearGraphBridge();
    resetGraphStreamState();
  });

  $: {
    // Reset auto-position guard for full rebuild operations (e.g. import, language switch, paste-all),
    // so updateAutoPosition re-positions the viewport on the fresh graph instead of keeping old offset.
    if ($fullEditUiState?.active && $fullEditUiState.phase === 'streaming' && lastAutoOffset != null &&
        fullBuildReasonSet.has($fullEditUiState.reason)) {
      lastAutoOffset = null;
    }
    graphRenderEffects.maybeAttachFullEditSession($fullEditUiState, {
      hasRenderRuntime: renderRuntimeReady,
      documentKey: documentKeyValue,
      language: languageIdValue,
      sourceText: $sourceText,
    });
  }

  $: if ($settings) {
    subgraphWorkspaceGraphCache.clear();
    subgraphWorkspaceRenderSignature = '';
    void refreshSubgraphWorkspacePanes();
  }

  $: {
    const nextRefreshSignature = [
      documentKeyValue,
      languageIdValue,
      editorRevisionValue,
      $graphAppliedRevision,
      $settings.parser.enableNest ? 'nest' : 'flat',
      buildSubgraphWorkspaceRenderSignature(renderConfig),
    ].join('|');
    if (nextRefreshSignature !== subgraphWorkspaceRefreshSignature) {
      subgraphWorkspaceRefreshSignature = nextRefreshSignature;
      subgraphWorkspaceRenderSignature = '';
      if (subgraphWorkspaceRefreshRevision !== $graphAppliedRevision) {
        subgraphWorkspaceRefreshRevision = $graphAppliedRevision;
        void refreshSubgraphWorkspacePanes();
      }
    }
  }
  $: {
    const fullEditProgressActive = isFullEditProgressActive();

    // Detect full-edit session end (active→inactive transition).
    // finishFullEditStream resets directly to idle, bypassing the 'settled'
    // phase that the $: block at line 1009 checks. Without this update,
    // lastFullEditHandled* is never set, fullEditHandled is always false,
    // and maybeRenderIncremental fires a redundant full rebuild.
    if (lastFullEditIncrementalActive && !fullEditProgressActive) {
      lastFullEditHandledDocumentKey = documentKeyValue;
      lastFullEditHandledRevision = editorRevisionValue;
    }
    lastFullEditIncrementalActive = fullEditProgressActive;

    const fullEditHandled =
      lastFullEditHandledDocumentKey === documentKeyValue && lastFullEditHandledRevision === editorRevisionValue;
    graphRenderEffects.maybeRenderJsonBlock($jsonBlockSelection, renderRuntimeReady);
    graphRenderEffects.maybeRenderIncremental({
      hasRenderRuntime: renderRuntimeReady,
      isBlocked:
        isFullEditProgressActive() ||
        fullEditHandled ||
        Boolean($jsonBlockSelection),
      documentKey: documentKeyValue,
      language: languageIdValue,
      sourceText: $sourceText,
      editorRevision: editorRevisionValue,
      graphAppliedRevision: $graphAppliedRevision,
    });
  }
  $: if ($fullEditUiState?.active && $fullEditUiState.phase === 'settled') {
    lastFullEditHandledDocumentKey = $fullEditUiState.documentKey ?? documentKeyValue;
    lastFullEditHandledRevision = $fullEditUiState.revision;
    scheduleFullEditCleanup('settled', () => {
      completeStreamProgress();
      graphMinimapRuntimeController.update();
    });
  }

  $: if ($fullEditUiState?.active && $fullEditUiState.phase === 'idle') {
    scheduleFullEditCleanup('idle', () => {
      completeStreamProgress();
    });
  }

  $: {
    const graphHighlight = $activeTempModel?.graphHighlight ?? null;
    const graphHighlightSignature = buildGraphHighlightSignature(graphHighlight, buildPathKey);
    const appliedRevision = $graphAppliedRevision;
    if (isFullEditInteractionBlocked()) {
      clearSearchHighlight();
      lastAppliedGraphHighlightSignature = '';
      lastAppliedGraphHighlightRevision = -1;
    } else if (!graphHighlightSignature) {
      clearSearchHighlight();
      lastAppliedGraphHighlightSignature = '';
      lastAppliedGraphHighlightRevision = -1;
    } else if (!enableRevealSync) {
      lastAppliedGraphHighlightSignature = '';
      lastAppliedGraphHighlightRevision = -1;
    } else if (
      shouldApplyGraphHighlight({
        hasLeafer: Boolean(leafer),
        isBlocked: false,
        graphHighlight,
        graphHighlightSignature,
        appliedRevision,
        lastAppliedSignature: lastAppliedGraphHighlightSignature,
        lastAppliedRevision: lastAppliedGraphHighlightRevision,
      })
    ) {
      lastAppliedGraphHighlightSignature = graphHighlightSignature;
      lastAppliedGraphHighlightRevision = appliedRevision;
      revealPath(graphHighlight.path, {
        target: graphHighlight.target,
        navigate: graphHighlight.source === 'search' || graphHighlight.source === 'breadcrumb',
      });
    }
  }

  $: if (measureRoot) {
    const nextSignature = [
      renderConfig.fontFamily,
      renderConfig.layout.baseFontSize,
      renderConfig.layout.rowPaddingInline,
      renderConfig.layout.rowPaddingBlock,
    ].join('|');
    if (nextSignature !== measureSignature) {
      measureSignature = nextSignature;
      scheduleMeasure();
    }
  }

  $: {
    const runtimeStateSignature = `${graphRuntimeReady ? 'ready' : 'loading'}|${errorMessage}`;
    if (runtimeStateSignature !== lastRuntimeStateSignature) {
      lastRuntimeStateSignature = runtimeStateSignature;
      dispatch('runtime-state', {
        ready: graphRuntimeReady,
        loading: !graphRuntimeReady && !errorMessage,
        error: Boolean(errorMessage),
      });
    }
  }
</script>

<div
  bind:this={graphViewerShell}
  bind:clientHeight={graphViewerShellHeight}
  class="graph-viewer-shell"
  class:graph-viewer-shell--with-workspace={subgraphWorkspaceVisiblePanes.length > 0}
  data-testid="graph-viewer-root"
>
  <div class="graph-viewer-main">
    <div
      class="absolute inset-0 z-0"
      class:invisible={showRuntimeLoading}
      class:pointer-events-none={showRuntimeLoading}
    >
      <div bind:this={container} class="absolute inset-0 touch-none" data-testid="graph-viewer-canvas"></div>
      <div
        bind:this={minimapHost}
        class="pointer-events-auto absolute bottom-4 right-4 z-[2] h-[150px] w-[220px] overflow-hidden rounded-[14px]
          shadow-[0_12px_28px_rgba(15,23,42,0.14)] backdrop-blur"
        class:hidden={streamProgressState.visible ||
          (isFullEditProgressActive() && $fullEditUiState?.phase !== 'settled')}
        data-testid="graph-viewer-minimap"
      ></div>
      <GraphRuntimeHost
        {container}
        {minimapHost}
        bind:graphRuntimeReady
        bind:errorMessage
        bind:leafer
        bind:LeaferCtor
        bind:PlainLeaferCtor
        bind:BoxCtor
        bind:TextCtor
        bind:PenCtor
        bind:MoveEventCtor
        bind:ZoomEventCtor
        bind:DragEventCtor
        bind:LeaferEventCtor
        bind:PointerEventCtor
        minimapRuntimeController={graphMinimapRuntimeController}
        {registerViewportEvents}
        {bindGraphEditorLifecycle}
        {updateSize}
        {scheduleMeasure}
        minimapWidth={MINIMAP_WIDTH}
        minimapHeight={MINIMAP_HEIGHT}
      />
    </div>
    {#if showRuntimeLoading}
      <GraphRuntimeLoading />
    {/if}
    <GraphStreamProgressOverlay state={streamProgressState} />
    {#if errorMessage}
      <div
        data-testid="graph-error-message"
        class="absolute left-4 top-4 rounded-[10px] border border-[#e2e8f0] bg-white px-3 py-2 text-[12px] text-[#0f172a]
        shadow-[0_8px_24px_rgba(15,23,42,0.12)] font-mono"
      >
        {errorMessage}
      </div>
    {/if}
  </div>

  {#if subgraphWorkspaceVisiblePanes.length}
    <div
      class={`app-split-divider app-split-divider--horizontal graph-subgraph-workspace__divider ${
        isDraggingSubgraphWorkspaceDivider ? 'app-split-divider--dragging' : ''
      }`}
      role="separator"
      aria-label="Resize subgraph workspace"
      aria-orientation="horizontal"
      use:splitLayoutDrag={{
        onDragStart: ({ clientY }) => handleSubgraphWorkspaceDividerDragStart(clientY),
        onDragMove: ({ clientY }) => handleSubgraphWorkspaceDividerDragMove(clientY),
        onDragEnd: () => handleSubgraphWorkspaceDividerDragEnd(),
      }}
    >
    </div>
    <div
      class="graph-subgraph-workspace"
      data-testid="graph-subgraph-workspace"
      style:height={`${subgraphWorkspaceHeightPx}px`}
      transition:fly={{ y: 18, duration: 180, opacity: 0.14, easing: cubicOut }}
    >
      <div class="graph-subgraph-workspace__track">
        {#each subgraphWorkspaceVisiblePanes as pane (pane.pathKey)}
          <section class="graph-subgraph-pane" data-testid="graph-subgraph-pane">
            <header class="graph-subgraph-pane__header" title={buildReadablePath(pane.path)}>
              <span class="graph-subgraph-pane__label">{pane.title}</span>
              <button
                type="button"
                class="graph-subgraph-pane__close"
                aria-label="Close subgraph pane"
                on:click|stopPropagation={() => closeSubgraphWorkspacePane(pane.absoluteIndex)}
              >
                <X size={14} strokeWidth={2} />
              </button>
            </header>
            <div class="graph-subgraph-pane__body">
              {#if pane.kind === 'content' && pane.status === 'ready' && pane.content}
                <div class="graph-subgraph-pane__content" data-testid="graph-subgraph-content-pane">
                  <div class="graph-subgraph-pane__content-editor" data-testid="graph-subgraph-monaco-pane">
                    <SidecarEditor
                      tabId={pane.content.tabId}
                      tabName={pane.content.tabName}
                      language={languageIdValue}
                      sourceText={pane.content.sourceText}
                      runtimeHookId={pane.content.tabId}
                      containerTestId="graph-subgraph-monaco-editor"
                      attachToPane={false}
                      destroyOnUnmount={true}
                      onScroll={() => {}}
                      onContentChange={(text) => {
                        emitReveal(pane.path, 'value', 'click');
                        void commitSubgraphWorkspaceValueEdit(pane, text);
                      }}
                      onEditorBlur={(text) => void commitSubgraphWorkspaceValueEdit(pane, text)}
                    />
                  </div>
                </div>
              {:else if pane.status === 'loading'}
                <div class="graph-subgraph-pane__placeholder">Loading subgraph…</div>
              {:else if pane.status === 'empty'}
                <div class="graph-subgraph-pane__placeholder">No subgraph data</div>
              {:else if pane.status === 'error'}
                <div class="graph-subgraph-pane__placeholder graph-subgraph-pane__placeholder--error">
                  {pane.error ?? 'Subgraph render failed'}
                </div>
              {/if}
              {#if pane.kind === 'graph'}
                <div
                  class="graph-subgraph-pane__canvas"
                  class:hidden={pane.status !== 'ready'}
                  use:subgraphWorkspaceHostAction={pane.pathKey}
                ></div>
              {/if}
            </div>
          </section>
        {/each}
      </div>
    </div>
  {/if}

  <div bind:this={measureRoot} class="absolute -left-[9999px] -top-[9999px] pointer-events-none opacity-0">
    <div bind:this={measureRow} class="inline-block">
      <span bind:this={measureRowText} class="inline-block"></span>
    </div>
    <div bind:this={measureHeader} class="inline-block font-semibold">
      <span bind:this={measureHeaderText} class="inline-block"></span>
    </div>
  </div>
</div>

<style>
  .graph-viewer-shell {
    position: relative;
    display: grid;
    height: 100%;
    width: 100%;
    min-height: 0;
    background: #f8fafc;
    grid-template-rows: minmax(0, 1fr) auto;
  }

  .graph-viewer-shell--with-workspace {
    grid-template-rows: minmax(0, 1fr) auto auto;
  }

  .graph-viewer-main {
    position: relative;
    min-height: 0;
    overflow: hidden;
  }

  .graph-subgraph-workspace {
    min-height: 0;
    overflow: hidden;
    border-top: 1px solid rgba(148, 163, 184, 0.28);
    background:
      linear-gradient(180deg, rgba(241, 245, 249, 0.96), rgba(236, 242, 247, 0.92) 12px, rgba(244, 247, 251, 0.98) 56px),
      linear-gradient(90deg, rgba(148, 163, 184, 0.1), rgba(255, 255, 255, 0));
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.76),
      inset 0 10px 20px rgba(148, 163, 184, 0.08),
      0 -12px 24px rgba(15, 23, 42, 0.08);
  }

  :global(.graph-subgraph-workspace__divider) {
    background: transparent;
  }

  .graph-subgraph-workspace__track {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: minmax(360px, 1fr);
    gap: 0;
    height: 100%;
    overflow-x: auto;
    overflow-y: hidden;
  }

  .graph-subgraph-pane {
    display: grid;
    min-width: 360px;
    min-height: 0;
    grid-template-rows: auto minmax(0, 1fr);
    border-right: 1px solid rgba(203, 213, 225, 0.72);
    background: linear-gradient(180deg, rgba(255, 255, 255, 0.86), rgba(247, 250, 252, 0.82));
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.68);
  }

  .graph-subgraph-pane__header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    overflow: hidden;
    padding: 4px 8px 4px 12px;
    border-bottom: 1px solid rgba(226, 232, 240, 0.9);
    color: #5f7087;
    font-size: 11px;
    line-height: 1.4;
    background: linear-gradient(180deg, rgba(255, 255, 255, 0.92), rgba(241, 245, 249, 0.82));
  }

  .graph-subgraph-pane__label {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .graph-subgraph-pane__close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    flex: 0 0 auto;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: #94a3b8;
    cursor: pointer;
    transition:
      background-color 120ms ease,
      color 120ms ease;
  }

  .graph-subgraph-pane__close:hover {
    background: rgba(226, 232, 240, 0.9);
    color: #334155;
  }

  .graph-subgraph-pane__close:focus-visible {
    outline: 2px solid rgba(59, 130, 246, 0.4);
    outline-offset: 1px;
  }

  .graph-subgraph-pane__close :global(svg) {
    pointer-events: none;
  }

  .graph-subgraph-pane__body {
    position: relative;
    min-height: 0;
    overflow: hidden;
  }

  .graph-subgraph-pane__content {
    display: grid;
    height: 100%;
    min-height: 0;
    background:
      radial-gradient(circle at top left, rgba(148, 163, 184, 0.14), transparent 44%),
      linear-gradient(180deg, rgba(255, 255, 255, 0.95), rgba(243, 247, 250, 0.98));
  }

  .graph-subgraph-pane__content-editor {
    min-height: 0;
    padding: 10px 12px 12px;
  }

  .graph-subgraph-pane__content-editor :global([data-testid='graph-subgraph-monaco-editor']) {
    height: 100%;
    border: 1px solid #dbe3ef;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.95);
    overflow: hidden;
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.75);
  }

  .graph-subgraph-pane__canvas {
    height: 100%;
    overflow: hidden;
  }

  .graph-subgraph-pane__placeholder {
    display: grid;
    place-items: center;
    height: 100%;
    padding: 16px;
    color: #64748b;
    font-size: 12px;
    background:
      radial-gradient(circle at top left, rgba(148, 163, 184, 0.14), transparent 42%),
      linear-gradient(180deg, rgba(238, 243, 249, 0.9), rgba(250, 252, 255, 0.92));
  }

  .graph-subgraph-pane__placeholder--error {
    color: #b91c1c;
  }

  .graph-subgraph-pane__canvas.hidden {
    display: none;
  }

  :global(.graph-subgraph-pane-view) {
    min-width: 100%;
    min-height: 100%;
  }

</style>
