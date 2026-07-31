<!-- Responsibility: own the Leafer lifecycle, controller assembly, cross-boundary orchestration, and DOM template. -->
<script lang="ts">
  import { onDestroy, onMount, tick, createEventDispatcher } from 'svelte';
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
    createColumnNavigatorController,
    shouldResetColumnNavigatorForFullEdit,
    workspacePathKey,
    type ColumnNavigatorPaneState,
    type VisibleColumnNavigatorPaneState,
  } from './graph-viewer/column-navigator/index';
  import type { ColumnNavigatorState } from './graph-viewer/column-navigator/types';
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
  const COLUMN_NAVIGATOR_DEFAULT_HEIGHT = 220;

  let graphViewerShell: HTMLDivElement;
  let graphViewerMain: HTMLDivElement;
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
  const dispatch = createEventDispatcher<{
    reveal: unknown;
    'runtime-state': RuntimeStateEventDetail;
    'column-navigator-state': ColumnNavigatorState;
  }>();
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
  let columnNavigatorChain: ColumnNavigatorPaneState[] = [];
  let columnNavigatorVisiblePanes: VisibleColumnNavigatorPaneState[] = [];
  let columnNavigatorOpen = false;
  let columnNavigatorLoading = false;
  let columnNavigatorInitialLoading = false;
  let columnNavigatorActivePath: PathSeg[] = [];
  let columnNavigatorCanGoBack = false;
  let columnNavigatorCanGoForward = false;
  let columnNavigatorHeightPx = COLUMN_NAVIGATOR_DEFAULT_HEIGHT;
  let restoredColumnNavigatorHeight = false;
  let isDraggingColumnNavigatorDivider = false;
  let columnNavigatorRoot: HTMLDivElement;
  let columnNavigatorRail: HTMLDivElement;
  let columnNavigatorDetailPane: VisibleColumnNavigatorPaneState | null = null;
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
  $: if (columnNavigatorOpen) {
    columnNavigatorController.syncHeightToShell();
  }
  $: if ($settings && !restoredColumnNavigatorHeight) {
    const savedColumnNavigatorHeight = settingsStore.getColumnNavigatorHeight();
    if (savedColumnNavigatorHeight !== null) {
      columnNavigatorController.setHeight(savedColumnNavigatorHeight);
      restoredColumnNavigatorHeight = true;
    }
  }
  $: {
    columnNavigatorDetailPane = columnNavigatorVisiblePanes.find((pane) => pane.kind === 'content') ?? null;
  }
  $: columnNavigatorInitialLoading =
    columnNavigatorLoading &&
    columnNavigatorVisiblePanes.length === 1 &&
    columnNavigatorVisiblePanes[0]?.status === 'loading';
  $: {
    const nextScrollKey = `${workspacePathKey(columnNavigatorActivePath)}|${columnNavigatorVisiblePanes.length}`;
    if (columnNavigatorOpen && nextScrollKey !== lastSubgraphScrollKey) {
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
  let lastColumnNavigatorResetSessionId = '';
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
      await openColumnNavigatorPath(path, -1);
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
    getWorkspaceRoot: () => columnNavigatorRoot ?? null,
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
  const clearRenderedSearchHighlights = graphTextLinkageController.clearRenderedSearchHighlights;
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
  const columnNavigatorController = createColumnNavigatorController({
    defaultHeightPx: COLUMN_NAVIGATOR_DEFAULT_HEIGHT,
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
    emitReveal: (path, target) => emitReveal(path, target, 'breadcrumb'),
    handleError,
    applyStructuredValueEdit,
    waitForCommittedDocument,
    markSubgraphRequested,
    markSubgraphMaterialized,
    onState: (state) => {
      columnNavigatorOpen = state.open;
      columnNavigatorLoading = state.isLoading;
      columnNavigatorActivePath = state.activePath;
      columnNavigatorChain = state.chain;
      columnNavigatorVisiblePanes = state.visiblePanes;
      columnNavigatorCanGoBack = state.canGoBack;
      columnNavigatorCanGoForward = state.canGoForward;
      columnNavigatorHeightPx = state.heightPx;
      isDraggingColumnNavigatorDivider = state.isDraggingDivider;
      dispatch('column-navigator-state', state);
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

  function syncSubgraphReadinessForPane(pane: ColumnNavigatorPaneState | null | undefined): void {
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
      syncSubgraphReadinessForPane(columnNavigatorController.getChain().find((pane) => pane.pathKey === base.subgraph.pathKey));
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
    clearRenderedSearchHighlights,
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

  function resetColumnNavigator(): void {
    columnNavigatorController.reset();
  }

  async function openColumnNavigatorPath(path: PathSeg[], parentAbsoluteIndex: number): Promise<void> {
    await columnNavigatorController.openPath(path, parentAbsoluteIndex);
  }

  async function commitColumnNavigatorValueEdit(
    pane: ColumnNavigatorPaneState,
    draft: string | undefined,
  ): Promise<void> {
    await columnNavigatorController.commitValueEdit(pane, draft);
  }

  async function selectColumnNavigatorPathInternal(path: PathSeg[]): Promise<void> {
    await columnNavigatorController.selectPath(path);
    // Selecting a leaf may mount the Monaco detail editor. Keep keyboard
    // navigation owned by the column navigator instead of transferring focus
    // to the newly mounted editor.
    await tick();
    columnNavigatorRoot?.focus();
  }

  async function scrollSubgraphSelectionIntoView(): Promise<void> {
    await tick();
    const selected = columnNavigatorRoot?.querySelector<HTMLElement>('[data-column-navigator-selected="true"]');
    selected?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    const terminalPane = columnNavigatorRail?.lastElementChild as HTMLElement | null;
    terminalPane?.scrollIntoView({ block: 'nearest', inline: 'center' });
  }

  function pathSegmentLabel(path: PathSeg[]): string {
    if (!path.length) return '$';
    const segment = path.at(-1)!;
    return isPathSegIndex(segment) ? `[${segment.index}]` : pathSegKeyValue(segment);
  }

  function isColumnNavigatorPathAncestor(path: PathSeg[], activePath: PathSeg[]): boolean {
    if (!path.length || path.length >= activePath.length) return false;
    return workspacePathKey(activePath.slice(0, path.length)) === workspacePathKey(path);
  }

  function isWorkspaceTextEditorTarget(target: EventTarget | null): boolean {
    return target instanceof Element && Boolean(
      target.closest('.monaco-editor, textarea, input, [contenteditable="true"]'),
    );
  }

  function handleColumnNavigatorKeydown(event: KeyboardEvent): void {
    if (!columnNavigatorOpen || hasActiveEdit()) return;
    if (isWorkspaceTextEditorTarget(event.target)) return;
    const historyDirection =
      event.altKey && event.key === 'ArrowLeft' ? -1 :
      event.altKey && event.key === 'ArrowRight' ? 1 :
      0;
    if (historyDirection) {
      event.preventDefault();
      void (historyDirection < 0
        ? columnNavigatorController.goBack()
        : columnNavigatorController.goForward());
      return;
    }
    if (event.key === 'ArrowUp' || event.key === 'ArrowDown') {
      event.preventDefault();
      void columnNavigatorController.moveSibling(event.key === 'ArrowUp' ? -1 : 1);
      return;
    }
    if (event.key === 'ArrowRight') {
      event.preventDefault();
      void columnNavigatorController.enterSelected();
      return;
    }
    if (event.key === 'ArrowLeft') {
      event.preventDefault();
      void columnNavigatorController.navigateParent();
      return;
    }
    if (event.key === 'Escape') {
      event.preventDefault();
      columnNavigatorRoot?.focus();
    }
  }

  function handleGraphPointerDown(event: PointerEvent): void {
    if (event.target instanceof Element && event.target.closest('[data-testid="graph-viewer-minimap"]')) return;
    graphViewerMain?.focus();
  }

  function handleGraphKeydown(event: KeyboardEvent): void {
    if (!columnNavigatorOpen) return;
    handleColumnNavigatorKeydown(event);
  }

  function handleColumnNavigatorDividerDragStart(clientY: number) {
    columnNavigatorController.startDividerDrag(clientY);
  }

  function handleColumnNavigatorDividerDragMove(clientY: number) {
    columnNavigatorController.moveDividerDrag(clientY);
  }

  function handleColumnNavigatorDividerDragEnd() {
    columnNavigatorController.endDividerDrag();
    void settingsStore.saveColumnNavigatorHeight(columnNavigatorHeightPx);
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

  export function getColumnNavigatorActivePath(): PathSeg[] {
    return columnNavigatorOpen
      ? columnNavigatorController.getActivePath().map((segment) => ({ ...segment }))
      : [];
  }

  export async function restoreColumnNavigatorPath(activePath: PathSeg[]): Promise<boolean> {
    columnNavigatorController.reset();
    if (!activePath.length) return true;
    try {
      await columnNavigatorController.openPath(activePath);
      return true;
    } catch {
      return false;
    }
  }

  export async function goColumnNavigatorBack(): Promise<void> {
    if (isFullEditInteractionBlocked() || !columnNavigatorOpen) return;
    await columnNavigatorController.goBack();
  }

  export async function goColumnNavigatorForward(): Promise<void> {
    if (isFullEditInteractionBlocked() || !columnNavigatorOpen) return;
    await columnNavigatorController.goForward();
  }

  export async function selectColumnNavigatorPath(path: PathSeg[]): Promise<void> {
    if (isFullEditInteractionBlocked() || !columnNavigatorOpen) return;
    await selectColumnNavigatorPathInternal(path);
  }

  const graphViewerRuntimeApi = {
    getClickProbeTargets: (scope: 'root' | 'workspace' | undefined) =>
      scope === 'workspace'
        ? graphRuntimeProbeActions.getColumnNavigatorProbeTargets()
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
      disposeColumnNavigator: () => columnNavigatorController.dispose(),
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
    const shouldResetWorkspace = shouldResetColumnNavigatorForFullEdit(
      $fullEditUiState,
      $graphAppliedRevision,
    );
    const sessionId = $fullEditUiState?.sessionId ?? '';
    if (!shouldResetWorkspace) {
      lastColumnNavigatorResetSessionId = '';
    } else if (sessionId && sessionId !== lastColumnNavigatorResetSessionId) {
      lastColumnNavigatorResetSessionId = sessionId;
      resetColumnNavigator();
    }
  }

  $: if ($settings) {
    void columnNavigatorController.syncProjection({
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
      // A newly committed graph may not have registered its replacement boxes
      // when the old visual selection is cleared.  The linkage controller
      // restores that visual state from the retained logical path, then uses
      // the snapshot projection (not box presence) to decide whether it can
      // be discarded.
      void graphTextLinkageController.reconcileActiveHighlight({
        documentKey: documentKeyValue,
        snapshotId: graphRenderCoordinator.getActiveSnapshotId(),
        graphAppliedRevision: appliedRevision,
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
  class:graph-viewer-shell--with-workspace={columnNavigatorOpen}
  data-testid="graph-viewer-root"
>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    bind:this={graphViewerMain}
    class="graph-viewer-main"
    tabindex="0"
    role="application"
    aria-label="Graph"
    on:pointerdown={handleGraphPointerDown}
    on:keydown={handleGraphKeydown}
  >
    <!--
      Keep the graph decoration outside Leafer's scene. This node is created once
      with the graph surface and is never touched by node/edge projection updates.
    -->
    <div
      class="graph-background-grid"
      aria-hidden="true"
      data-testid="graph-background-grid"
    ></div>
    <div
      class="absolute inset-0 z-[1]"
      class:invisible={showRuntimeLoading}
      class:pointer-events-none={showRuntimeLoading}
    >
      <div bind:this={container} class="absolute inset-0 touch-none" data-testid="graph-viewer-canvas"></div>
      <div
        bind:this={minimapHost}
        class="pointer-events-auto absolute bottom-4 right-4 z-[2] h-[150px] w-[220px] overflow-hidden rounded-[14px]
          shadow-[0_12px_28px_rgba(15,23,42,0.14)] backdrop-blur transition-[bottom] duration-200 ease-out"
        style:bottom={`${columnNavigatorOpen ? columnNavigatorHeightPx + 16 : 16}px`}
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

  {#if columnNavigatorOpen}
    <div
      class="column-navigator-overlay"
      style={`--column-navigator-height: ${columnNavigatorHeightPx}px`}
    >
    <div
      class={`app-split-divider app-split-divider--horizontal column-navigator-graph__divider ${
        isDraggingColumnNavigatorDivider ? 'app-split-divider--dragging' : ''
      }`}
      role="separator"
      aria-label="Resize column navigator"
      aria-orientation="horizontal"
      use:splitLayoutDrag={{
        onDragStart: ({ clientY }) => handleColumnNavigatorDividerDragStart(clientY),
        onDragMove: ({ clientY }) => handleColumnNavigatorDividerDragMove(clientY),
        onDragEnd: () => handleColumnNavigatorDividerDragEnd(),
      }}
    >
    </div>
    <div
      bind:this={columnNavigatorRoot}
      class="column-navigator-graph"
      class:column-navigator-graph--loading={columnNavigatorLoading}
      data-testid="column-navigator-graph"
      style:height={`${columnNavigatorHeightPx}px`}
      style:font-family={renderConfig.fontFamily}
      tabindex="0"
      role="tree"
      aria-label="Column Navigator column browser"
      on:keydown={handleColumnNavigatorKeydown}
      aria-busy={columnNavigatorLoading}
    >
      {#if columnNavigatorInitialLoading}
        <div class="column-navigator-loading-skeleton" role="status" aria-live="polite" aria-label="Opening column navigator">
          <div class="column-navigator-loading-skeleton__column"></div>
          <div class="column-navigator-loading-skeleton__column"></div>
          <div class="column-navigator-loading-skeleton__detail"></div>
        </div>
      {:else}
      <div bind:this={columnNavigatorRail} class="column-navigator-graph__track">
        {#each columnNavigatorVisiblePanes as pane (`${pane.kind}:${pane.pathKey}`)}
          {#if pane.kind === 'column'}
            <section class="column-navigator-pane" data-testid="column-navigator-pane" data-column-navigator-path-key={pane.pathKey}>
              {#if pane.status === 'ready'}
                <div class="column-navigator-pane__items" role="list" aria-label={`${pane.title} children`}>
                  {#each pane.items as item (item.pathKey)}
                    <button
                      type="button"
                      class="column-navigator-item"
                      class:index={isPathSegIndex(item.path.at(-1)!)}
                      class:selected={item.pathKey === workspacePathKey(columnNavigatorActivePath)}
                      class:path-ancestor={isColumnNavigatorPathAncestor(item.path, columnNavigatorActivePath)}
                      data-column-navigator-selected={item.pathKey === workspacePathKey(columnNavigatorActivePath)}
                      data-column-navigator-path-ancestor={isColumnNavigatorPathAncestor(item.path, columnNavigatorActivePath)}
                      data-column-navigator-item-path={JSON.stringify(item.path)}
                      data-column-navigator-item-path-key={item.pathKey}
                      data-column-navigator-item-preview={item.preview}
                      data-column-navigator-item-value-type={item.valueType}
                      data-column-navigator-item-index={isPathSegIndex(item.path.at(-1)!)}
                      aria-pressed={item.pathKey === workspacePathKey(columnNavigatorActivePath)}
                      on:click={() => void selectColumnNavigatorPathInternal(item.path)}
                    >
                      {#if isPathSegIndex(item.path.at(-1)!)}
                        <span
                          class="column-navigator-item__kind"
                          style:color={resolveSemanticTypeColor(renderConfig.colors.semanticType, item.semType)}
                        >
                          <span class="column-navigator-item__dot"></span>
                        </span>
                      {/if}
                      <span class="column-navigator-item__label">{item.label}</span>
                      <span
                        class="column-navigator-item__preview"
                        class:container-preview={item.valueType === 'object' || item.valueType === 'array'}
                        style:color={item.valueType === 'object' || item.valueType === 'array'
                          ? undefined
                          : resolveSemanticTypeColor(renderConfig.colors.semanticType, item.semType)}
                      >{item.preview}</span>
                      {#if item.isContainer}
                        <span class="column-navigator-item__chevron-slot" aria-hidden="true">
                          <ChevronRight class="column-navigator-item__chevron" size={13} strokeWidth={1.8} />
                        </span>
                      {/if}
                    </button>
                  {/each}
                </div>
              {:else if pane.status === 'loading'}
                <div class="column-navigator-pane__placeholder">Reading path…</div>
              {:else if pane.status === 'error'}
                <div class="column-navigator-pane__placeholder column-navigator-pane__placeholder--error">
                  {pane.error ?? 'Column projection failed'}
                </div>
              {:else}
                <div class="column-navigator-pane__placeholder">No direct children</div>
              {/if}
            </section>
          {/if}
        {/each}
      </div>
      {/if}
      {#if columnNavigatorLoading && !columnNavigatorInitialLoading}
        <span class="column-navigator-loading-indicator" role="status" aria-live="polite">Updating path…</span>
      {/if}
      <button
        type="button"
        class="column-navigator-graph__dismiss"
        aria-label="Close column navigator"
        on:click={() => columnNavigatorController.reset()}
      ><X size={15} strokeWidth={2} /></button>
      {#if columnNavigatorDetailPane?.status === 'ready' && columnNavigatorDetailPane.content}
        <section
          class="column-navigator-detail"
          data-testid="column-navigator-pane"
          data-column-navigator-content-path-key={columnNavigatorDetailPane.pathKey}
        >
          <div class="column-navigator-pane__content" data-testid="column-navigator-content-pane">
            <div class="column-navigator-pane__content-editor" data-testid="column-navigator-monaco-pane">
              {#key columnNavigatorDetailPane.content.tabId}
                <SidecarEditor
                  tabId={columnNavigatorDetailPane.content.tabId}
                  tabName={columnNavigatorDetailPane.content.tabName}
                  language={languageIdValue}
                  sourceText={columnNavigatorDetailPane.content.sourceText}
                  projectedSemanticTokens={columnNavigatorDetailPane.content.semanticTokens}
                  projectedSnapshotId={columnNavigatorDetailPane.content.snapshotId}
                  projectedDocumentKey={documentKeyValue}
                  projectedRevision={editorRevisionValue}
                  runtimeHookId={columnNavigatorDetailPane.content.tabId}
                  containerTestId="column-navigator-monaco-editor"
                  attachToPane={false}
                  destroyOnUnmount={true}
                  hideLineNumbers={true}
                  compactGutter={true}
                  readOnly={readonly || columnNavigatorLoading}
                  onScroll={() => {}}
                  onContentChange={(text) => {
                    emitReveal(columnNavigatorDetailPane!.path, 'value', 'click');
                    void commitColumnNavigatorValueEdit(columnNavigatorDetailPane!, text);
                  }}
                  onEditorBlur={(text) => void commitColumnNavigatorValueEdit(columnNavigatorDetailPane!, text)}
                />
              {/key}
            </div>
          </div>
        </section>
      {/if}
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

  .graph-viewer-main {
    position: relative;
    min-height: 0;
    overflow: hidden;
    outline: none;
    isolation: isolate;
  }

  /*
    Static viewport decoration: no Leafer objects, no scene reconciliation, and
    no dependency on graph nodes/edges. Keeping it as a sibling beneath the
    interactive render surface prevents graph updates from rebuilding the grid.
  */
  .graph-background-grid {
    position: absolute;
    z-index: 0;
    inset: 0;
    pointer-events: none;
    contain: paint;
    background-color: #ffffff;
    background-image:
      linear-gradient(rgba(226, 232, 240, 0.72) 1px, transparent 1px),
      linear-gradient(90deg, rgba(226, 232, 240, 0.72) 1px, transparent 1px);
    background-size: 28px 28px;
  }

  .column-navigator-graph {
    position: absolute;
    right: 0;
    bottom: 0;
    left: 0;
    display: grid;
    grid-template-rows: minmax(0, 1fr);
    min-height: 0;
    overflow: hidden;
    outline: none;
    border-top: 1px solid rgba(148, 163, 184, 0.28);
    background: #eef3f8;
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.76),
      0 -12px 24px rgba(15, 23, 42, 0.08);
  }

  .column-navigator-graph:focus-visible {
    box-shadow:
      inset 0 2px 0 rgba(59, 130, 246, 0.42),
      0 -12px 24px rgba(15, 23, 42, 0.08);
  }

  .column-navigator-loading-indicator {
    position: absolute;
    z-index: 5;
    top: 6px;
    right: 30px;
    color: #64748b;
    font-size: 11px;
    opacity: 0;
    pointer-events: none;
    animation: column-navigator-loading-indicator-in 1ms linear 150ms forwards;
  }

  @keyframes column-navigator-loading-indicator-in {
    to { opacity: 1; }
  }

  :global(.column-navigator-graph__divider) {
    position: absolute;
    right: 0;
    bottom: var(--column-navigator-height);
    left: 0;
    background: transparent;
  }

  .column-navigator-overlay {
    position: absolute;
    right: 0;
    bottom: 0;
    left: 0;
    height: 100%;
    z-index: 20;
    pointer-events: none;
  }

  .column-navigator-overlay :global(.column-navigator-graph__divider),
  .column-navigator-overlay .column-navigator-graph {
    pointer-events: auto;
  }

  .column-navigator-graph__dismiss {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 28px;
    border: 0;
    border-radius: 0;
    background: transparent;
    color: #64748b;
    cursor: pointer;
  }

  .column-navigator-graph__dismiss {
    position: absolute;
    top: 0;
    right: 0;
    z-index: 6;
    border: 1px solid #cbd5e1;
    background: #e2e8f0;
    color: #475569;
    box-shadow: none;
  }

  .column-navigator-graph__dismiss:hover {
    color: #1e293b;
    background: #cbd5e1;
  }

  .column-navigator-graph__track {
    display: flex;
    height: 100%;
    min-height: 0;
    overflow-x: auto;
    overflow-y: hidden;
    overscroll-behavior-inline: contain;
    scrollbar-color: rgba(100, 116, 139, 0.4) transparent;
    padding-right: 440px;
  }

  .column-navigator-loading-skeleton {
    display: grid;
    grid-template-columns: 288px 288px minmax(0, 1fr);
    height: 100%;
    min-height: 0;
  }

  .column-navigator-loading-skeleton__column,
  .column-navigator-loading-skeleton__detail {
    position: relative;
    overflow: hidden;
    border-right: 1px solid rgba(203, 213, 225, 0.82);
    background: rgba(250, 252, 255, 0.9);
  }

  .column-navigator-loading-skeleton__detail {
    border-right: 0;
    background: rgba(248, 250, 252, 0.96);
  }

  .column-navigator-loading-skeleton__column::after,
  .column-navigator-loading-skeleton__detail::after {
    position: absolute;
    inset: 18px 14px;
    content: '';
    background: linear-gradient(90deg, #edf2f7 25%, #f8fafc 38%, #edf2f7 63%);
    background-size: 400% 100%;
    animation: column-navigator-skeleton-shimmer 1.4s ease-in-out infinite;
  }

  @keyframes column-navigator-skeleton-shimmer {
    from { background-position: 100% 0; }
    to { background-position: 0 0; }
  }

  .column-navigator-pane {
    display: grid;
    width: 288px;
    min-width: 288px;
    flex: 0 0 288px;
    min-height: 0;
    grid-template-rows: minmax(0, 1fr);
    border-right: 1px solid rgba(203, 213, 225, 0.82);
    background: rgba(250, 252, 255, 0.9);
  }

  .column-navigator-detail {
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

  .column-navigator-pane__header {
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

  .column-navigator-pane__eyebrow {
    color: #94a3b8;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.11em;
    text-transform: uppercase;
  }

  .column-navigator-pane__label {
    min-width: 0;
    overflow: hidden;
    color: #334155;
    font-size: 11px;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .column-navigator-pane__count,
  .column-navigator-detail__type {
    color: #94a3b8;
    font-size: 10px;
    font-variant-numeric: tabular-nums;
  }

  .column-navigator-pane__items {
    min-height: 0;
    overflow-y: auto;
    padding: 5px 6px 10px;
    scrollbar-color: rgba(100, 116, 139, 0.36) transparent;
  }

  .column-navigator-item {
    display: grid;
    grid-template-columns: fit-content(42%) minmax(48px, 1fr) auto;
    grid-template-rows: 16px;
    align-items: center;
    align-content: center;
    column-gap: 10px;
    row-gap: 6px;
    width: 100%;
    min-height: 31px;
    padding: 5px 7px;
    border: 1px solid transparent;
    border-radius: 0;
    background: transparent;
    color: #334155;
    font-family: inherit;
    text-align: left;
    cursor: default;
    transition:
      border-color 110ms ease,
      background-color 110ms ease,
      box-shadow 110ms ease;
  }

  .column-navigator-item:hover {
    background: rgba(226, 232, 240, 0.62);
  }

  .column-navigator-item.selected {
    border-color: #bfdbfe;
    background: #eaf2ff;
    box-shadow: none;
  }

  .column-navigator-item.path-ancestor:not(.selected) {
    border-color: rgba(148, 163, 184, 0.22);
    background: rgba(148, 163, 184, 0.15);
    box-shadow: none;
  }

  .column-navigator-item.index {
    grid-template-columns: 18px fit-content(38%) minmax(48px, 1fr) auto;
  }

  .column-navigator-item.index .column-navigator-item__dot {
    background: #5b83c4;
    opacity: 1;
  }

  .column-navigator-item:focus-visible {
    outline: 2px solid rgba(59, 130, 246, 0.42);
    outline-offset: -1px;
  }

  .column-navigator-item__kind {
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .column-navigator-item__dot {
    width: 6px;
    height: 6px;
    border-radius: 999px;
    background: currentColor;
    opacity: 0.72;
  }

  .column-navigator-item__label,
  .column-navigator-item__preview {
    min-width: 0;
    overflow: hidden;
    font-size: 12px;
    line-height: 16px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .column-navigator-item__label {
    color: #334155;
    font-weight: 550;
  }

  .column-navigator-item__preview {
    text-align: right;
    opacity: 0.82;
  }

  .column-navigator-item__preview.container-preview {
    color: #6b7280;
    opacity: 1;
  }

  .column-navigator-item__chevron {
    display: block;
    color: #94a3b8;
  }

  .column-navigator-item__chevron-slot {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 16px;
    width: 13px;
  }

  .column-navigator-pane__content {
    display: grid;
    height: 100%;
    min-height: 0;
    background: #f1f5f9;
  }

  .column-navigator-pane__content-editor {
    min-height: 0;
    padding: 8px;
  }

  .column-navigator-pane__content-editor :global([data-testid='column-navigator-monaco-editor']) {
    height: 100%;
    border: 0;
    border-radius: 0;
    background: #ffffff;
    overflow: hidden;
    box-shadow: none;
  }

  .column-navigator-pane__placeholder {
    display: grid;
    place-items: center;
    min-height: 0;
    padding: 16px;
    color: #64748b;
    font-size: 12px;
    text-align: center;
  }

  .column-navigator-pane__placeholder--error {
    color: #b91c1c;
  }

</style>
