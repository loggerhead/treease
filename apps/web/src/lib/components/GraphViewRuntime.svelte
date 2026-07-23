<!-- Responsibility: own the Leafer lifecycle, controller assembly, cross-boundary orchestration, and DOM template. -->
<script lang="ts">
  import { onDestroy, onMount, tick, createEventDispatcher } from 'svelte';
  import { cubicOut } from 'svelte/easing';
  import { fly } from 'svelte/transition';
  import { X } from 'lucide-svelte';
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
  import { buildReadablePath, type PathSeg } from '../store/tree-path';
  import { type MinimapViewData } from '../leafer-minimap';
  import { GraphRuntimeHost, GraphRuntimeLoading } from './graph-viewer/runtime';
  import GraphStreamProgressOverlay from './graph-viewer/GraphStreamProgressOverlay.svelte';
  import GraphEntitlementOverlay from './graph-viewer/GraphEntitlementOverlay.svelte';
  import SidecarEditor from './Editor/SidecarEditor.svelte';
  import { splitLayoutDrag } from './ui/split-layout';
  import {
    buildGraphHighlightSignature,
    createGraphFullEditRuntime,
    createGraphPointerController,
    createGraphStreamProgressController,
    createGraphTreeStateController,
    createGraphTextLinkageController,
    createGraphValueEditController,
    createGraphViewportController,
    shouldApplyGraphHighlight,
    type GraphStreamProgressState,
    type LeaferEventTarget,
    type LeaferZoomLayer,
  } from './graph-viewer/interaction';
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
    createSubgraphWorkspaceController,
    rebaseSubgraphWorkspacePath,
    shouldResetSubgraphWorkspaceForFullEdit,
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
  import { resolveGraphCellDisplayText } from '../graph/literal-display';
  import { type GraphCell, type GraphCellKind, type GraphNode } from '../graph/graph-viewer-render';
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
  let entitlementOverlay: UsageBlock | null = null;
  const graphReadinessWaiters = new Set<{
    resolve: (ready: boolean) => void;
    timeout: ReturnType<typeof setTimeout>;
  }>();
  let documentKeyValue = '';
  let lastEntitlementDocumentKey = '';
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
  let subgraphWorkspaceHeightPx = SUBGRAPH_WORKSPACE_DEFAULT_HEIGHT;
  let isDraggingSubgraphWorkspaceDivider = false;

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
  $: if (subgraphWorkspaceVisiblePanes.length) {
    subgraphWorkspaceController.syncHeightToShell();
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
    getVisiblePanes: () => subgraphWorkspaceVisiblePanes,
    getWorkspaceRuntime: (pathKey) => subgraphWorkspaceController.getRuntime(pathKey),
    getWorkspaceRect: () =>
      (document.querySelector("[data-testid='graph-subgraph-workspace']") as HTMLElement | null)?.getBoundingClientRect() ??
      null,
    rebaseWorkspacePath: rebaseSubgraphWorkspacePath,
    resolveCellText: (entry) =>
      resolveGraphCellDisplayText(
        entry.cell?.text,
        entry.cell?.value,
        String(entry.cell?.valueType ?? ''),
        languageIdValue,
      ),
    getLanguageId: () => languageIdValue,
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
      onBlocked: (block) => {
        entitlementOverlay = block;
      },
    }),
    handleError,
  });
  const hasActiveEdit = graphValueEditController.hasActiveEdit;
  const applyGraphEdit = graphValueEditController.applyGraphEdit;
  const bindGraphEditorLifecycle = graphValueEditController.bindGraphEditorLifecycle;
  resetActiveEditState = graphValueEditController.resetActiveEditState;
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
    getConstructors: () => ({ LeaferCtor, PlainLeaferCtor, BoxCtor, TextCtor, PenCtor }),
    inferGraphPaths: (nodes, edges) => graphSceneController.inferGraphPaths(nodes, edges),
    clearSearchHighlight,
    clearActiveGraphSelection: () => {
      activeTempModel.update((current) => clearGraphSelectionForFullEdit(current));
    },
    emitReveal: (path, target) => emitReveal(path, target, 'click'),
    handleError,
    applyGraphEdit,
    waitForCommittedDocument,
    markSubgraphRequested,
    markSubgraphMaterialized,
    bindGraphEditorLifecycle,
    bindPointerClick: (target, handler) => graphPointerController.bindPointerClick(target, handler),
    getMoveEventName: () => (MoveEventCtor?.BEFORE_MOVE ?? MoveEventCtor?.MOVE) as string | undefined,
    bindVerticalScrollGesture: (target, handler) => graphPointerController.bindVerticalScrollGesture(target, handler),
    bindPointerDown: (target, handler) => graphPointerController.bindPointerDown(target, handler),
    getPointFromEvent: (hostApp, target, event, space) =>
      graphPointerController.getPointFromEvent(hostApp, target, event, space),
    resolveInteractiveCellPath,
    onState: (state) => {
      subgraphWorkspaceChain = state.chain;
      subgraphWorkspaceVisiblePanes = state.visiblePanes;
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
    const interactiveReady =
      pane.kind === 'content'
        ? pane.status === 'ready'
        : pane.status === 'ready' && subgraphWorkspaceController.hasRuntime(pane.pathKey);
    syncSubgraphInteractionReadiness({
      requestId: pane.requestId,
      pathKey: pane.pathKey,
      sourceRevision: editorRevisionValue,
      interactiveRevision: editorRevisionValue,
      interactiveReady,
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

  function closeSubgraphWorkspacePane(absoluteIndex: number): void {
    subgraphWorkspaceController.closePane(absoluteIndex);
  }

  async function commitSubgraphWorkspaceValueEdit(
    pane: SubgraphWorkspacePaneState,
    draft: string | undefined,
  ): Promise<void> {
    await subgraphWorkspaceController.commitValueEdit(pane, draft);
  }

  const subgraphWorkspaceHostAction = subgraphWorkspaceController.hostAction;

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
    return subgraphWorkspaceController.getChain().map((pane) => pane.path.map((segment) => ({ ...segment })));
  }

  export async function restoreSubgraphWorkspacePaths(paths: PathSeg[][]): Promise<boolean> {
    subgraphWorkspaceController.reset();
    for (let index = 0; index < paths.length; index += 1) {
      try {
        await subgraphWorkspaceController.openPath(paths[index]!, index - 1);
      } catch {
        return false;
      }
    }
    return true;
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
    entitlementOverlay = block;
  }

  async function refreshEntitlementOverlay(): Promise<void> {
    const block = await refreshUsageGate(entitlementOverlay?.capability);
    entitlementOverlay = block;
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

  $: if (documentKeyValue !== lastEntitlementDocumentKey) {
    lastEntitlementDocumentKey = documentKeyValue;
    entitlementOverlay = null;
  }

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
    {#if entitlementOverlay}
      <GraphEntitlementOverlay block={entitlementOverlay} onRefresh={refreshEntitlementOverlay} />
    {/if}
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
                      rootSemType={pane.content.rootSemType}
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
