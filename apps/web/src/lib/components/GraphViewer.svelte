<!-- 职责：GraphViewer 稳定入口组件：Leafer 生命周期、controller 装配、跨域编排、DOM 模板 -->
<script lang="ts">
  import { onDestroy, tick, createEventDispatcher } from 'svelte';
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
  import { callSharedWasmWorker, getSharedWasmWorkerClient } from '../wasm/wasm-worker-singleton';
  import { getActiveDocumentSnapshotId } from '../services/DocumentSessionService';
  import { getFullEditDocumentJobSession } from '../graph-stream/full-edit-document-job-session';
  import { type PathSeg } from '../store/tree-path';
  import { type MinimapViewData } from '../leafer-minimap';
  import GraphRuntimeHost from './graph-viewer/GraphRuntimeHost.svelte';
  import GraphRuntimeLoading from './graph-viewer/GraphRuntimeLoading.svelte';
  import GraphStreamProgressOverlay from './graph-viewer/GraphStreamProgressOverlay.svelte';
  import { buildGraphTooltipPanelShellMarkup, canOpenSubgraphPreviewForCell, createGraphHoverPanelController } from './graph-viewer/graph-hover-panel';
  import { createGraphPointerController, type LeaferEventTarget } from './graph-viewer/graph-pointer-controller';
  import { createGraphRuntimeProbeController } from './graph-viewer/graph-runtime-probe-controller';
  import {
    createGraphStreamProgressController,
    type GraphStreamProgressState,
  } from './graph-viewer/graph-stream-progress';
  import { createGraphTextLinkageController } from './graph-viewer/graph-text-linkage';
  import { createGraphTooltipRuntimeController } from './graph-viewer/graph-tooltip-runtime-controller';
  import { createGraphMinimapRuntimeController } from './graph-viewer/graph-minimap-runtime-controller';
  import { createGraphValueEditController } from './graph-viewer/graph-value-edit';
  import { createGraphViewportController, type LeaferZoomLayer } from './graph-viewer/graph-viewport-controller';
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
  import { createGraphRenderSession } from './graph-viewer/graph-render-session';
  import { createGraphSceneController } from './graph-viewer/graph-scene';
  import type { GraphSceneViewData } from './graph-viewer/graph-scene-runtime';
  import { createGraphMeasurementController } from './graph-viewer/graph-measurement-controller';
  import {
    buildGraphHighlightSignature,
    shouldApplyGraphHighlight,
    shouldRunHoverPanelPrewarm,
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
    LeaferInteractiveTarget,
    LeaferText,
    ScrollableBox,
  } from './graph-viewer/model';
  import type {
    GraphRuntimeHoverPanelDebugState,
    GraphRuntimeHoverPreviewState,
  } from './graph-viewer/runtime/scene-types';
  import { editorLanguageFallback, type SupportedEditorLanguageId } from '../monaco/language-support';
  import type { EditorIO, GraphHighlightTarget } from '../store/editor-store';
  import { handleError } from '../utils/error-handler';
  import { GRAPH_CONFIG } from '../config/constants';
  import { type GraphCell, type GraphCellKind, type GraphNode } from '../graph/graph-viewer-render';
  import { buildPathKey, buildTooltipContent } from '../graph/graph-viewer-path';
  import { isDocumentRevisionGuardCurrent } from '../guards/document-revision-guard';
  import {
    clearGraphBridge,
    installGraphBridge,
    replaceGraphStreamState,
    resetGraphStreamState,
  } from '../test-bridge/register-graph-bridge';
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
  export let synchronizedRuntimeLoading = false;

  const MINIMAP_WIDTH = 220;
  const MINIMAP_HEIGHT = 150;

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
  let currentData: unknown = null;
  let documentKeyValue = '';
  let languageIdValue: SupportedEditorLanguageId = editorLanguageFallback;
  let editorRevisionValue = 0;
  let editorIOValue: EditorIO | null = null;
  let pendingHoverPanelPrewarmRevision = -1;
  let lastAutoOffset: { x: number; y: number } | null = null;
  let edgeLayer: Box | null = null;
  let nodeLayer: Box | null = null;
  let overlayLayer: Box | null = null;
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
  let runtimeHoverPreviewState: GraphRuntimeHoverPreviewState | null = null;
  let runtimeHoverPanelDebugState: GraphRuntimeHoverPanelDebugState = { phase: 'idle', error: '' };
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

  let graphSceneController: ReturnType<typeof createGraphSceneController>;
  let hoverPanelController: ReturnType<typeof createGraphHoverPanelController>;
  let graphRuntimeProbeController: ReturnType<typeof createGraphRuntimeProbeController>;
  let lastFullEditProgressActive = false;
  let lastFullEditIncrementalActive = false;
  let fullEditSettledCleanupHandle: number | null = null;
  let fullEditIdleCleanupHandle: number | null = null;
  let lastFullEditHandledDocumentKey = '';
  let lastFullEditHandledRevision = -1;
  $: {
    const fullEditProgressActive = $fullEditUiState?.active === true && $fullEditUiState.phase !== 'idle';
    if (lastFullEditProgressActive && !fullEditProgressActive) {
      graphStreamProgressController.completeIfActive();
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
  const getRuntimeProbeTargets = (scope: 'root' | 'panel' = 'root') =>
    graphRuntimeProbeController?.getRuntimeProbeTargets(scope) ?? [];
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
    dispatchReveal: (path, target, trigger) => dispatch('reveal', { path, target, trigger }),
    handleError,
  });
  const clearSearchHighlight = graphTextLinkageController.clearSearchHighlight;
  const resolveTreePathByPosition = graphTextLinkageController.resolveTreePathByPosition;
  const ensurePathIndex = graphTextLinkageController.ensurePathIndex;
  const hydrateResolvedGraphPaths = graphTextLinkageController.hydrateResolvedGraphPaths;
  const emitReveal = graphTextLinkageController.emitReveal;

  const graphValueEditController = createGraphValueEditController({
    getCurrentData: () => currentData,
    getSourceText: () => $sourceText ?? '',
    getDocumentKey: () => documentKeyValue,
    getLanguageId: () => languageIdValue,
    getEnableNest: () => $settings.parser.enableNest,
    getEditorIO: () => editorIOValue,
    getEditorRevision: () => editorRevisionValue,
    getActiveSnapshotId: () => getActiveDocumentSnapshotId(documentKeyValue),
    resolveTreePathByPosition,
    nextTreeStateToken: () => ++treeStateToken,
    publishTreeState,
    emitEditorMutation: editorStore.actions.emitMutation,
    updateActiveTempModel: (updater) => activeTempModel.update(updater),
    refreshTooltipVisibility: () => graphTooltipRuntimeController.refreshVisibility(),
    dispatchGraphEditEvent: (type, detail) => emitGraphEditEvent(type, detail),
    handleError,
  });
  const hasActiveEdit = graphValueEditController.hasActiveEdit;
  const bindGraphEditorLifecycle = graphValueEditController.bindGraphEditorLifecycle;
  const resetActiveEditState = graphValueEditController.resetActiveEditState;
  const commitTooltipPanelProbe = graphValueEditController.commitTooltipPanelProbe;

  function clearGraphViewerTestHooks(): void {
    clearGraphViewerTestHookState({
      clearRuntimeProbeState: () => graphRuntimeProbeController?.clearTestState(),
      setRuntimeHoverPreviewState: (state) => {
        runtimeHoverPreviewState = state;
      },
      setRuntimeHoverPanelDebugState: (state) => {
        runtimeHoverPanelDebugState = state;
      },
    });
  }

  function nowMs(): number {
    return typeof performance !== 'undefined' && typeof performance.now === 'function' ? performance.now() : Date.now();
  }

  function isFullEditInteractionBlocked(): boolean {
    const state = $fullEditUiState;
    return state?.active === true && Boolean(state.sessionId) && state.phase !== 'idle';
  }


  function isTextClickTarget(target: object): boolean {
    if (TextCtor && target instanceof TextCtor) return true;
    const tag = String((target as { __tag?: string; tag?: string }).__tag ?? (target as { tag?: string }).tag ?? '');
    return tag === 'Text';
  }

  let graphRenderEffects: ReturnType<typeof createGraphViewerRenderEffects>;

  const graphRenderCoordinator = createGraphRenderSession({
    getContainer: () => container,
    getLanguageId: () => languageIdValue,
    getDocumentKey: () => documentKeyValue,
    getEnableNest: () => $settings.parser.enableNest,
    getRenderConfig: () => renderConfig,
    getJsonBlockSelection: () => $jsonBlockSelection,
    hasRenderTarget: () => renderRuntimeReady,
    shouldAttachGraphViewerTestHooks,
    getGraphStreamState: () => window._treease?.graph.getStreamState() ?? null,
    replaceGraphStreamState,
    clearGraphViewerTestHooks: () => {
      clearGraphViewerTestHooks();
    },
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
    getWorkerClient: () => getSharedWasmWorkerClient(),
    hydrateResolvedGraphPaths: async (nodes, text) => {
      return await hydrateResolvedGraphPaths(nodes, text);
    },
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
      if (analysis?.tree) {
        publishTreeState(requestId, analysis.tree as TreeNode, analysis.value, 'graph', revision);
        return;
      }
      clearTreeState(requestId, 'graph', revision);
    },
    onStreamFinalRedraw: (mode, revision) => {
      if (
        isDocumentRevisionGuardCurrent(
          { documentKey: documentKeyValue, revision },
          { documentKey: documentKeyValue, revision: editorRevisionValue },
        ) &&
        (mode === 'committed' || mode === 'streaming' || mode === 'json-block')
      ) {
        const renderedDocumentKey =
          mode === 'json-block' ? ($jsonBlockSelection?.blockDocumentKey ?? documentKeyValue) : documentKeyValue;
        const renderedText = mode === 'json-block' ? ($jsonBlockSelection?.text ?? '') : $sourceText;
        const renderedLanguage = mode === 'json-block' ? 'json' : languageIdValue;
        graphRenderEffects?.markRendered(renderedDocumentKey, revision, renderedText, renderedLanguage);
        graphAppliedRevision.set(revision);
        pendingHoverPanelPrewarmRevision = revision;
        replaceGraphStreamState({
          ...(window._treease?.graph.getStreamState() ?? { partialSeen: false, finalSeen: false }),
          appliedAtMs: nowMs(),
        });
      } else {
      }
    },
    updateStreamProgress: (event) => {
      if ($fullEditUiState?.active && $fullEditUiState.phase === 'idle') {
        return;
      }
      graphStreamProgressController.handleEvent(event as any);
    },
    resetStreamProgress: () => {
      graphStreamProgressController.reset();
    },
    completeStreamProgress: () => {
      graphStreamProgressController.completeIfActive();
    },
    clearGraphStateEffects: () => {
      activeTempModel.update((current) => ({ ...current, treePath: [], graphHighlight: null }));
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
      if (!isFullEditInteractionBlocked()) graphMinimapRuntimeController.update();
      return result;
    },
    cancelActiveRenderWork: () => graphSceneController.cancelActiveRenderWork(),
    replaceRenderedGraph: (value) => {
      const result = graphSceneController.replaceAll(value);
      if (!isFullEditInteractionBlocked()) graphMinimapRuntimeController.update();
      return result;
    },
    getLastRenderedGraph: () => graphSceneController.getLastGraphData(),
  });

  hoverPanelController = createGraphHoverPanelController({
    getCurrentData: () => currentData,
    getLanguageId: () => languageIdValue,
    getActiveSnapshotId: () => graphRenderCoordinator.getActiveSnapshotId(),
    getSourceText: () => $sourceText ?? '',
    getDocumentKey: () => documentKeyValue,
    getRevision: () => editorRevisionValue,
    getEnableNest: () => $settings.parser.enableNest,
    getRenderConfig: () => renderConfig,
    getSettings: () => $settings,
    getConstructors: () => ({ LeaferCtor, PlainLeaferCtor, BoxCtor, TextCtor, PenCtor }),
    getTooltipEvents: () => ({ LeaferEvent: LeaferEventCtor, PointerEvent: PointerEventCtor }),
    getValueTypeToSemType: () => valueTypeToSemType as Record<string, string>,
    getRootClickTargets: () => listClickTargetProbes(),
    setRuntimeHoverPreviewState: (preview) => {
      runtimeHoverPreviewState = preview;
    },
    setRuntimeHoverPanelDebugState: (state) => {
      runtimeHoverPanelDebugState = state;
    },
    bindGraphEditorLifecycle,
    canOpenSubgraphPreviewForCell,
    resolveTreePathByPosition,
    resolveInteractiveCellPath,
    inferGraphPaths: graphSceneController.inferGraphPaths,
    upsertCellEntry,
    updateCellEntry,
    registerPanelClickTarget: (store, targetIds, boundTargets, box, cell, kind, nodeKind) =>
      graphRuntimeProbeController.registerPanelClickTarget(store, targetIds, boundTargets, box, cell, kind, nodeKind),
    bindVerticalScrollGesture: (target, handler) => graphPointerController.bindVerticalScrollGesture(target, handler),
    bindPointerDown: (target, handler) => graphPointerController.bindPointerDown(target, handler),
    getPointFromEvent: (hostApp, target, event, space) =>
      graphPointerController.getPointFromEvent(hostApp, target, event, space),
    refreshTooltipPosition: () => graphTooltipRuntimeController.refreshPosition(),
    handleError,
  });
  const graphTooltipRuntimeController = createGraphTooltipRuntimeController({
    hoverPanelController,
    resolveTooltipHoverTarget,
    getTooltipContent: getRuntimeTooltipContent,
    hasActiveEdit,
    isFullEditInteractionBlocked,
    setRuntimeHoverPanelDebugState: (state) => {
      runtimeHoverPanelDebugState = state as GraphRuntimeHoverPanelDebugState;
    },
  });
  const graphMinimapRuntimeController = createGraphMinimapRuntimeController({
    getViewData: () => toMinimapViewData(graphSceneController.getLastGraphData()),
    onViewportChange: () => {
      void graphSceneController.updateViewport();
    },
  });

  graphRuntimeProbeController = createGraphRuntimeProbeController({
    shouldAttachGraphViewerTestHooks,
    isTextClickTarget,
    isFullEditStreaming: () => isFullEditInteractionBlocked(),
    bindPointerClick: (target, handler) => graphPointerController.bindPointerClick(target, handler),
    getContainerRect: () => container?.getBoundingClientRect() ?? null,
    getRootClickTargets: () => listClickTargetProbes(),
    getPanelClickTargets: () =>
      (
        hoverPanelController as {
          getTooltipPanelClickTargets?: () => ReturnType<typeof listClickTargetProbes>;
        }
      ).getTooltipPanelClickTargets?.() ?? [],
    getPanelPath: () =>
      (hoverPanelController as { getTooltipPanelPath?: () => PathSeg[] }).getTooltipPanelPath?.() ?? [],
    getPanelApp: () =>
      (hoverPanelController as { getTooltipPanelApp?: () => LeaferAppLike | null }).getTooltipPanelApp?.() ?? null,
    getRootApp: () => leafer as LeaferAppLike | null,
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
    commitTooltipPanelProbe,
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
    void snapshotId;
    const accepted = requestId === treeStateToken;
    if (!accepted) return false;
    currentData = value;
    treeState.set({ tree, value, source, revision });
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
    currentData = null;
    treeState.set({ tree: null, value: null, source, revision });
    return true;
  }

  function getRenderRoot(): Box | null {
    return getLeaferContentRoot(leafer);
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

  function emitGraphEditEvent(type: 'graph-edit-open' | 'graph-edit-commit' | 'graph-edit-probes', detail: unknown) {
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

  function resolveInteractiveHoverTarget(target: LeaferInteractiveTarget | null): LeaferText | null {
    let current = target;
    while (current) {
      if (current.__graphCell && current.__graphCellKind) {
        return current as LeaferText;
      }
      current = current.parent ?? null;
    }
    return null;
  }

  function resolveTooltipHoverTarget(node: LeaferInteractiveTarget | null): LeaferText | null {
    return resolveInteractiveHoverTarget(node);
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
    getClickProbeTargets: (scope?: 'root' | 'panel') => getRuntimeProbeTargets(scope ?? 'root'),
    getHighlightTarget: () => getRuntimeHighlightTarget(),
    getLastReveal: () => getLastReveal(),
    clearLastReveal: () => clearLastReveal(),
    getRowScrollState: (path?: PathSeg[] | null) => getRuntimeRowScrollState(path),
    getHoverPreview: () => runtimeHoverPreviewState,
    getHoverPanelDebugState: () => runtimeHoverPanelDebugState,
    getHoverPanelPrewarmDebugSnapshot: () => hoverPanelController.getTooltipPanelPrewarmDebugSnapshot?.() ?? null,
    getHoverPanelRuntimeDebugSnapshot: () => hoverPanelController.getTooltipPanelRuntimeDebugSnapshot?.() ?? null,
    getHitResult: (point: { x: number; y: number }) => getRuntimeHitResult(point),
    getLastGraphData: () => graphSceneController.getLastGraphData(),
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
    clearSearchHighlight();
  }

  export function zoomIn() {
    applyZoom(1.1);
  }

  export function zoomOut() {
    applyZoom(1 / 1.1);
  }

  function getRuntimeTooltipContent(target: LeaferText | null): string {
    const previewTarget = hoverPanelController.resolveGraphHoverPreviewTarget(target);
    if (previewTarget?.previewKind === 'subgraph') {
      return buildGraphTooltipPanelShellMarkup();
    }
    return buildTooltipContent(currentData, target as LeaferText, languageIdValue) as string;
  }

  onDestroy(() => {
    if (fullEditSettledCleanupHandle != null) cancelAnimationFrame(fullEditSettledCleanupHandle);
    if (fullEditIdleCleanupHandle != null) cancelAnimationFrame(fullEditIdleCleanupHandle);
    graphMeasurementController.dispose();
    graphRenderEffects.dispose();
    void graphRenderCoordinator.dispose();
    graphSceneController.dispose();
    resetActiveEditState();
    hoverPanelController.disposeTooltipEditor();
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
    hoverPanelController.applyTheme($settings);
  }
  $: {
    const fullEditProgressActive = $fullEditUiState?.active === true && $fullEditUiState.phase !== 'idle';

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
        ($fullEditUiState?.active === true && $fullEditUiState.phase !== 'idle') ||
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
    if (fullEditSettledCleanupHandle == null)
      fullEditSettledCleanupHandle = requestAnimationFrame(() => {
        fullEditSettledCleanupHandle = null;
        graphStreamProgressController.completeIfActive();
        graphMinimapRuntimeController.update();
      });
  }

  $: if ($fullEditUiState?.active && $fullEditUiState.phase === 'idle') {
    if (fullEditIdleCleanupHandle == null)
      fullEditIdleCleanupHandle = requestAnimationFrame(() => {
        fullEditIdleCleanupHandle = null;
        graphStreamProgressController.completeIfActive();
      });
  }

  $: if (
    shouldRunHoverPanelPrewarm({
      pendingRevision: pendingHoverPanelPrewarmRevision,
      editorRevision: editorRevisionValue,
      graphAppliedRevision: $graphAppliedRevision,
      isBlocked: isFullEditInteractionBlocked(),
    })
  ) {
    const revision = pendingHoverPanelPrewarmRevision;
    pendingHoverPanelPrewarmRevision = -1;
    requestAnimationFrame(() => {
      if (editorRevisionValue !== revision || isFullEditInteractionBlocked() || $graphAppliedRevision < revision) {
        return;
      }
      hoverPanelController.scheduleTooltipPanelPrewarm();
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

<div class="relative h-full w-full bg-[#f8fafc]" data-testid="graph-viewer-root">
  <div
    class="absolute inset-0 z-0"
    class:invisible={showRuntimeLoading}
    class:pointer-events-none={showRuntimeLoading}
  >
    <div bind:this={container} class="absolute inset-0 touch-none" data-testid="graph-viewer-canvas"></div>
    <div
      bind:this={minimapHost}
      class="pointer-events-auto absolute bottom-4 right-4 z-[2] h-[150px] w-[220px] overflow-hidden rounded-[14px]
        border border-[#cbd5e1] bg-white/95 shadow-[0_12px_28px_rgba(15,23,42,0.14)] backdrop-blur"
      class:hidden={streamProgressState.visible ||
        ($fullEditUiState?.active === true && $fullEditUiState.phase !== 'idle' && $fullEditUiState.phase !== 'settled')}
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
      tooltipRuntimeController={graphTooltipRuntimeController}
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
  <div bind:this={measureRoot} class="absolute -left-[9999px] -top-[9999px] pointer-events-none opacity-0">
    <div bind:this={measureRow} class="inline-block">
      <span bind:this={measureRowText} class="inline-block"></span>
    </div>
    <div bind:this={measureHeader} class="inline-block font-semibold">
      <span bind:this={measureHeaderText} class="inline-block"></span>
    </div>
  </div>
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

<style>
  :global(.leafer-x-tooltip) {
    max-width: 520px;
    padding: 8px;
    overflow: hidden;
    pointer-events: auto;
  }

  :global(.leafer-x-tooltip *) {
    pointer-events: auto;
  }

  :global(.leafer-x-tooltip.leafer-x-tooltip--interactive) {
    pointer-events: auto;
  }

  :global(.leafer-x-tooltip.leafer-x-tooltip--interactive *) {
    pointer-events: auto;
  }

  :global(.leafer-x-tooltip .graph-tooltip-panel-shell) {
    overflow: hidden;
    border-radius: 8px;
  }

  :global(.leafer-x-tooltip .graph-tooltip-pre-shell) {
    max-width: min(640px, calc(100vw - 48px));
    max-height: 320px;
    overflow: auto;
    border-radius: 6px;
  }

  :global(.leafer-x-tooltip .graph-tooltip-pre-shell pre) {
    width: max-content;
    min-width: 100%;
    margin: 0;
    padding: 8px;
    white-space: pre;
  }

  :global(.leafer-x-tooltip .graph-tooltip-panel) {
    min-width: 120px;
    min-height: 80px;
    overflow: hidden;
  }

  :global(.leafer-x-tooltip .graph-tooltip-panel--loading) {
    display: flex;
    align-items: stretch;
    justify-content: stretch;
  }

  :global(.leafer-x-tooltip .graph-tooltip-panel-skeleton) {
    display: flex;
    width: 240px;
    min-width: 120px;
    min-height: 96px;
    flex-direction: column;
    gap: 10px;
    padding: 14px;
    box-sizing: border-box;
    background: linear-gradient(180deg, rgba(248, 250, 252, 0.98), rgba(241, 245, 249, 0.98));
  }

  :global(.leafer-x-tooltip .graph-tooltip-panel-skeleton__bar) {
    height: 12px;
    border-radius: 999px;
    background: linear-gradient(90deg, rgba(203, 213, 225, 0.7), rgba(226, 232, 240, 0.95), rgba(203, 213, 225, 0.7));
  }

  :global(.leafer-x-tooltip .graph-tooltip-panel-skeleton__bar--wide) {
    width: 100%;
  }

  :global(.leafer-x-tooltip .graph-tooltip-panel-skeleton__bar--short) {
    width: 58%;
  }

  :global(.leafer-x-tooltip .graph-tooltip-panel-view) {
    width: 100%;
    height: 100%;
  }

  :global(.leafer-x-tooltip .graph-tooltip-code-shell) {
    width: 480px;
    max-width: 480px;
    max-height: 320px;
    overflow: auto;
    border-radius: 6px;
  }

  :global(.leafer-x-tooltip .graph-tooltip-code) {
    width: 480px;
    max-width: 480px;
    min-height: 44px;
  }

  :global(.leafer-x-tooltip .graph-tooltip-meta-path) {
    color: #6b7280;
  }

  :global(.leafer-x-tooltip .graph-tooltip-key) {
    color: #a31515;
  }

  :global(.leafer-x-tooltip .graph-tooltip-value-boolean) {
    color: #0451a5;
  }

  :global(.leafer-x-tooltip .graph-tooltip-value-null) {
    color: #0451a5;
  }

  :global(.leafer-x-tooltip .graph-tooltip-value-string) {
    color: #0451a5;
  }

  :global(.leafer-x-tooltip .graph-tooltip-value-number) {
    color: #098658;
  }
</style>
