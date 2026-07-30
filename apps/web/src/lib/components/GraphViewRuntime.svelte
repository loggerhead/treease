<!-- Responsibility: own the Leafer lifecycle, controller assembly, cross-boundary orchestration, and DOM template. -->
<script lang="ts">
  import { onDestroy, onMount, tick, createEventDispatcher } from 'svelte';
  import { cubicOut } from 'svelte/easing';
  import { fly } from 'svelte/transition';
  import { ChevronLeft, ChevronRight, X } from 'lucide-svelte';
  import {
    documentKey as documentKeyStore,
    editorIO,
    emitEditorMutation,
    editorRevision,
    graphAppliedRevision,
    languageId as languageIdStore,
    sourceText,
  } from '../store/document-session-store';
  import { activeTempModel, treeState } from '../store/graph-selection-store';
  import { activeDocumentSemanticStateByKey } from '../store/active-document-authority';
  import { fullEditUiState, jsonBlockSelection } from '../store/full-edit-ui-store';

  import { type GraphViewerConfig } from '../settings/ui-settings';
  import { settings, settingsStore } from '../settings/settings-store';
  import { shouldShowGraphRuntimeLoading, type RuntimeStateEventDetail } from '../runtime-loading';
  import { callSharedWasmWorker } from '../wasm/wasm-worker-singleton';
  import { getWorkspaceSnapshotId } from '../store/workspace-store';
  import { getFullEditDocumentJobSession } from '../graph-stream/full-edit-document-job-session';
  import {
    buildReadablePath,
    isPathSegIndex,
    pathSegKeyValue,
    type PathSeg,
  } from '../store/tree-path';
  import { type MinimapViewData } from '../leafer-minimap';
  import { GraphRuntimeHost, GraphRuntimeLoading } from './graph-viewer/runtime';
  import GraphStreamProgressOverlay from './graph-viewer/GraphStreamProgressOverlay.svelte';
  import SidecarEditor from './Editor/SidecarEditor.svelte';
  import { splitLayoutDrag } from './ui/split-layout';
  import {
    buildGraphHighlightSignature,
    createGraphFullEditRuntime,
    createGraphPointerController,
    createGraphStreamProgressController,
    createGraphTreeStateController,
    createGraphTextLinkageController,
    createGraphViewportController,
    shouldApplyGraphHighlight,
    type GraphStreamProgressState,
    type LeaferEventTarget,
    type LeaferZoomLayer,
  } from './graph-viewer/interaction';
  import { createWebGraphEditAdapter } from './graph-viewer/web-graph-edit-adapter';
  import {
    createGraphRenderBindings,
    createGraphSceneRenderDeps,
    createGraphRenderState,
    createGraphRenderSession,
    createGraphSceneController,
    createGraphTextLinkageRenderDeps,
    createGraphViewerRenderEffects,
    getClientProbeCoordFromBoxLike,
    getClientRectFromBoxLike,
    getWorldRectFromBoxLike,
    getZoomScale,
    resolveInteractiveCellPath as resolveInteractiveCellPathWithFallback,
    toGraphClickTarget,
    toMinimapViewData,
    type GraphRenderGuard,
  } from './graph-viewer/rendering';
  import {
    createGraphViewRuntimeLifecycle,
    disposeGraphViewRuntime,
    isGraphViewRuntimeRenderCurrent,
  } from './graph-viewer/graph-view-runtime-lifecycle';
  import type { GraphSceneViewData } from './graph-viewer/rendering';
  import {
    buildClientProbeCoord,
    clearCanvasHintOverlay,
    createGraphMeasurementController,
    createGraphMinimapRuntimeController,
    createGraphRuntimeProbeActions,
    createGraphRuntimeProbeController,
    dispatchGraphEditEvent,
    ensureCanvasHintOverlay,
    exportLeaferImage,
    getLeaferContentRoot,
  } from './graph-viewer/runtime';
  import {
    buildPathSegFromCell,
    buildWorkspacePathPrefixes,
    createSubgraphWorkspaceController,
    shouldResetSubgraphWorkspaceForFullEdit,
    workspacePathKey,
    type SubgraphWorkspacePaneState,
    type VisibleSubgraphWorkspacePaneState,
  } from './graph-viewer/workspace';
  import {
    clearGraphViewerTestHooks as clearGraphViewerTestHookState,
    shouldAttachGraphViewerTestHooks,
  } from './graph-viewer/graph-viewer-test-hooks';
  import type {
    CellBoxEntry,
    LeaferAppLike,
    LeaferBox,
    ScrollableBox,
  } from './graph-viewer/model';
  import { editorLanguageFallback, type SupportedEditorLanguageId } from '../monaco/language-support';
  import type { EditorIO } from '../store/document-session-store';
  import { handleError } from '../utils/error-handler';
  import { GRAPH_CONFIG } from '../config/constants';
  import { resolveSemanticTypeColor } from '@treease/graph-viewer-runtime';
  import { type GraphCell, type GraphCellKind, type GraphNode } from '@treease/graph-viewer-runtime';
  import { buildPathKey } from '../graph/graph-viewer-path';
  import { isDocumentRevisionGuardCurrent } from '../guards/document-revision-guard';
  import { clearGraphSelectionForFullEdit } from './GraphViewer.graph-highlight';
  import {
    refreshUsageGate,
    runPostpaidCapability,
    type UsageBlock,
  } from '../billing/entitlement-gate';
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
  export let onEntitlementBlocked: (block: UsageBlock) => void = () => {};

  const MINIMAP_WIDTH = 220;
  const MINIMAP_HEIGHT = 150;
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
  const graphReadinessWaiters = new Set<{
    resolve: (ready: boolean) => void;
    timeout: ReturnType<typeof setTimeout>;
  }>();
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
  const graphRenderState = createGraphRenderState();
  const graphRenderBindings = createGraphRenderBindings(graphRenderState);
  const {
    upsertCellEntry,
    updateCellEntry,
    registerCellBox,
    unregisterCellBox,
    registerRowBox,
    unregisterRowBox,
  } = graphRenderBindings;
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
  let subgraphWorkspaceVisiblePanes: VisibleSubgraphWorkspacePaneState[] = [];
  let subgraphWorkspaceOpen = false;
  let subgraphWorkspaceActivePath: PathSeg[] = [];
  let subgraphWorkspaceCanGoBack = false;
  let subgraphWorkspaceCanGoForward = false;
  let subgraphWorkspaceHeightPx = SUBGRAPH_WORKSPACE_DEFAULT_HEIGHT;
  let isDraggingSubgraphWorkspaceDivider = false;
  let subgraphWorkspaceRoot: HTMLDivElement;
  let subgraphWorkspaceRail: HTMLDivElement;
  let subgraphWorkspaceDetailPane: VisibleSubgraphWorkspacePaneState | null = null;
  let lastSubgraphScrollKey = '';

  let graphSceneController: ReturnType<typeof createGraphSceneController>;
  let graphRuntimeProbeController: ReturnType<typeof createGraphRuntimeProbeController>;
  let graphRuntimeProbeActions: ReturnType<typeof createGraphRuntimeProbeActions>;
  let fullEditSettledCleanupHandle: number | null = null;
  let fullEditIdleCleanupHandle: number | null = null;
  const graphTreeStateController = createGraphTreeStateController({
    getToken: () => treeStateToken,
    setToken: (token) => {
      treeStateToken = token;
    },
    setTreeState: treeState.set,
  });
  let renderConfig: GraphViewerConfig = $settings.viewer.graphViewer;
  $: renderConfig = {
    ...$settings.viewer.graphViewer,
    colors: {
      ...$settings.viewer.graphViewer.colors,
      // Graph cells and Monaco always consume the same user-configured semantic palette.
      semanticType: $settings.editor.semanticTypeColors,
    },
  };
  $: if (subgraphWorkspaceOpen) {
    subgraphWorkspaceController.syncHeightToShell();
  }
  $: {
    subgraphWorkspaceDetailPane = subgraphWorkspaceVisiblePanes.find((pane) => pane.kind === 'content') ?? null;
  }
  $: {
    const nextScrollKey = `${workspacePathKey(subgraphWorkspaceActivePath)}|${subgraphWorkspaceVisiblePanes.length}`;
    if (subgraphWorkspaceOpen && nextScrollKey !== lastSubgraphScrollKey) {
      lastSubgraphScrollKey = nextScrollKey;
      void scrollSubgraphSelectionIntoView();
    }
  }
  const fullBuildReasonSet = new Set([
    'import-file',
    'drop-file',
    'language-switch',
    'whole-document-replacement',
  ]);
  let lastSubgraphWorkspaceResetSessionId = '';
  const measureTextSample = GRAPH_CONFIG.measureTextSample;
  let measureRoot: HTMLDivElement;
  let measureRow: HTMLDivElement;
  let measureRowText: HTMLSpanElement;
  let measureHeader: HTMLDivElement;
  let measureHeaderText: HTMLSpanElement;
  let resetActiveEditState: () => void = () => {};
  const requestFrame =
    typeof globalThis.requestAnimationFrame === 'function'
      ? globalThis.requestAnimationFrame.bind(globalThis)
      : () => 0;
  const cancelFrame =
    typeof globalThis.cancelAnimationFrame === 'function'
      ? globalThis.cancelAnimationFrame.bind(globalThis)
      : () => {};
  const graphFullEditRuntime = createGraphFullEditRuntime({
    getFullEditUiState: () => $fullEditUiState,
    getLeafer: () => leafer as LeaferAppLike | null,
    getGraphData: () => graphSceneController.getLastGraphData(),
    replaceAll: (graphData) => graphSceneController.replaceAll(graphData),
    updateMinimap: () => graphMinimapRuntimeController.update(),
    resetActiveEditState,
    isInteractionBlocked: () => isFullEditInteractionBlocked(),
    requestFrame,
    completeStreamProgress: () => graphStreamProgressController.completeIfActive(),
    getCleanupHandles: () => ({
      settled: fullEditSettledCleanupHandle,
      idle: fullEditIdleCleanupHandle,
    }),
    setCleanupHandles: (handles) => {
      fullEditSettledCleanupHandle = handles.settled;
      fullEditIdleCleanupHandle = handles.idle;
    },
  });
  let measureSignature = '';

  type GraphSearchTarget = 'node' | 'key' | 'value';

  type GraphSearchResult = {
    nodeId: number | undefined;
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
      leafer as (
        LeaferAppLike & { zoomLayer: LeaferZoomLayer | undefined; getValidScale: ((scale: number) => number) | undefined }
      ) | null,
    getSuppressGraphPointerUntil: () => suppressGraphPointerUntil,
    getMoveEventName: () => (MoveEventCtor?.BEFORE_MOVE ?? MoveEventCtor?.MOVE) as string | undefined,
    getZoomEventName: () => (ZoomEventCtor?.BEFORE_ZOOM ?? ZoomEventCtor?.ZOOM) as string | undefined,
    bindPointerClick: (target, handler) => graphPointerController.bindPointerClick(target, handler),
    updateRenderableProjection: () => graphSceneController?.updateRenderableProjection(),
    updateViewportOverlays: () => {
      graphMinimapRuntimeController.updateViewport();
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

  const centerOnBox = (box: LeaferBox) => graphViewportController.centerOnBox(box);
  const centerOnNode = (node: GraphNode) => graphViewportController.centerOnNode(node);
  const updateSize = () => graphViewportController.updateSize();
  function requestLeaferRender(): void {
    const renderedLeafer = leafer;
    const target = renderedLeafer as unknown as (
      { update: (() => void) | undefined; forceRender: (() => void) | undefined; updateClientBounds: (() => void) | undefined } & object
    ) | null;
    target?.update?.();
    target?.forceRender?.();
    // Update client bounds for hit-testing (needed for pointer events to find correct elements)
    target?.updateClientBounds?.();
    if (typeof requestAnimationFrame === 'function') {
      requestAnimationFrame(() => {
        if (renderedLeafer === leafer) target?.forceRender?.();
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

  graphRuntimeProbeController = createGraphRuntimeProbeController({
    shouldAttachGraphViewerTestHooks,
    isTextClickTarget,
    isFullEditStreaming: () => isFullEditInteractionBlocked(),
    bindPointerClick: (target, handler) => graphPointerController.bindPointerClick(target, handler),
    getContainerRect: () => container?.getBoundingClientRect() ?? null,
    getRootClickTargets: () => graphRuntimeProbeActions.listClickTargetProbes(),
    getRootApp: () => leafer as LeaferAppLike | null,
    getLanguageId: () => languageIdValue,
    getCellBoxByPathMap: () => graphRenderState.getCellBoxByPathMap(),
    buildPathKey,
    getClientProbeCoordFromBox: (box, app) => getClientProbeCoordFromBoxLike(box, app),
    getClientRectFromBox: (box, app) => getClientRectFromBoxLike(box, app),
    getWorldRectFromBox: (box) => getWorldRectFromBoxLike(box),
    getClientPointFromWorld: (point) => {
      const worldLeafer = leafer as LeaferAppOrLeafer | null;
      if (!point || typeof worldLeafer?.getClientPointByWorld !== 'function') return null;
      const clientPoint = worldLeafer.getClientPointByWorld(point);
      const x = Number(clientPoint?.x);
      const y = Number(clientPoint?.y);
      if (!Number.isFinite(x) || !Number.isFinite(y)) return null;
      return { x, y };
    },
    getViewportWorldCenter: () => {
      const worldLeafer = leafer as LeaferAppOrLeafer & {
        updateClientBounds: (() => void) | undefined;
        clientBounds: { x: number; y: number; width: number; height: number } | undefined;
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
    resolveTreePathByPosition: (row, column) => resolveTreePathByPosition(row, column),
    resolveInteractiveCellPath,
    emitReveal: (path, target, source) => {
      if (source === 'runtime-query') {
        emitReveal(path, target, 'click');
        return;
      }
      emitReveal(path, target, source);
    },
    onRegisteredTargetClick: async ({ path, scope }) => {
      if (scope !== 'root') return;
      await openSubgraphWorkspacePath(path, -1);
    },
    commitProbe: async ({ cell, kind }, text) => {
      if (kind !== 'key' && kind !== 'value') return false;
      dispatchGraphEditEvent(container, 'graph-edit-open', {
        path: cell.path,
        kind,
        valueType: cell.valueType,
      });
      return applyGraphEdit(cell, kind, text, null);
    },
  });
  graphRuntimeProbeActions = createGraphRuntimeProbeActions({
    getController: () => graphRuntimeProbeController,
    getWorkspaceRoot: () => subgraphWorkspaceRoot ?? null,
  });

  const graphTextLinkageController = createGraphTextLinkageController({
    getDocumentKey: () => documentKeyValue,
    getSourceText: () => $sourceText ?? '',
    getLanguageId: () => languageIdValue,
    getActiveSnapshotId: () => graphRenderCoordinator.getActiveSnapshotId(),
    getEnableNest: () => $settings.parser.enableNest,
    getRenderConfig: () => renderConfig,
    getClickTargetProbes: () => graphRuntimeProbeActions.listClickTargetProbes(),
    setGraphHighlightTestState: graphRuntimeProbeActions.setGraphHighlightTestState,
    setGraphRevealTestState: graphRuntimeProbeActions.setGraphRevealTestState,
    setGraphRowScrollTestState: graphRuntimeProbeActions.setGraphRowScrollTestState,
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
    ...createGraphTextLinkageRenderDeps(graphRenderState, (anchor) =>
      graphSceneController.scrollTableToRow(anchor.nodeId, anchor.rowIndex),
    ),
    materializeTarget: (renderHandle) => graphSceneController.materializeTarget(renderHandle),
  });
  const clearSearchHighlight = graphTextLinkageController.clearSearchHighlight;
  const resolveTreePathByPosition = graphTextLinkageController.resolveTreePathByPosition;
  const ensurePathIndex = graphTextLinkageController.ensurePathIndex;
  const emitReveal = graphTextLinkageController.emitReveal;

  const graphEditAdapter = createWebGraphEditAdapter({
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
    nextTreeStateToken: () => graphTreeStateController.nextToken(),
    publishTreeState: (...args) => graphTreeStateController.publish(...args),
    emitEditorMutation,
    updateActiveTempModel: (updater) => activeTempModel.update(updater),
    dispatchGraphEditEvent: (type, detail) => dispatchGraphEditEvent(container, type, detail),
    runBidirectionalEdit: async (documentKey, execute) => runPostpaidCapability({
      capability: 'bidirectional_edit',
      idempotencyKey: documentKey,
      metadata: { surface: 'graph_cell' },
      surface: 'graph_edit',
      execute,
      onBlocked: onEntitlementBlocked,
    }),
    handleError,
  });
  const hasActiveEdit = graphEditAdapter.hasActiveEdit;
  const applyGraphEdit = graphEditAdapter.applyGraphEdit;
  const applyStructuredValueEdit = graphEditAdapter.applyStructuredValueEdit;
  const bindGraphEditorLifecycle = graphEditAdapter.bindRuntimeEditor;
  resetActiveEditState = graphEditAdapter.resetActiveEditState;
  const onRuntimeReady = ({ editor }: { editor: unknown | null }) => bindGraphEditorLifecycle(editor);
  const subgraphWorkspaceController = createSubgraphWorkspaceController({
    defaultHeightPx: SUBGRAPH_WORKSPACE_DEFAULT_HEIGHT,
    getActiveSnapshotId: () => graphRenderCoordinator.getActiveSnapshotId(),
    getWorkspaceSnapshotId: () => getWorkspaceSnapshotId(documentKeyValue),
    getDocumentKey: () => documentKeyValue,
    getLanguageId: () => languageIdValue,
    getRevision: () => editorRevisionValue,
    getRenderConfig: () => renderConfig,
    getEnableNest: () => $settings.parser.enableNest,
    getReadonly: () => readonly,
    getShellHeight: () => graphViewerShellHeight,
    inferGraphPaths: (nodes, edges) => graphSceneController.inferGraphPaths(nodes, edges),
    clearSearchHighlight,
    clearActiveGraphSelection: () => {
      activeTempModel.update((current) => clearGraphSelectionForFullEdit(current));
    },
    emitReveal: (path, target) => emitReveal(path, target, 'click'),
    handleError,
    applyStructuredValueEdit,
    waitForCommittedDocument,
    markSubgraphRequested,
    markSubgraphMaterialized,
    onState: (state) => {
      subgraphWorkspaceOpen = state.open;
      subgraphWorkspaceActivePath = state.activePath;
      subgraphWorkspaceChain = state.chain;
      subgraphWorkspaceVisiblePanes = state.visiblePanes;
      subgraphWorkspaceCanGoBack = state.canGoBack;
      subgraphWorkspaceCanGoForward = state.canGoForward;
      subgraphWorkspaceHeightPx = state.heightPx;
      isDraggingSubgraphWorkspaceDivider = state.isDraggingDivider;
    },
    onPaneReady: syncSubgraphReadinessForPane,
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
    return isGraphViewRuntimeRenderCurrent(guard, {
      documentKey: documentKeyValue,
      revision: editorRevisionValue,
      jsonBlockSelection: $jsonBlockSelection,
    });
  }

  function getGraphInteractionState() {
    const sceneState = graphSceneController.getInteractionState();
    const current = isGraphRenderGuardCurrent(graphRenderGuard);
    const interactiveReady = current && sceneState.kind === 'scene-committed';
    return {
      ...graphRenderGuard,
      current,
      ...sceneState,
      interactiveReady,
    };
  }

  function resolveGraphReadinessWaiters(): void {
    const interaction = getGraphInteractionState();
    const ready = graphRuntimeReady && interaction.interactiveReady === true;
    const failed = Boolean(errorMessage);
    if (!ready && !failed) return;
    graphReadinessWaiters.forEach((waiter) => {
      clearTimeout(waiter.timeout);
      waiter.resolve(ready);
    });
    graphReadinessWaiters.clear();
  }

  export function waitForGraphReady(): Promise<boolean> {
    if (errorMessage) return Promise.resolve(false);
    if (graphRuntimeReady && getGraphInteractionState().interactiveReady === true) return Promise.resolve(true);
    return new Promise((resolve) => {
      const waiter = {
        resolve,
        timeout: setTimeout(() => {
          graphReadinessWaiters.delete(waiter);
          resolve(false);
        }, 10_000),
      };
      graphReadinessWaiters.add(waiter);
    });
  }

  function syncGraphReadinessFromInteraction() {
    const interaction = getGraphInteractionState();
    if (!interaction?.documentKey || typeof interaction.revision !== 'number') return interaction;
    syncGraphInteractionReadiness({
      documentKey: interaction.documentKey,
      revision: interaction.revision,
      mode: interaction.mode ?? 'committed',
      hasGraphData: interaction.graphReady,
      nodeCount: interaction.nodeCount ?? 0,
      pendingRenderWork: interaction.projectionPending,
      interactiveReady: interaction.interactiveReady === true,
    });
    return interaction;
  }

  function syncSubgraphReadinessForPane(pane: SubgraphWorkspacePaneState | null | undefined): void {
    if (!pane?.pathKey || !pane.requestId) return;
    syncSubgraphInteractionReadiness({
      requestId: pane.requestId,
      pathKey: pane.pathKey,
      sourceRevision: editorRevisionValue,
      interactiveRevision: editorRevisionValue,
      interactiveReady: pane.status === 'ready',
    });
  }

  function waitForCommittedDocument(documentKey: string, afterRevision: number): Promise<boolean> {
    return new Promise((resolve) => {
      let settled = false;
      let unsubscribe = () => {};
      unsubscribe = activeDocumentSemanticStateByKey.subscribe((states) => {
        const state = states[documentKey];
        if (!state || state.revision <= afterRevision) return;
        if (state.status === 'pendingWholeDocument' || state.status === 'pendingJsonBlockEligible') return;
        if (settled) return;
        settled = true;
        unsubscribe();
        resolve(true);
      });
      if (settled) unsubscribe();
    });
  }

  function getRuntimeReadiness() {
    const interaction = syncGraphReadinessFromInteraction();
    const base = readRuntimeReadiness();
    if (base.subgraph.pathKey) {
      syncSubgraphReadinessForPane(subgraphWorkspaceController.getChain().find((pane) => pane.pathKey === base.subgraph.pathKey));
    }
    const next = readRuntimeReadiness();
    return {
      ...next,
      graph: {
        ...next.graph,
        mode: interaction?.mode ?? next.graph.mode,
        pendingRenderWork: interaction?.projectionPending ?? next.graph.pendingRenderWork,
      },
    };
  }


  function isTextClickTarget(target: object): boolean {
    if (TextCtor && target instanceof TextCtor) return true;
    const tag = String(
      (target as { __tag: string | undefined; tag: string | undefined }).__tag ??
        (target as { tag: string | undefined }).tag ??
        '',
    );
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
    nextTreeToken: () => graphTreeStateController.nextToken(),
    publishTreeState: (...args) => graphTreeStateController.publish(...args),
    clearTreeState: (...args) => graphTreeStateController.clear(...args),
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
    onStreamFinalAnalysis: (documentKey, language, revision, _analysis) => {
      if (
        !isDocumentRevisionGuardCurrent(
          { documentKey, revision },
          { documentKey: documentKeyValue, revision: editorRevisionValue },
        )
      ) {
        return;
      }
      const requestId = graphTreeStateController.nextToken();
      graphTreeStateController.clear(requestId, 'graph', revision);
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
      if ($fullEditUiState?.active && !graphFullEditRuntime.isProgressActive()) {
        return;
      }
      graphStreamProgressController.handleEvent(event as any);
    },
    resetStreamProgress: () => {
      graphStreamProgressController.reset();
    },
    completeStreamProgress: () => {
      graphFullEditRuntime.completeStreamProgress();
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
    getClickTargetProbes: () => graphRuntimeProbeActions.listClickTargetProbes(),
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
    registerClickTarget: graphRuntimeProbeActions.registerClickTarget,
    bindVerticalScrollGesture: (target, handler) => graphPointerController.bindVerticalScrollGesture(target, handler),
    bindPointerDown: (target, handler) => graphPointerController.bindPointerDown(target, handler),
    getPointFromEvent: (hostApp, target, event, space) =>
      graphPointerController.getPointFromEvent(hostApp, target, event, space),
    refreshActiveHighlight: () => graphTextLinkageController.refreshActiveHighlight(),
    updateLeafer: requestLeaferRender,
    handleError,
    ...createGraphSceneRenderDeps(graphRenderState, () => graphRuntimeProbeController?.resetRootClickTargets()),
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
    requestViewport: (view) => graphViewportController.moveToWorldViewport(view),
  });
  const graphViewRuntimeLifecycle = createGraphViewRuntimeLifecycle({
    fullBuildReasons: fullBuildReasonSet,
    setLastAutoOffset: (value) => {
      lastAutoOffset = value;
    },
    isFullEditProgressActive,
    completeStreamProgress,
    attachFullEditSession: (state, snapshot) => graphRenderEffects.maybeAttachFullEditSession(state, snapshot),
    renderJsonBlock: (selection, hasRenderRuntime) => graphRenderEffects.maybeRenderJsonBlock(selection, hasRenderRuntime),
    renderIncremental: (snapshot) => graphRenderEffects.maybeRenderIncremental(snapshot),
    scheduleFullEditCleanup,
    updateMinimap: () => graphMinimapRuntimeController.update(),
  });
  let lastSyncedReadonly = readonly;

  $: if (readonly !== lastSyncedReadonly) {
    lastSyncedReadonly = readonly;
    graphFullEditRuntime.syncReadonlyEditability();
  }

  function resolveInteractiveCellPath(cell: GraphCell, fallbackPath: PathSeg[]): Promise<PathSeg[]> {
    return resolveInteractiveCellPathWithFallback(cell, fallbackPath, resolveTreePathByPosition);
  }

  function getRenderRoot(): Box | null {
    return getLeaferContentRoot(leafer as LeaferAppLike | null);
  }

  function isFullEditProgressActive(): boolean {
    return graphFullEditRuntime.isProgressActive();
  }

  function completeStreamProgress(): void {
    graphFullEditRuntime.completeStreamProgress();
  }

  function scheduleFullEditCleanup(kind: 'settled' | 'idle', task: () => void): void {
    graphFullEditRuntime.scheduleCleanup(kind, task);
  }

  function ensureCanvasHint(): void {
    canvasHintText = ensureCanvasHintOverlay(
      container,
      leafer as LeaferAppLike | null,
      overlayLayer,
      TextCtor,
      canvasHintText,
    );
  }

  function resetCanvasHint(): void {
    canvasHintText = clearCanvasHintOverlay(canvasHintText);
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

  /**
   * Register a highlightable cell.
   * @param cell Cell data
   * @param kind Cell type
   * @param box Cell container
   * @returns void
   */
  function getClientProbeCoord(box: LeaferBox): { x: number; y: number } | null {
    return buildClientProbeCoord(box, leafer as LeaferAppLike | null, container);
  }

  function resetSubgraphWorkspace(): void {
    subgraphWorkspaceController.reset();
  }

  async function openSubgraphWorkspacePath(path: PathSeg[], parentAbsoluteIndex: number): Promise<void> {
    await subgraphWorkspaceController.openPath(path, parentAbsoluteIndex);
  }

  async function commitSubgraphWorkspaceValueEdit(
    pane: SubgraphWorkspacePaneState,
    draft: string | undefined,
  ): Promise<void> {
    await subgraphWorkspaceController.commitValueEdit(pane, draft);
  }

  async function selectSubgraphWorkspacePath(path: PathSeg[]): Promise<void> {
    await subgraphWorkspaceController.selectPath(path);
  }

  async function scrollSubgraphSelectionIntoView(): Promise<void> {
    await tick();
    const selected = subgraphWorkspaceRoot?.querySelector<HTMLElement>('[data-subgraph-selected="true"]');
    selected?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    const terminalPane = subgraphWorkspaceRail?.lastElementChild as HTMLElement | null;
    terminalPane?.scrollIntoView({ block: 'nearest', inline: 'center' });
  }

  function pathSegmentLabel(path: PathSeg[]): string {
    if (!path.length) return '$';
    const segment = path.at(-1)!;
    return isPathSegIndex(segment) ? `[${segment.index}]` : pathSegKeyValue(segment);
  }

  function isSubgraphWorkspacePathAncestor(path: PathSeg[], activePath: PathSeg[]): boolean {
    if (!path.length || path.length >= activePath.length) return false;
    return workspacePathKey(activePath.slice(0, path.length)) === workspacePathKey(path);
  }

  function isWorkspaceTextEditorTarget(target: EventTarget | null): boolean {
    return target instanceof Element && Boolean(
      target.closest('.monaco-editor, textarea, input, [contenteditable="true"]'),
    );
  }

  function handleSubgraphWorkspaceKeydown(event: KeyboardEvent): void {
    if (isWorkspaceTextEditorTarget(event.target)) return;
    const historyDirection =
      event.altKey && event.key === 'ArrowLeft' ? -1 :
      event.altKey && event.key === 'ArrowRight' ? 1 :
      0;
    if (historyDirection) {
      event.preventDefault();
      void (historyDirection < 0
        ? subgraphWorkspaceController.goBack()
        : subgraphWorkspaceController.goForward());
      return;
    }
    if (event.key === 'ArrowUp' || event.key === 'ArrowDown') {
      event.preventDefault();
      void subgraphWorkspaceController.moveSibling(event.key === 'ArrowUp' ? -1 : 1);
      return;
    }
    if (event.key === 'ArrowRight') {
      event.preventDefault();
      void subgraphWorkspaceController.enterSelected();
      return;
    }
    if (event.key === 'ArrowLeft') {
      event.preventDefault();
      void subgraphWorkspaceController.navigateParent();
      return;
    }
    if (event.key === 'Escape') {
      event.preventDefault();
      subgraphWorkspaceRoot?.focus();
    }
  }

  function handleSubgraphWorkspaceDividerDragStart(clientY: number) {
    subgraphWorkspaceController.startDividerDrag(clientY);
  }

  function handleSubgraphWorkspaceDividerDragMove(clientY: number) {
    subgraphWorkspaceController.moveDividerDrag(clientY);
  }

  function handleSubgraphWorkspaceDividerDragEnd() {
    subgraphWorkspaceController.endDividerDrag();
  }

  export function revealSearchResult(result: GraphSearchResult): void {
    if (isFullEditInteractionBlocked()) return;
    graphTextLinkageController.revealSearchResult(result);
  }

  export function revealPath(
    path: PathSeg[],
    options: { target: 'key' | 'value' | 'node' | undefined; navigate: boolean | undefined },
  ): Promise<boolean> {
    if (isFullEditInteractionBlocked()) return Promise.resolve(false);
    return graphTextLinkageController.revealPath(path, options);
  }

  export function getSubgraphWorkspacePaths(): PathSeg[][] {
    return subgraphWorkspaceOpen
      ? [subgraphWorkspaceController.getActivePath().map((segment) => ({ ...segment }))]
      : [];
  }

  export async function restoreSubgraphWorkspacePaths(paths: PathSeg[][]): Promise<boolean> {
    subgraphWorkspaceController.reset();
    const activePath = paths.at(-1);
    if (!activePath) return true;
    try {
      await subgraphWorkspaceController.openPath(activePath);
      return true;
    } catch {
      return false;
    }
  }

  const graphViewerRuntimeApi = {
    getClickProbeTargets: (scope: 'root' | 'workspace' | undefined) =>
      scope === 'workspace'
        ? graphRuntimeProbeActions.getSubgraphWorkspaceProbeTargets()
        : graphRuntimeProbeActions.getRuntimeProbeTargets(scope ?? 'root'),
    getHighlightTarget: () => graphRuntimeProbeActions.getRuntimeHighlightTarget(),
    getLastReveal: () => graphRuntimeProbeActions.getLastReveal(),
    clearLastReveal: () => graphRuntimeProbeActions.clearLastReveal(),
    getRowScrollState: (path: PathSeg[] | null | undefined) =>
      graphRuntimeProbeActions.getRuntimeRowScrollState(path),
    getHitResult: (point: { x: number; y: number }) => graphRuntimeProbeActions.getRuntimeHitResult(point),
    getLastGraphData: () => graphSceneController.getLastGraphData(),
    getInteractionState: () => getGraphInteractionState(),
    getRuntimeReadiness: () => getRuntimeReadiness(),
    getStreamProgressState: () => streamProgressState,
    revealPath: (path: PathSeg[], options: { target: 'key' | 'value' | 'node' | undefined; navigate: boolean | undefined }) =>
      revealPath(path, options),
    activateProbe: (probeId: string) => graphRuntimeProbeActions.activateRuntimeProbe(probeId),
    commitProbe: (probeId: string, text: string) => graphRuntimeProbeActions.commitRuntimeProbe(probeId, text),
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
    await exportLeaferImage(leafer as object);
  }

  function setError(message: string) {
    errorMessage = message;
    graphSceneController.clear();
    ensureCanvasHint();
    clearSearchHighlight();
  }

  function resetAppliedGraphHighlightState(options: { clearHighlight?: boolean } = {}): void {
    if (options?.clearHighlight) {
      clearSearchHighlight();
    }
    lastAppliedGraphHighlightSignature = '';
    lastAppliedGraphHighlightRevision = -1;
  }

  export function zoomIn() {
    applyZoom(1.1);
  }

  export function zoomOut() {
    applyZoom(1 / 1.1);
  }

  export function showEntitlementOverlay(block: UsageBlock): void {
    onEntitlementBlocked(block);
  }

  onMount(() => {
    void refreshUsageGate();
  });

  onDestroy(() => {
    graphReadinessWaiters.forEach((waiter) => {
      clearTimeout(waiter.timeout);
      waiter.resolve(false);
    });
    graphReadinessWaiters.clear();
    disposeGraphViewRuntime({
      cleanupHandles: { settled: fullEditSettledCleanupHandle, idle: fullEditIdleCleanupHandle },
      cancelFrame,
      resetCanvasHint,
      disposeMeasurement: () => graphMeasurementController.dispose(),
      disposeRenderEffects: () => graphRenderEffects.dispose(),
      disposeRenderCoordinator: () => graphRenderCoordinator.dispose(),
      disposeScene: () => graphSceneController.dispose(),
      resetActiveEditState,
      disposeSubgraphWorkspace: () => subgraphWorkspaceController.dispose(),
      unsubscribeStreamProgress: unsubscribeGraphStreamProgress,
      disposeStreamProgress: () => graphStreamProgressController.dispose(),
      resetLifecycle: graphViewRuntimeLifecycle.reset,
      clearGraphBridge,
      resetGraphStreamState,
    });
  });

  $: graphViewRuntimeLifecycle.syncRender({
    fullEditUiState: $fullEditUiState,
    jsonBlockSelection: $jsonBlockSelection,
    renderRuntimeReady,
    documentKey: documentKeyValue,
    language: languageIdValue,
    sourceText: $sourceText,
    editorRevision: editorRevisionValue,
    graphAppliedRevision: $graphAppliedRevision,
    lastAutoOffset,
  });

  $: {
    graphRuntimeReady;
    errorMessage;
    renderRuntimeReady;
    editorRevisionValue;
    $graphAppliedRevision;
    resolveGraphReadinessWaiters();
  }

  $: {
    const shouldResetWorkspace = shouldResetSubgraphWorkspaceForFullEdit(
      $fullEditUiState,
      $graphAppliedRevision,
    );
    const sessionId = $fullEditUiState?.sessionId ?? '';
    if (!shouldResetWorkspace) {
      lastSubgraphWorkspaceResetSessionId = '';
    } else if (sessionId && sessionId !== lastSubgraphWorkspaceResetSessionId) {
      lastSubgraphWorkspaceResetSessionId = sessionId;
      resetSubgraphWorkspace();
    }
  }

  $: if ($settings) {
    void subgraphWorkspaceController.syncProjection({
      documentKey: documentKeyValue,
      languageId: languageIdValue,
      revision: editorRevisionValue,
      graphAppliedRevision: $graphAppliedRevision,
      snapshotId: getWorkspaceSnapshotId(documentKeyValue),
      enableNest: $settings.parser.enableNest,
      renderConfig,
    });
  }
  $: graphViewRuntimeLifecycle.settle($fullEditUiState);

  $: {
    const graphHighlight = $activeTempModel?.graphHighlight ?? null;
    const graphHighlightSignature = buildGraphHighlightSignature(graphHighlight, buildPathKey);
    const appliedRevision = $graphAppliedRevision;
    if (isFullEditInteractionBlocked()) {
      resetAppliedGraphHighlightState({ clearHighlight: true });
    } else if (!graphHighlightSignature) {
      resetAppliedGraphHighlightState({ clearHighlight: true });
    } else if (!enableRevealSync) {
      resetAppliedGraphHighlightState();
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
  class:graph-viewer-shell--with-workspace={subgraphWorkspaceOpen}
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
        {onRuntimeReady}
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

  {#if subgraphWorkspaceOpen}
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
      bind:this={subgraphWorkspaceRoot}
      class="graph-subgraph-workspace"
      data-testid="graph-subgraph-workspace"
      style:height={`${subgraphWorkspaceHeightPx}px`}
      tabindex="0"
      role="tree"
      aria-label="Subgraph column browser"
      on:keydown={handleSubgraphWorkspaceKeydown}
      transition:fly={{ y: 18, duration: 180, opacity: 0.14, easing: cubicOut }}
    >
      <nav class="graph-subgraph-workspace__pathbar" aria-label="Subgraph path">
        <div class="graph-subgraph-workspace__history">
          <button
            type="button"
            aria-label="Back in workspace history"
            disabled={!subgraphWorkspaceCanGoBack}
            on:click={() => void subgraphWorkspaceController.goBack()}
          ><ChevronLeft size={15} strokeWidth={2} /></button>
          <button
            type="button"
            aria-label="Forward in workspace history"
            disabled={!subgraphWorkspaceCanGoForward}
            on:click={() => void subgraphWorkspaceController.goForward()}
          ><ChevronRight size={15} strokeWidth={2} /></button>
        </div>
        <div class="graph-subgraph-workspace__breadcrumbs">
          {#each buildWorkspacePathPrefixes(subgraphWorkspaceActivePath) as prefix (workspacePathKey(prefix))}
            <button
              type="button"
              class:active={workspacePathKey(prefix) === workspacePathKey(subgraphWorkspaceActivePath)}
              title={buildReadablePath(prefix)}
              on:click={() => void selectSubgraphWorkspacePath(prefix)}
            >{pathSegmentLabel(prefix)}</button>
            {#if workspacePathKey(prefix) !== workspacePathKey(subgraphWorkspaceActivePath)}
              <ChevronRight size={12} strokeWidth={1.8} aria-hidden="true" />
            {/if}
          {/each}
        </div>
      </nav>
      <div bind:this={subgraphWorkspaceRail} class="graph-subgraph-workspace__track">
        {#each subgraphWorkspaceVisiblePanes as pane (`${pane.kind}:${pane.pathKey}`)}
          {#if pane.kind === 'column'}
            <section class="graph-subgraph-pane" data-testid="graph-subgraph-pane" data-column-path-key={pane.pathKey}>
              {#if pane.status === 'ready'}
                <div class="graph-subgraph-pane__items" role="list" aria-label={`${pane.title} children`}>
                  {#each pane.items as item (item.pathKey)}
                    <button
                      type="button"
                      class="graph-subgraph-item"
                      class:index={isPathSegIndex(item.path.at(-1)!)}
                      class:selected={item.pathKey === workspacePathKey(subgraphWorkspaceActivePath)}
                      class:path-ancestor={isSubgraphWorkspacePathAncestor(item.path, subgraphWorkspaceActivePath)}
                      data-subgraph-selected={item.pathKey === workspacePathKey(subgraphWorkspaceActivePath)}
                      data-subgraph-path-ancestor={isSubgraphWorkspacePathAncestor(item.path, subgraphWorkspaceActivePath)}
                      data-subgraph-item-path={JSON.stringify(item.path)}
                      data-subgraph-item-path-key={item.pathKey}
                      data-subgraph-item-preview={item.preview}
                      data-subgraph-item-value-type={item.valueType}
                      data-subgraph-item-index={isPathSegIndex(item.path.at(-1)!)}
                      aria-pressed={item.pathKey === workspacePathKey(subgraphWorkspaceActivePath)}
                      on:click={() => void selectSubgraphWorkspacePath(item.path)}
                    >
                      {#if isPathSegIndex(item.path.at(-1)!)}
                        <span
                          class="graph-subgraph-item__kind"
                          style:color={resolveSemanticTypeColor(renderConfig.colors.semanticType, item.semType)}
                        >
                          <span class="graph-subgraph-item__dot"></span>
                        </span>
                      {/if}
                      <span class="graph-subgraph-item__label">{item.label}</span>
                      <span
                        class="graph-subgraph-item__preview"
                        class:container-preview={item.valueType === 'object' || item.valueType === 'array'}
                        style:color={item.valueType === 'object' || item.valueType === 'array'
                          ? undefined
                          : resolveSemanticTypeColor(renderConfig.colors.semanticType, item.semType)}
                      >{item.preview}</span>
                      {#if item.isContainer}
                        <ChevronRight class="graph-subgraph-item__chevron" size={13} strokeWidth={1.8} />
                      {/if}
                    </button>
                  {/each}
                </div>
              {:else if pane.status === 'loading'}
                <div class="graph-subgraph-pane__placeholder">Reading path…</div>
              {:else if pane.status === 'error'}
                <div class="graph-subgraph-pane__placeholder graph-subgraph-pane__placeholder--error">
                  {pane.error ?? 'Column projection failed'}
                </div>
              {:else}
                <div class="graph-subgraph-pane__placeholder">No direct children</div>
              {/if}
            </section>
          {/if}
        {/each}
      </div>
      <button
        type="button"
        class="graph-subgraph-workspace__dismiss"
        aria-label="Close subgraph workspace"
        on:click={() => subgraphWorkspaceController.reset()}
      ><X size={15} strokeWidth={2} /></button>
      {#if subgraphWorkspaceDetailPane?.status === 'ready' && subgraphWorkspaceDetailPane.content}
        <section
          class="graph-subgraph-detail"
          data-testid="graph-subgraph-pane"
          data-content-path-key={subgraphWorkspaceDetailPane.pathKey}
        >
          <div class="graph-subgraph-pane__content" data-testid="graph-subgraph-content-pane">
            <div class="graph-subgraph-pane__content-editor" data-testid="graph-subgraph-monaco-pane">
              {#key subgraphWorkspaceDetailPane.content.tabId}
                <SidecarEditor
                  tabId={subgraphWorkspaceDetailPane.content.tabId}
                  tabName={subgraphWorkspaceDetailPane.content.tabName}
                  language={languageIdValue}
                  sourceText={subgraphWorkspaceDetailPane.content.sourceText}
                  projectedSemanticTokens={subgraphWorkspaceDetailPane.content.semanticTokens}
                  runtimeHookId={subgraphWorkspaceDetailPane.content.tabId}
                  containerTestId="graph-subgraph-monaco-editor"
                  attachToPane={false}
                  destroyOnUnmount={true}
                  hideLineNumbers={true}
                  onScroll={() => {}}
                  onContentChange={(text) => {
                    emitReveal(subgraphWorkspaceDetailPane!.path, 'value', 'click');
                    void commitSubgraphWorkspaceValueEdit(subgraphWorkspaceDetailPane!, text);
                  }}
                  onEditorBlur={(text) => void commitSubgraphWorkspaceValueEdit(subgraphWorkspaceDetailPane!, text)}
                />
              {/key}
            </div>
          </div>
        </section>
      {/if}
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
    position: relative;
    display: grid;
    grid-template-rows: 38px minmax(0, 1fr);
    min-height: 0;
    overflow: hidden;
    outline: none;
    border-top: 1px solid rgba(148, 163, 184, 0.28);
    background:
      linear-gradient(180deg, rgba(245, 248, 252, 0.98), rgba(238, 243, 248, 0.98)),
      radial-gradient(circle at 14% 0%, rgba(59, 130, 246, 0.08), transparent 32%);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.76),
      0 -12px 24px rgba(15, 23, 42, 0.08);
  }

  .graph-subgraph-workspace:focus-visible {
    box-shadow:
      inset 0 2px 0 rgba(59, 130, 246, 0.42),
      0 -12px 24px rgba(15, 23, 42, 0.08);
  }

  :global(.graph-subgraph-workspace__divider) {
    background: transparent;
  }

  .graph-subgraph-workspace__pathbar {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    align-items: center;
    gap: 8px;
    min-width: 0;
    padding: 0 8px;
    border-bottom: 1px solid rgba(203, 213, 225, 0.78);
    background: rgba(255, 255, 255, 0.76);
    backdrop-filter: blur(12px);
  }

  .graph-subgraph-workspace__history {
    display: flex;
    gap: 2px;
  }

  .graph-subgraph-workspace__history button,
  .graph-subgraph-workspace__dismiss {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: #64748b;
    cursor: pointer;
  }

  .graph-subgraph-workspace__dismiss {
    position: absolute;
    top: 8px;
    right: 8px;
    z-index: 6;
    background: rgba(255, 255, 255, 0.82);
    box-shadow: 0 2px 8px rgba(15, 23, 42, 0.08);
  }

  .graph-subgraph-workspace__history button:hover:not(:disabled),
  .graph-subgraph-workspace__dismiss:hover {
    color: #1e293b;
    background: rgba(226, 232, 240, 0.76);
  }

  .graph-subgraph-workspace__history button:disabled {
    color: #cbd5e1;
    cursor: default;
  }

  .graph-subgraph-workspace__breadcrumbs {
    display: flex;
    align-items: center;
    min-width: 0;
    overflow-x: auto;
    scrollbar-width: none;
  }

  .graph-subgraph-workspace__breadcrumbs::-webkit-scrollbar {
    display: none;
  }

  .graph-subgraph-workspace__breadcrumbs button {
    flex: 0 0 auto;
    max-width: 180px;
    overflow: hidden;
    padding: 4px 6px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: #64748b;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    font-size: 11px;
    line-height: 1.3;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: pointer;
  }

  .graph-subgraph-workspace__breadcrumbs button:hover,
  .graph-subgraph-workspace__breadcrumbs button.active {
    color: #1e3a5f;
    background: rgba(219, 234, 254, 0.72);
  }

  .graph-subgraph-workspace__breadcrumbs :global(svg) {
    flex: 0 0 auto;
    color: #a8b3c2;
  }

  .graph-subgraph-workspace__track {
    display: flex;
    height: 100%;
    min-height: 0;
    overflow-x: auto;
    overflow-y: hidden;
    overscroll-behavior-inline: contain;
    scrollbar-color: rgba(100, 116, 139, 0.4) transparent;
    padding-right: 440px;
  }

  .graph-subgraph-pane {
    display: grid;
    width: 288px;
    min-width: 288px;
    flex: 0 0 288px;
    min-height: 0;
    grid-template-rows: minmax(0, 1fr);
    border-right: 1px solid rgba(203, 213, 225, 0.82);
    background: rgba(250, 252, 255, 0.9);
  }

  .graph-subgraph-detail {
    position: absolute;
    z-index: 4;
    top: 0;
    right: 0;
    bottom: 0;
    display: grid;
    width: 440px;
    min-width: 440px;
    flex: 0 0 440px;
    min-height: 0;
    grid-template-rows: minmax(0, 1fr);
    border-left: 1px solid rgba(203, 213, 225, 0.92);
    background: rgba(248, 250, 252, 0.96);
  }

  .graph-subgraph-pane__header {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    overflow: hidden;
    min-height: 36px;
    padding: 0 10px;
    border-bottom: 1px solid rgba(226, 232, 240, 0.96);
    background: rgba(255, 255, 255, 0.72);
  }

  .graph-subgraph-pane__eyebrow {
    color: #94a3b8;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.11em;
    text-transform: uppercase;
  }

  .graph-subgraph-pane__label {
    min-width: 0;
    overflow: hidden;
    color: #334155;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    font-size: 11px;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .graph-subgraph-pane__count,
  .graph-subgraph-detail__type {
    color: #94a3b8;
    font-size: 10px;
    font-variant-numeric: tabular-nums;
  }

  .graph-subgraph-pane__items {
    min-height: 0;
    overflow-y: auto;
    padding: 5px 6px 10px;
    scrollbar-color: rgba(100, 116, 139, 0.36) transparent;
  }

  .graph-subgraph-item {
    display: grid;
    grid-template-columns: minmax(132px, 1.25fr) minmax(48px, 0.75fr) auto;
    align-items: center;
    gap: 6px;
    width: 100%;
    min-height: 31px;
    padding: 5px 7px;
    border: 1px solid transparent;
    border-radius: 7px;
    background: transparent;
    color: #334155;
    text-align: left;
    cursor: default;
    transition:
      border-color 110ms ease,
      background-color 110ms ease,
      box-shadow 110ms ease;
  }

  .graph-subgraph-item:hover {
    background: rgba(226, 232, 240, 0.62);
  }

  .graph-subgraph-item.selected {
    border-color: rgba(96, 165, 250, 0.38);
    background: linear-gradient(90deg, rgba(219, 234, 254, 0.92), rgba(239, 246, 255, 0.82));
    box-shadow: inset 2px 0 0 #3b82f6;
  }

  .graph-subgraph-item.path-ancestor:not(.selected) {
    border-color: rgba(148, 163, 184, 0.22);
    background: rgba(148, 163, 184, 0.15);
    box-shadow: inset 2px 0 0 rgba(100, 116, 139, 0.48);
  }

  .graph-subgraph-item.index {
    grid-template-columns: 18px minmax(108px, 1.15fr) minmax(48px, 0.75fr) auto;
  }

  .graph-subgraph-item.index .graph-subgraph-item__dot {
    background: #5b83c4;
    opacity: 1;
  }

  .graph-subgraph-item:focus-visible {
    outline: 2px solid rgba(59, 130, 246, 0.42);
    outline-offset: -1px;
  }

  .graph-subgraph-item__kind {
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .graph-subgraph-item__dot {
    width: 6px;
    height: 6px;
    border-radius: 999px;
    background: currentColor;
    opacity: 0.72;
  }

  .graph-subgraph-item__label,
  .graph-subgraph-item__preview {
    min-width: 0;
    overflow: hidden;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    font-size: 11px;
    line-height: 1.45;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .graph-subgraph-item__label {
    color: #334155;
    font-weight: 550;
  }

  .graph-subgraph-item__preview {
    text-align: right;
    opacity: 0.82;
  }

  .graph-subgraph-item__preview.container-preview {
    color: #6b7280;
    opacity: 1;
  }

  .graph-subgraph-item__chevron {
    color: #94a3b8;
  }

  .graph-subgraph-pane__content {
    display: grid;
    height: 100%;
    min-height: 0;
    background: linear-gradient(180deg, rgba(248, 250, 252, 0.96), rgba(241, 245, 249, 0.96));
  }

  .graph-subgraph-pane__content-editor {
    min-height: 0;
    padding: 40px 10px 10px;
  }

  .graph-subgraph-pane__content-editor :global([data-testid='graph-subgraph-monaco-editor']) {
    height: 100%;
    border: 1px solid #dbe3ef;
    border-radius: 9px;
    background: rgba(255, 255, 255, 0.95);
    overflow: hidden;
    box-shadow: none;
  }

  .graph-subgraph-pane__placeholder {
    display: grid;
    place-items: center;
    min-height: 0;
    padding: 16px;
    color: #64748b;
    font-size: 12px;
    text-align: center;
  }

  .graph-subgraph-pane__placeholder--error {
    color: #b91c1c;
  }

</style>
