<!-- Responsibility: own the Leafer lifecycle, controller assembly, cross-boundary orchestration, and DOM template. -->
<script lang="ts">
  import { onDestroy, onMount, tick, createEventDispatcher } from 'svelte';
  import { ChevronRight, FileUp, Keyboard, MousePointer2 } from 'lucide-svelte';
  import {
    documentKey as documentKeyStore,
    editorIO,
    emitEditorMutation,
    editorRevision,
    graphAppliedRevision,
    languageId as languageIdStore,
    sourceText,
  } from '../store/document-session-store';
  import {
    activeTempModel,
    getActiveTempModelSnapshot,
    treeState,
    type TempModel,
  } from '../store/graph-selection-store';
  import { activeDocumentSemanticStateByKey } from '../store/active-document-authority';
  import { jsonBlockSelection } from '../store/full-edit-ui-store';
  import { activeFullEditUiState as fullEditUiState } from '../store/active-full-edit-ui-store';

  import { type GraphViewerConfig } from '../settings/ui-settings';
  import { settings, settingsStore } from '../settings/settings-store';
  import { shouldShowGraphRuntimeLoading, type RuntimeStateEventDetail } from '../runtime-loading';
  import { callSharedWasmWorker } from '../wasm/wasm-worker-singleton';
  import { getWorkspaceSnapshotId, getWorkspaceState } from '../store/workspace-store';
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
  import InteractionHint from './ui/InteractionHint.svelte';
  import {
    computePaneWidths,
    createSplitLayoutDragController,
    createSplitLayoutState,
    expandSplit,
    splitLayoutDrag,
    SplitLayoutCollapseHint,
    SplitLayoutCollapsedControl,
    type SplitLayoutConfig,
  } from './ui/split-layout';
  import { fileDropFeedback } from './ui/file-drop-feedback';
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
    createGraphMeasurementController,
    createGraphMinimapRuntimeController,
    createGraphRuntimeProbeActions,
    createGraphRuntimeProbeController,
    dispatchGraphEditEvent,
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
  import { editorLanguageFallback, importFormatOptions, type SupportedEditorLanguageId } from '../monaco/language-support';
  import { getLanguageExample } from '../monaco/language-examples';
  import type { EditorIO } from '../store/document-session-store';
  import { handleError } from '../utils/error-handler';
  import { GRAPH_CONFIG } from '../config/constants';
  import { resolveSemanticTypeColor } from '@treease/graph-viewer-runtime';
  import {
    type GraphCell,
    type GraphCellKind,
    type GraphNode,
    type GraphViewportState,
  } from '@treease/graph-viewer-runtime';
  import { buildPathKey } from '../graph/graph-viewer-path';
  import { isDocumentRevisionGuardCurrent } from '../guards/document-revision-guard';
  import { clearGraphSelectionForFullEdit } from './GraphViewer.graph-highlight';
  import {
    refreshUsageGate,
    runPostpaidCapability,
    type UsageBlock,
  } from '../billing/entitlement-gate';
  import { graphViewTopologyKey } from '../billing/graph-view-usage';
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
  import type { SharedWorkspaceMutationTarget } from '../share/share-workspace-lifecycle';

  type LeaferAppOrLeafer = LeaferApp | Leafer;
  export let active = true;
  export let synchronizedRuntimeLoading = false;
  export let readonly = false;
  export let onFileDrop: (event: globalThis.DragEvent) => void | Promise<void> = () => {};
  export let onRequestImportFile: (payload: { sourceFormat: string; targetFormat: string; accept: string[] }) => void | Promise<void> = () => {};
  export let onLoadExample: (example: string, language: SupportedEditorLanguageId) => void | Promise<void> = () => {};
  export let onEntitlementBlocked: (block: UsageBlock) => void = () => {};
  export let ensureSharedWorkspacePromoted: (target: SharedWorkspaceMutationTarget) => Promise<boolean> = async () => true;

  const MINIMAP_WIDTH = 220;
  const MINIMAP_HEIGHT = 150;
  const COLUMN_NAVIGATOR_DEFAULT_HEIGHT = 220;
  const COLUMN_NAVIGATOR_PANE_WIDTH_PX = 288;
  const COLUMN_NAVIGATOR_DETAIL_LAYOUT_CONFIG: SplitLayoutConfig = {
    defaultSplitRatio: 0.66,
    minPaneWidthPx: 200,
    dividerWidthPx: 10,
    collapsedControlInsetPx: 16,
    collapsedPaneWidthPx: 44,
  };
  const COLUMN_NAVIGATOR_HINT_STORAGE_KEY = 'treease:column-navigator-keyboard-hint-seen';
  const COLUMN_NAVIGATOR_HINT_MAX_PRESENTATIONS = 3;
  const COLUMN_NAVIGATOR_HINT_VISIBLE_MS = 4_500;
  const COLUMN_NAVIGATOR_HINT_FADE_MS = 180;
  const CANVAS_HINT_STORAGE_KEY = 'treease:canvas-drag-hint-seen';
  const CANVAS_HINT_MAX_PRESENTATIONS = 3;
  const CANVAS_HINT_VISIBLE_MS = 10_000;
  const CANVAS_HINT_FADE_MS = 180;
  const CANVAS_HINT_TOKENS = [
    'Hold ',
    { key: 'Space' },
    ' and drag to move the canvas.',
  ];
  const COLUMN_NAVIGATOR_HINT_TOKENS = [
    'Browse nodes with ',
    { key: '↑' },
    { key: '↓' },
    { key: '←' },
    { key: '→' },
    '. Use ',
    { key: '[' },
    ' and ',
    { key: ']' },
    ' to move through history.',
  ];

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
  let lastTopologyBytes: Uint8Array | null = null;
  const meteredTopologyKeys = new Set<string>();
  let graphRuntimeRetryToken = 0;
  let graphRuntimeReady = false;
  let showRuntimeLoading = true;
  let renderRuntimeReady = false;
  let isEmptyDocument = false;
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
  let canvasHintReady = false;
  let canvasHintGraphReady = false;
  let canvasHintPresentationCount = 0;
  let canvasHintTriggered = false;
  let canvasHintVisible = false;
  let canvasHintFading = false;
  let canvasHintTimer: ReturnType<typeof setTimeout> | null = null;
  let canvasHintFadeTimer: ReturnType<typeof setTimeout> | null = null;
  let suppressGraphPointerUntil = 0;
  let MoveEventCtor: typeof MoveEvent | undefined;
  let ZoomEventCtor: typeof ZoomEvent | undefined;
  let DragEventCtor: typeof DragEvent | undefined;
  let LeaferEventCtor: typeof LeaferEvent | undefined;
  let PointerEventCtor: typeof LeaferPointerEvent | undefined;
  const dispatch = createEventDispatcher<{
    navigation: unknown;
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
  $: isEmptyDocument = $sourceText.trim() === '';
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
  type ColumnNavigatorPaneMotion = {
    pane: VisibleColumnNavigatorPaneState;
    phase: 'entering' | 'entered' | 'exiting';
  };
  let renderedColumnNavigatorPanes: ColumnNavigatorPaneMotion[] = [];
  const columnNavigatorPaneExitTimers = new Map<string, ReturnType<typeof setTimeout>>();
  let columnNavigatorCollapsed = true;
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
  let columnNavigatorDetailContainerWidthPx = 0;
  let columnNavigatorDetailLayout = createSplitLayoutState(
    COLUMN_NAVIGATOR_DETAIL_LAYOUT_CONFIG.defaultSplitRatio,
  );
  let columnNavigatorDetailWidthPx = 0;
  let columnNavigatorTrailingSpacePx = 0;
  let columnNavigatorDetailDividerLeftPx = 0;
  let columnNavigatorDetailControlLeftPx = 0;
  let isDraggingColumnNavigatorDetailDivider = false;
  let columnNavigatorDetailCollapseHint: 'left' | 'right' | null = null;
  let hasColumnNavigatorDetail = false;
  let columnNavigatorDetailCollapsed = false;
  let columnNavigatorDetailPane: VisibleColumnNavigatorPaneState | null = null;
  let columnNavigatorHintReady = false;
  let columnNavigatorHintPresentationCount = 0;
  let columnNavigatorHintTriggered = false;
  let columnNavigatorHintVisible = false;
  let columnNavigatorHintFading = false;
  let columnNavigatorHintTimer: ReturnType<typeof setTimeout> | null = null;
  $: hasColumnNavigatorDetail = Boolean(
    columnNavigatorDetailPane?.status === 'ready' && columnNavigatorDetailPane.content,
  );
  $: columnNavigatorDetailCollapsed = columnNavigatorDetailLayout.layoutMode === 'left-only';
  $: {
    const widths = computePaneWidths(
      columnNavigatorDetailLayout,
      columnNavigatorDetailContainerWidthPx,
      COLUMN_NAVIGATOR_DETAIL_LAYOUT_CONFIG,
    );
    // Keep this rail's scroll range independent from divider movement. The trailing space lets the
    // terminal fixed-width column move entirely left of the detail overlay.
    columnNavigatorTrailingSpacePx = Math.max(
      columnNavigatorDetailContainerWidthPx - COLUMN_NAVIGATOR_PANE_WIDTH_PX,
      0,
    );
    columnNavigatorDetailWidthPx = widths.rightPaneWidthPx;
    columnNavigatorDetailDividerLeftPx = widths.splitterLeftPx;
    columnNavigatorDetailControlLeftPx = widths.splitterControlLeftPx;
  }
  const columnNavigatorDetailDragController = createSplitLayoutDragController(
    COLUMN_NAVIGATOR_DETAIL_LAYOUT_CONFIG,
  );
  let searchPreviewSnapshot: {
    tempModel: TempModel;
    viewport: GraphViewportState | null;
    columnNavigatorCollapsed: boolean;
    columnNavigatorPath: PathSeg[];
  } | null = null;
  let columnNavigatorHintFadeTimer: ReturnType<typeof setTimeout> | null = null;
  let lastSubgraphSelectionScrollKey = '';
  let lastSubgraphPaneScrollKey = '';

  function syncRenderedColumnNavigatorPanes(nextPanes: VisibleColumnNavigatorPaneState[]): void {
    const nextColumns = nextPanes.filter((pane) => pane.kind === 'column');
    const nextKeys = new Set(nextColumns.map((pane) => pane.pathKey));
    const currentByKey = new Map(renderedColumnNavigatorPanes.map((entry) => [entry.pane.pathKey, entry]));

    for (const [pathKey, timer] of columnNavigatorPaneExitTimers) {
      if (nextKeys.has(pathKey)) {
        clearTimeout(timer);
        columnNavigatorPaneExitTimers.delete(pathKey);
      }
    }

    const nextEntries = nextColumns.map((pane) => {
      const current = currentByKey.get(pane.pathKey);
      return {
        pane,
        phase: current?.phase === 'exiting' ? 'entering' : current ? 'entered' : 'entering',
      } satisfies ColumnNavigatorPaneMotion;
    });
    const exitingEntries = renderedColumnNavigatorPanes
      .filter((entry) => !nextKeys.has(entry.pane.pathKey))
      .map((entry) => ({ ...entry, phase: 'exiting' as const }));
    renderedColumnNavigatorPanes = [...nextEntries, ...exitingEntries];

    for (const entry of exitingEntries) {
      if (columnNavigatorPaneExitTimers.has(entry.pane.pathKey)) continue;
      const timer = setTimeout(() => {
        columnNavigatorPaneExitTimers.delete(entry.pane.pathKey);
        renderedColumnNavigatorPanes = renderedColumnNavigatorPanes.filter(
          (current) => current.pane.pathKey !== entry.pane.pathKey,
        );
      }, 180);
      columnNavigatorPaneExitTimers.set(entry.pane.pathKey, timer);
    }
  }

  function clearColumnNavigatorPaneExitTimers(): void {
    for (const timer of columnNavigatorPaneExitTimers.values()) clearTimeout(timer);
    columnNavigatorPaneExitTimers.clear();
  }

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
  $: columnNavigatorController.syncHeightToShell();
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
  $: if (
    columnNavigatorHintReady &&
    !columnNavigatorCollapsed &&
    !columnNavigatorLoading &&
    !columnNavigatorHintTriggered &&
    columnNavigatorHintPresentationCount < COLUMN_NAVIGATOR_HINT_MAX_PRESENTATIONS
  ) {
    showColumnNavigatorHint();
  }
  $: if (
    canvasHintReady &&
    canvasHintGraphReady &&
    !showRuntimeLoading &&
    !isEmptyDocument &&
    !canvasHintTriggered &&
    canvasHintPresentationCount < CANVAS_HINT_MAX_PRESENTATIONS
  ) {
    showCanvasHint();
  }
  $: {
    const nextSelectionScrollKey = workspacePathKey(columnNavigatorActivePath);
    if (!columnNavigatorCollapsed && nextSelectionScrollKey !== lastSubgraphSelectionScrollKey) {
      lastSubgraphSelectionScrollKey = nextSelectionScrollKey;
      void scrollSubgraphSelectionIntoView();
    }
  }
  $: {
    const nextPaneScrollKey = columnNavigatorVisiblePanes
      .filter((pane) => pane.kind === 'column')
      .map((pane) => pane.pathKey)
      .join('|');
    if (!columnNavigatorCollapsed && nextPaneScrollKey !== lastSubgraphPaneScrollKey) {
      lastSubgraphPaneScrollKey = nextPaneScrollKey;
      void scrollSubgraphPanesIntoView();
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
    publishNavigation: (path, target, source) => {
      if (source === 'runtime-query') {
        publishNavigation(path, target, 'click');
        return;
      }
      publishNavigation(path, target, source);
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
    dispatchReveal: (path, target, trigger) => dispatch('navigation', { path, target, trigger }),
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
  const publishNavigation = graphTextLinkageController.publishNavigation;

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
    beforeApplyMutation: ({ documentKey, model }) => {
      const workspace = getWorkspaceState();
      const tabId = workspace.activeTabId;
      return ensureSharedWorkspacePromoted({
        tabId,
        documentKey,
        readDocumentKey: () => getWorkspaceState().tabsById[tabId]?.documentKey ?? '',
        readText: () => model.getValue(),
        isCurrent: () => {
          const current = getWorkspaceState();
          return (
            !model.isDisposed() &&
            current.activeTabId === tabId &&
            current.tabsById[tabId]?.documentKey === documentKey &&
            editorIOValue?.context === 'editor' &&
            editorIOValue.getModel() === model
          );
        },
      });
    },
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
    getWorkspaceSnapshotId: () => getWorkspaceSnapshotId(documentKeyValue),
    getDocumentKey: () => documentKeyValue,
    getLanguageId: () => languageIdValue,
    getRevision: () => editorRevisionValue,
    getEnableNest: () => $settings.parser.enableNest,
    getReadonly: () => readonly,
    getShellHeight: () => graphViewerShellHeight,
    clearSearchHighlight,
    clearActiveGraphSelection: () => {
      activeTempModel.update((current) => clearGraphSelectionForFullEdit(current));
    },
    publishNavigation: (path, target) => publishNavigation(path, target, 'breadcrumb'),
    handleError,
    applyStructuredValueEdit,
    waitForCommittedDocument,
    markSubgraphRequested,
    markSubgraphMaterialized,
    onState: (state) => {
      columnNavigatorCollapsed = state.collapsed;
      columnNavigatorLoading = state.isLoading;
      columnNavigatorActivePath = state.activePath;
      columnNavigatorChain = state.chain;
      columnNavigatorVisiblePanes = state.visiblePanes;
      syncRenderedColumnNavigatorPanes(state.visiblePanes);
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

  /** Scene commits happen outside Svelte assignments, so publish from the render boundary. */
  function publishGraphRuntimeState(): void {
    const interactiveReady = graphRuntimeReady && getGraphInteractionState().interactiveReady === true;
    const runtimeStateSignature = `${graphRuntimeReady ? 'ready' : 'loading'}|${interactiveReady ? 'interactive' : 'pending'}|${errorMessage}`;
    if (runtimeStateSignature === lastRuntimeStateSignature) return;
    lastRuntimeStateSignature = runtimeStateSignature;
    dispatch('runtime-state', {
      ready: graphRuntimeReady,
      loading: !graphRuntimeReady && !errorMessage,
      error: Boolean(errorMessage),
      interactiveReady,
    });
  }

  function resolveGraphReadinessWaiters(signals: {
    graphRuntimeReady: boolean;
    errorMessage: string;
    renderRuntimeReady: boolean;
    editorRevision: number;
    graphAppliedRevision: number;
  }): void {
    const interaction = getGraphInteractionState();
    const ready = signals.graphRuntimeReady && interaction.interactiveReady === true;
    const failed = Boolean(signals.errorMessage);
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

  export function isGraphInteractive(): boolean {
    return graphRuntimeReady && getGraphInteractionState().interactiveReady === true;
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
    onStreamFinalAnalysis: (documentKey, language, revision, _analysis, snapshotId) => {
      if (
        !isDocumentRevisionGuardCurrent(
          { documentKey, revision },
          { documentKey: documentKeyValue, revision: editorRevisionValue },
        )
      ) {
        return;
      }
      if (snapshotId == null) {
        columnNavigatorController.reset();
        activeTempModel.update((current) => ({ ...current, treePath: [], graphHighlight: null }));
      }
      const requestId = graphTreeStateController.nextToken();
      graphTreeStateController.clear(requestId, 'graph', revision);
    },
    onTopologyRendered: async (topologyBytes) => {
      lastTopologyBytes = topologyBytes;
      await recordGraphView(topologyBytes);
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
      const message = error instanceof Error ? error.message : String(error);
      console.error('[GraphViewer] streaming render failed', error);
      setError(message);
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
    getCellBoxByPathMap: () => graphRenderState.getCellBoxByPathMap(),
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
    publishNavigation,
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
      canvasHintGraphReady = true;
      if (!isFullEditInteractionBlocked()) graphMinimapRuntimeController.update();
      publishGraphRuntimeState();
      return result;
    },
    flushPendingRenderWork: async () => {
      await graphSceneController.flushPendingRenderWork();
      publishGraphRuntimeState();
    },
    cancelActiveRenderWork: () => graphSceneController.cancelActiveRenderWork(),
    replaceRenderedGraph: (value) => {
      const result = graphSceneController.replaceAll(value);
      canvasHintGraphReady = true;
      if (!isFullEditInteractionBlocked()) graphMinimapRuntimeController.update();
      publishGraphRuntimeState();
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

  async function recordGraphView(topologyBytes: Uint8Array): Promise<void> {
    if (!active) return;
    const idempotencyKey = await graphViewTopologyKey(topologyBytes);
    if (meteredTopologyKeys.has(idempotencyKey)) return;
    meteredTopologyKeys.add(idempotencyKey);
    try {
      await runPostpaidCapability({
        capability: 'graph_view',
        idempotencyKey,
        metadata: { surface: 'graph_view', topologyVersion: 1 },
        surface: 'graph_view',
        execute: async () => undefined,
        onBlocked: onEntitlementBlocked,
      });
    } catch (error) {
      meteredTopologyKeys.delete(idempotencyKey);
      throw error;
    }
  }

  $: if (active && lastTopologyBytes) {
    void recordGraphView(lastTopologyBytes);
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

  function clearCanvasHintTimers(): void {
    if (canvasHintTimer !== null) {
      clearTimeout(canvasHintTimer);
      canvasHintTimer = null;
    }
    if (canvasHintFadeTimer !== null) {
      clearTimeout(canvasHintFadeTimer);
      canvasHintFadeTimer = null;
    }
  }

  function dismissCanvasHint(): void {
    if (!canvasHintVisible || canvasHintFading) return;
    clearCanvasHintTimers();
    canvasHintFading = true;
    canvasHintFadeTimer = setTimeout(() => {
      canvasHintVisible = false;
      canvasHintFading = false;
      canvasHintFadeTimer = null;
    }, CANVAS_HINT_FADE_MS);
  }

  function showCanvasHint(): void {
    canvasHintTriggered = true;
    canvasHintPresentationCount += 1;
    canvasHintVisible = true;
    try {
      localStorage.setItem(CANVAS_HINT_STORAGE_KEY, String(canvasHintPresentationCount));
    } catch {
      // Storage may be unavailable in private or restricted browser contexts.
    }
    canvasHintTimer = setTimeout(dismissCanvasHint, CANVAS_HINT_VISIBLE_MS);
  }

  function resetCanvasHint(): void {
    clearCanvasHintTimers();
    canvasHintVisible = false;
    canvasHintFading = false;
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

  function handleCollapseColumnNavigator(): void {
    columnNavigatorController.collapse();
  }

  function handleExpandColumnNavigator(): void {
    columnNavigatorController.expand();
  }

  function handlePinColumnNavigatorCollapsed(): void {
    columnNavigatorController.pinCollapsed();
  }

  async function commitColumnNavigatorValueEdit(
    pane: ColumnNavigatorPaneState,
    draft: string | undefined,
  ): Promise<void> {
    await columnNavigatorController.commitValueEdit(pane, draft);
  }

  function clearColumnNavigatorHintTimers(): void {
    if (columnNavigatorHintTimer !== null) {
      clearTimeout(columnNavigatorHintTimer);
      columnNavigatorHintTimer = null;
    }
    if (columnNavigatorHintFadeTimer !== null) {
      clearTimeout(columnNavigatorHintFadeTimer);
      columnNavigatorHintFadeTimer = null;
    }
  }

  function dismissColumnNavigatorHint(): void {
    if (!columnNavigatorHintVisible || columnNavigatorHintFading) return;
    clearColumnNavigatorHintTimers();
    columnNavigatorHintFading = true;
    columnNavigatorHintFadeTimer = setTimeout(() => {
      columnNavigatorHintVisible = false;
      columnNavigatorHintFading = false;
      columnNavigatorHintFadeTimer = null;
    }, COLUMN_NAVIGATOR_HINT_FADE_MS);
  }

  function showColumnNavigatorHint(): void {
    columnNavigatorHintTriggered = true;
    columnNavigatorHintPresentationCount += 1;
    columnNavigatorHintVisible = true;
    try {
      localStorage.setItem(COLUMN_NAVIGATOR_HINT_STORAGE_KEY, String(columnNavigatorHintPresentationCount));
    } catch {
      // Storage may be unavailable in private or restricted browser contexts.
    }
    columnNavigatorHintTimer = setTimeout(dismissColumnNavigatorHint, COLUMN_NAVIGATOR_HINT_VISIBLE_MS);
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
  }

  async function scrollSubgraphPanesIntoView(): Promise<void> {
    await tick();
    const rail = columnNavigatorRail;
    const terminalPane = columnNavigatorRail?.querySelector<HTMLElement>(
      ':scope > [data-testid="column-navigator-pane"]:last-of-type',
    );
    if (!rail || !terminalPane) return;

    // Keep the newest column beside the detail editor, but leave an already-visible
    // column untouched. Re-centering on every path change makes sibling navigation jitter.
    const detailLeft = hasColumnNavigatorDetail
      ? columnNavigatorDetailDividerLeftPx
      : rail.clientWidth;
    const terminalRight = terminalPane.offsetLeft + terminalPane.offsetWidth - rail.scrollLeft;
    if (terminalRight <= detailLeft) return;

    const maxScrollLeft = Math.max(0, rail.scrollWidth - rail.clientWidth);
    const desiredScrollLeft = Math.min(
      maxScrollLeft,
      Math.max(0, terminalPane.offsetLeft + terminalPane.offsetWidth - detailLeft),
    );
    if (Math.abs(rail.scrollLeft - desiredScrollLeft) > 0.5) {
      rail.scrollLeft = desiredScrollLeft;
    }
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
    if (hasActiveEdit()) return;
    if (isWorkspaceTextEditorTarget(event.target)) return;
    const historyDirection = event.key === '[' ? -1 : event.key === ']' ? 1 : 0;
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

  function handleColumnNavigatorDetailDragStart(clientX: number): void {
    if (!hasColumnNavigatorDetail || !columnNavigatorRoot) return;
    isDraggingColumnNavigatorDetailDivider = true;
    const update = columnNavigatorDetailDragController.start(
      columnNavigatorDetailLayout,
      clientX,
      columnNavigatorRoot.getBoundingClientRect(),
    );
    if (update) columnNavigatorDetailLayout = update.state;
    columnNavigatorDetailCollapseHint = update?.collapseSide ?? null;
  }

  function handleColumnNavigatorDetailDragMove(clientX: number): void {
    const update = columnNavigatorDetailDragController.move(columnNavigatorDetailLayout, clientX);
    if (update) columnNavigatorDetailLayout = update.state;
    columnNavigatorDetailCollapseHint = update?.collapseSide ?? null;
  }

  function handleColumnNavigatorDetailDragEnd(): void {
    columnNavigatorDetailDragController.end();
    isDraggingColumnNavigatorDetailDivider = false;
    columnNavigatorDetailCollapseHint = null;
  }

  function expandColumnNavigatorDetail(): void {
    if (columnNavigatorDetailLayout.layoutMode === 'split') return;
    columnNavigatorDetailLayout = expandSplit(
      columnNavigatorDetailLayout,
      columnNavigatorDetailContainerWidthPx,
      COLUMN_NAVIGATOR_DETAIL_LAYOUT_CONFIG,
    );
  }

  function beginSearchPreview(): void {
    if (searchPreviewSnapshot) return;
    searchPreviewSnapshot = {
      tempModel: getActiveTempModelSnapshot(),
      viewport: graphViewportController.getViewportState(),
      columnNavigatorCollapsed,
      columnNavigatorPath: columnNavigatorActivePath.map((segment) => ({ ...segment })),
    };
  }

  export function previewSearchResult(result: GraphSearchResult): void {
    if (isFullEditInteractionBlocked() || !result?.path?.length) return;
    beginSearchPreview();
    graphTextLinkageController.publishNavigation(result.path, result.target, 'search-preview');
  }

  export function commitSearchPreview(): void {
    searchPreviewSnapshot = null;
  }

  export async function cancelSearchPreview(): Promise<void> {
    const snapshot = searchPreviewSnapshot;
    if (!snapshot) return;
    searchPreviewSnapshot = null;
    graphTextLinkageController.cancelPendingReveal();
    graphTextLinkageController.clearSearchHighlight();
    columnNavigatorController.reset();
    if (snapshot.columnNavigatorPath.length) {
      await columnNavigatorController.openPath(snapshot.columnNavigatorPath);
      if (snapshot.columnNavigatorCollapsed) columnNavigatorController.collapse();
    } else if (!snapshot.columnNavigatorCollapsed) {
      columnNavigatorController.expand();
    }
    activeTempModel.set(snapshot.tempModel);
    await tick();
    await graphViewportController.waitForRevealTransition();
    if (snapshot.viewport) graphViewportController.restoreViewportState(snapshot.viewport);
  }

  export function revealSearchResult(result: GraphSearchResult): void {
    if (isFullEditInteractionBlocked() || !result?.path?.length) return;
    graphTextLinkageController.publishNavigation(result.path, result.target, 'search-commit', { navigate: true });
    commitSearchPreview();
  }

  export function revealPath(
    path: PathSeg[],
    options: { target: 'key' | 'value' | 'node' | undefined; navigate: boolean | undefined },
  ): Promise<boolean> {
    if (isFullEditInteractionBlocked()) return Promise.resolve(false);
    return graphTextLinkageController.revealPath(path, options);
  }

  export function getColumnNavigatorActivePath(): PathSeg[] {
    return columnNavigatorController.getActivePath().map((segment) => ({ ...segment }));
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

  export function collapseColumnNavigator(): void {
    handleCollapseColumnNavigator();
  }

  export function expandColumnNavigator(): void {
    handleExpandColumnNavigator();
  }

  export function pinColumnNavigatorCollapsed(): void {
    handlePinColumnNavigatorCollapsed();
  }

  export async function goColumnNavigatorBack(): Promise<void> {
    if (isFullEditInteractionBlocked()) return;
    await columnNavigatorController.goBack();
  }

  export async function goColumnNavigatorForward(): Promise<void> {
    if (isFullEditInteractionBlocked()) return;
    await columnNavigatorController.goForward();
  }

  export async function selectColumnNavigatorPath(path: PathSeg[]): Promise<void> {
    if (isFullEditInteractionBlocked()) return;
    await selectColumnNavigatorPathInternal(path);
  }

  export async function applyColumnNavigatorNavigationPath(path: PathSeg[]): Promise<void> {
    if (isFullEditInteractionBlocked()) return;
    await columnNavigatorController.applyExternalPath(path);
    await tick();
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
    getViewportState: () => graphViewportController.getViewportState(),
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
    canvasHintGraphReady = false;
    graphSceneController?.clear?.();
    clearSearchHighlight();
  }

  async function retryGraph(): Promise<void> {
    errorMessage = '';
    graphRuntimeReady = false;
    graphRuntimeRetryToken += 1;
    await tick();
    const ready = await waitForGraphReady();
    if (!ready || !documentKeyValue || !renderRuntimeReady) return;
    try {
      await graphRenderCoordinator.renderDocumentGraph({
        kind: 'incremental',
        documentKey: documentKeyValue,
        language: languageIdValue,
        text: $sourceText,
        revision: editorRevisionValue,
      });
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error));
      console.error('[graph] retry failed', error);
    }
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
    try {
      const storedPresentationCount = localStorage.getItem(COLUMN_NAVIGATOR_HINT_STORAGE_KEY);
      // Treat the previous boolean marker as one presentation so existing users get one final hint.
      columnNavigatorHintPresentationCount = storedPresentationCount === 'true'
        ? 1
        : Math.max(0, Number.parseInt(storedPresentationCount ?? '0', 10) || 0);
      canvasHintPresentationCount = Math.max(
        0,
        Number.parseInt(localStorage.getItem(CANVAS_HINT_STORAGE_KEY) ?? '0', 10) || 0,
      );
    } catch {
      // Storage may be unavailable in private or restricted browser contexts.
    }
    columnNavigatorHintReady = true;
    canvasHintReady = true;
    void refreshUsageGate().catch((error) => {
      console.error('[graph] usage gate initialization failed', error);
    });
  });

  onDestroy(() => {
    clearColumnNavigatorPaneExitTimers();
    clearColumnNavigatorHintTimers();
    clearCanvasHintTimers();
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

  $: resolveGraphReadinessWaiters({
    graphRuntimeReady,
    errorMessage,
    renderRuntimeReady,
    editorRevision: editorRevisionValue,
    graphAppliedRevision: $graphAppliedRevision,
  });

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
    });
  }
  $: graphViewRuntimeLifecycle.settle($fullEditUiState);

  $: {
    const graphHighlight = $activeTempModel?.graphHighlight ?? null;
    const graphHighlightSignature = buildGraphHighlightSignature(graphHighlight, buildPathKey);
    const appliedRevision = $graphAppliedRevision;
    if (isFullEditInteractionBlocked()) {
      graphSceneController.syncGraphHighlight(null);
      resetAppliedGraphHighlightState({ clearHighlight: true });
    } else if (!graphHighlightSignature) {
      graphSceneController.syncGraphHighlight(null);
      resetAppliedGraphHighlightState({ clearHighlight: true });
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
      graphSceneController.syncGraphHighlight(graphHighlight);
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

  $: publishGraphRuntimeState();

  function handleFileDrop(event: globalThis.DragEvent): void {
    event.preventDefault();
    event.stopPropagation();
    void onFileDrop(event);
  }

  function handleFileDragOver(event: globalThis.DragEvent): void {
    event.preventDefault();
    event.stopPropagation();
  }

  function getEmptyStateLanguage(): SupportedEditorLanguageId {
    return languageIdValue || editorLanguageFallback;
  }

  function handleEmptyStateOpenFile(): void {
    const language = getEmptyStateLanguage();
    const format = importFormatOptions.find((option) => option.id === language);
    void onRequestImportFile({
      sourceFormat: language,
      targetFormat: language,
      accept: format?.extensions ?? [],
    });
  }

  function handleEmptyStateLoadExample(): void {
    const language = getEmptyStateLanguage();
    const example = getLanguageExample(language);
    if (example) void onLoadExample(example, language);
  }
</script>

<div
  bind:this={graphViewerShell}
  bind:clientHeight={graphViewerShellHeight}
  class="graph-viewer-shell"
  class:graph-viewer-shell--with-workspace={!columnNavigatorCollapsed}
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
    data-testid="graph-viewer-dropzone"
    on:pointerdown={handleGraphPointerDown}
    on:keydown={handleGraphKeydown}
    on:drop={handleFileDrop}
    on:dragover={handleFileDragOver}
    use:fileDropFeedback
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
    <div class="file-drop-feedback-overlay" aria-hidden="true"></div>
    <div
      class="absolute inset-0 z-[1]"
      class:invisible={showRuntimeLoading}
      class:pointer-events-none={showRuntimeLoading}
    >
      <div bind:this={container} class="absolute inset-0 touch-none" data-testid="graph-viewer-canvas"></div>
      <div
        bind:this={minimapHost}
        class="pointer-events-auto absolute bottom-4 right-4 z-[2] h-[150px] w-[220px] overflow-hidden rounded-[9px]
          border border-[var(--border-strong)] bg-[var(--panel-bg)] shadow-[0_8px_20px_rgba(29,39,53,0.10)] transition-[bottom] duration-200 ease-out"
        style:bottom={`${!columnNavigatorCollapsed ? columnNavigatorHeightPx + 16 : 16}px`}
        class:hidden={streamProgressState.visible ||
          (isFullEditProgressActive() && $fullEditUiState?.phase !== 'settled')}
        data-testid="graph-viewer-minimap"
      ></div>
      {#key graphRuntimeRetryToken}
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
      {/key}
    </div>
    {#if showRuntimeLoading}
      <GraphRuntimeLoading />
    {:else if isEmptyDocument && !errorMessage}
      <div class="graph-empty-state" data-testid="graph-empty-state">
        <div class="graph-empty-state__body">
          <div class="graph-empty-state__mark" aria-hidden="true"><span></span><span></span><span></span></div>
          <span class="graph-empty-state__eyebrow">GRAPH VIEW</span>
          <h2>Turn your data into a graph</h2>
          <div class="graph-empty-state__tips" aria-label="Graph tips">
            <div class="graph-empty-state__tip">
              <span class="graph-empty-state__tip-icon" aria-hidden="true"><MousePointer2 size={13} strokeWidth={2} /></span>
              <span><strong>Canvas</strong> Hold <kbd>Space</kbd> and drag to move</span>
            </div>
            <div class="graph-empty-state__tip">
              <span class="graph-empty-state__tip-icon" aria-hidden="true"><Keyboard size={13} strokeWidth={2} /></span>
              <span><strong>Navigator</strong> Use <kbd>↑</kbd><kbd>↓</kbd><kbd>←</kbd><kbd>→</kbd> to browse · <kbd>[</kbd><kbd>]</kbd> history</span>
            </div>
            <div class="graph-empty-state__tip">
              <span class="graph-empty-state__tip-icon" aria-hidden="true"><FileUp size={13} strokeWidth={2} /></span>
              <span><strong>Quick start</strong> Drop a file here to open it.</span>
            </div>
          </div>
          <div class="graph-empty-state__actions">
            <button type="button" class="graph-empty-state__button graph-empty-state__button--primary" on:click={handleEmptyStateOpenFile}>
              Open file
            </button>
            <button type="button" class="graph-empty-state__button graph-empty-state__button--secondary" on:click={handleEmptyStateLoadExample}>
              Load example
            </button>
          </div>
        </div>
      </div>
    {/if}
    <GraphStreamProgressOverlay state={streamProgressState} />
    {#if canvasHintVisible}
      <div class="canvas-drag-hint">
        <InteractionHint
          icon="pointer"
          label="Canvas navigation"
          tokens={CANVAS_HINT_TOKENS}
          fading={canvasHintFading}
          testId="canvas-drag-hint"
          dismissLabel="Dismiss canvas navigation hint"
          onDismiss={dismissCanvasHint}
        />
      </div>
    {/if}
    {#if errorMessage}
      <div
        data-testid="graph-error-message"
        class="pointer-events-auto absolute right-4 top-4 z-[3] max-w-[130px] rounded-[10px] border border-[#e2e8f0] bg-white px-3 py-2 text-[12px] text-[#0f172a]
        shadow-[0_8px_24px_rgba(15,23,42,0.12)] font-mono"
        role="alert"
      >
        <p>{errorMessage}</p>
        <button
          type="button"
          class="mt-2 rounded border border-[#cbd5e1] px-2 py-1 text-[11px] font-sans font-medium"
          data-testid="graph-retry-button"
          on:click={() => void retryGraph()}
        >
          Retry graph
        </button>
      </div>
    {/if}
  </div>

  {#if !columnNavigatorCollapsed}
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
      bind:clientWidth={columnNavigatorDetailContainerWidthPx}
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
      <div
        class="column-navigator-graph__track"
        bind:this={columnNavigatorRail}
      >
        {#each renderedColumnNavigatorPanes as paneMotion (paneMotion.pane.pathKey)}
          {@const pane = paneMotion.pane}
          {#if pane.kind === 'column'}
            <section
              class="column-navigator-pane"
              class:column-navigator-pane--entering={paneMotion.phase === 'entering'}
              class:column-navigator-pane--exiting={paneMotion.phase === 'exiting'}
              data-testid="column-navigator-pane"
              data-column-navigator-path-key={pane.pathKey}
            >
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
        {#if hasColumnNavigatorDetail}
          <div
            class="column-navigator-graph__trailing-space"
            data-testid="column-navigator-trailing-space"
            aria-hidden="true"
            style:width={`${columnNavigatorTrailingSpacePx}px`}
          ></div>
        {/if}
      </div>
      {/if}
      {#if columnNavigatorLoading && !columnNavigatorInitialLoading}
        <span class="column-navigator-loading-indicator" role="status" aria-live="polite">Updating path…</span>
      {/if}
      {#if columnNavigatorDetailCollapseHint}
        <SplitLayoutCollapseHint side={columnNavigatorDetailCollapseHint} />
      {/if}
      {#if hasColumnNavigatorDetail}
        <div
          class={`app-split-divider app-split-divider--vertical column-navigator-detail-divider ${isDraggingColumnNavigatorDetailDivider ? 'app-split-divider--dragging' : ''} ${columnNavigatorDetailCollapsed ? 'app-split-divider--collapsed app-split-divider--right-edge' : ''}`}
          data-testid="column-navigator-detail-divider"
          role="separator"
          aria-label="Resize column navigator editor"
          aria-orientation="vertical"
          style:left={`${columnNavigatorDetailDividerLeftPx}px`}
          use:splitLayoutDrag={{
            onDragStart: ({ clientX }) => handleColumnNavigatorDetailDragStart(clientX),
            onDragMove: ({ clientX }) => handleColumnNavigatorDetailDragMove(clientX),
            onDragEnd: () => handleColumnNavigatorDetailDragEnd(),
          }}
        ></div>
        <section
          class="column-navigator-detail"
          class:column-navigator-detail--collapsed={columnNavigatorDetailCollapsed}
          class:column-navigator-detail--instant={isDraggingColumnNavigatorDetailDivider}
          data-testid="column-navigator-pane"
          data-column-navigator-content-path-key={columnNavigatorDetailPane.pathKey}
          style:width={`${columnNavigatorDetailWidthPx}px`}
        >
          <div
            class="column-navigator-pane__content column-navigator-detail__content"
            data-testid="column-navigator-content-pane"
            aria-hidden={columnNavigatorDetailCollapsed}
          >
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
                    publishNavigation(columnNavigatorDetailPane!.path, 'value', 'click');
                    void commitColumnNavigatorValueEdit(columnNavigatorDetailPane!, text);
                  }}
                  onEditorBlur={(text) => void commitColumnNavigatorValueEdit(columnNavigatorDetailPane!, text)}
                />
              {/key}
            </div>
          </div>
        </section>
        {#if columnNavigatorDetailLayout.layoutMode !== 'split'}
          <SplitLayoutCollapsedControl
            mode={columnNavigatorDetailLayout.layoutMode}
            leftPx={columnNavigatorDetailControlLeftPx}
            expandLeftLabel="Expand column navigator columns"
            expandRightLabel="Expand column navigator editor"
            testId="column-navigator-detail-expand"
            onExpand={expandColumnNavigatorDetail}
          />
        {/if}
      {/if}
    </div>
    {#if columnNavigatorHintVisible}
      <div class="column-navigator-keyboard-hint">
        <InteractionHint
          icon="keyboard"
          label="Keyboard navigation"
          tokens={COLUMN_NAVIGATOR_HINT_TOKENS}
          fading={columnNavigatorHintFading}
          testId="column-navigator-keyboard-hint"
          dismissLabel="Dismiss keyboard navigation hint"
          onDismiss={dismissColumnNavigatorHint}
        />
      </div>
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
    background: var(--panel-bg-alt);
    grid-template-rows: minmax(0, 1fr) auto;
  }

  .graph-viewer-main {
    position: relative;
    min-height: 0;
    overflow: hidden;
    outline: none;
    isolation: isolate;
  }

  .file-drop-feedback-overlay {
    position: absolute;
    z-index: 10;
    inset: 0;
    pointer-events: none;
    opacity: 0;
    background: rgb(224 243 255 / 88%);
    transition: opacity 120ms ease-out;
  }

  :global(.graph-viewer-main.file-drop-feedback--active) .file-drop-feedback-overlay {
    opacity: 1;
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
    background-color: #f8fbfd;
    background-image:
      linear-gradient(rgb(181 204 220 / 25%) 1px, transparent 1px),
      linear-gradient(90deg, rgb(181 204 220 / 25%) 1px, transparent 1px);
    background-size: 32px 32px;
  }

  .canvas-drag-hint {
    position: absolute;
    z-index: 4;
    top: 5px;
    left: 50%;
    transform: translateX(-50%);
  }

  .graph-empty-state {
    position: absolute;
    z-index: 3;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 48px 24px 70px;
    pointer-events: none;
    background: radial-gradient(ellipse 460px 300px at 50% 48%, rgb(255 255 255 / 58%), transparent 74%);
  }

  .graph-empty-state__body {
    position: relative;
    z-index: 0;
    display: flex;
    width: min(400px, 100%);
    flex-direction: column;
    align-items: center;
    padding: 22px 24px 20px;
    border: 1px solid var(--border-strong);
    border-radius: 14px;
    background: rgb(255 255 255 / 92%);
    box-shadow: 0 14px 32px rgb(29 61 78 / 10%), 0 1px 3px rgb(29 61 78 / 5%);
    backdrop-filter: blur(10px);
    text-align: center;
    pointer-events: auto;
    animation: graph-empty-state-in 420ms cubic-bezier(.22, 1, .36, 1) both;
  }

  .graph-empty-state__mark {
    display: flex;
    width: 34px;
    height: 24px;
    align-items: flex-end;
    justify-content: center;
    gap: 3px;
    margin-bottom: 10px;
    padding: 4px 6px;
    border: 1px solid color-mix(in srgb, var(--accent) 28%, var(--border-strong));
    border-radius: 7px;
    background: var(--accent-soft);
  }

  .graph-empty-state__mark span {
    width: 4px;
    border-radius: 2px 2px 1px 1px;
    background: var(--accent);
  }

  .graph-empty-state__mark span:nth-child(1) { height: 7px; opacity: .55; }
  .graph-empty-state__mark span:nth-child(2) { height: 13px; }
  .graph-empty-state__mark span:nth-child(3) { height: 10px; opacity: .75; }

  .graph-empty-state__eyebrow {
    margin-top: 0;
    color: #6f8aa0;
    font-size: 9px;
    font-weight: 750;
    letter-spacing: .18em;
  }

  .graph-empty-state h2 {
    margin: 8px 0 0;
    color: #163449;
    font-family: Georgia, 'Times New Roman', serif;
    font-family: inherit;
    font-size: 18px;
    font-weight: 720;
    letter-spacing: -.018em;
  }

  .graph-empty-state__tips {
    display: grid;
    width: 100%;
    gap: 6px;
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid var(--border-subtle);
    text-align: left;
  }

  .graph-empty-state__tip {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-muted);
    font-size: 11px;
    line-height: 1.35;
  }

  .graph-empty-state__tip-icon {
    display: inline-grid;
    width: 22px;
    height: 22px;
    flex: 0 0 auto;
    place-items: center;
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    color: var(--accent);
    background: var(--accent-soft);
  }

  .graph-empty-state__tip strong { color: var(--text-primary); font-weight: 650; }
  .graph-empty-state__tip kbd {
    display: inline-flex;
    min-width: 17px;
    height: 17px;
    align-items: center;
    justify-content: center;
    margin: 0 1px;
    padding: 0 3px;
    border: 1px solid var(--border-strong);
    border-radius: 4px;
    color: var(--text-primary);
    background: var(--panel-bg-alt);
    font: 10px ui-monospace, SFMono-Regular, Menlo, monospace;
  }

  .graph-empty-state__actions {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 18px;
  }

  .graph-empty-state__button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    min-height: 34px;
    padding: 0 14px;
    border: 1px solid transparent;
    border-radius: 8px;
    font-family: inherit;
    font-size: 12px;
    font-weight: 650;
    cursor: pointer;
    transition: transform 140ms ease, box-shadow 140ms ease, background-color 140ms ease;
  }

  .graph-empty-state__button:hover {
    transform: translateY(-1px);
  }

  .graph-empty-state__button--primary {
    color: #fff;
    background: var(--accent);
    box-shadow: 0 5px 12px rgb(45 118 151 / 22%);
  }

  .graph-empty-state__button--primary:hover {
    box-shadow: 0 7px 16px rgb(45 118 151 / 28%);
  }

  .graph-empty-state__button--secondary {
    color: #315b73;
    border-color: #c8d8e2;
    background: var(--accent-soft);
  }

  .graph-empty-state__button--secondary:hover:not(:disabled) {
    border-color: color-mix(in srgb, var(--accent) 38%, var(--border-strong));
    background: color-mix(in srgb, var(--accent-soft) 72%, #fff);
  }

  .graph-empty-state__button:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 50%, transparent);
    outline-offset: 2px;
  }

  @keyframes graph-empty-state-in {
    from { opacity: 0; transform: translateY(8px); }
    to { opacity: 1; transform: translateY(0); }
  }

  @media (max-width: 720px) {
    .graph-empty-state__body { width: min(350px, 100%); }
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
    border-top: 1px solid var(--border-strong);
    background: var(--panel-bg-alt);
    box-shadow:
      inset 0 1px 0 rgb(255 255 255 / 68%),
      0 -12px 24px rgb(29 39 53 / 7%);
  }

  .column-navigator-graph:focus-visible {
    box-shadow:
      inset 0 2px 0 color-mix(in srgb, var(--accent) 42%, transparent),
      0 -12px 24px rgb(29 39 53 / 7%);
  }

  .column-navigator-keyboard-hint {
    position: absolute;
    z-index: 2;
    bottom: calc(var(--column-navigator-height) + 10px);
    left: 50%;
    pointer-events: auto;
    transform: translateX(-50%);
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

  .column-navigator-graph__track {
    display: flex;
    height: 100%;
    min-height: 0;
    overflow-x: auto;
    overflow-y: hidden;
    overscroll-behavior-inline: contain;
    scrollbar-color: rgba(100, 116, 139, 0.4) transparent;
  }

  .column-navigator-graph__trailing-space {
    flex: 0 0 auto;
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
    border-right: 1px solid var(--border-strong);
    background: var(--panel-bg);
  }

  .column-navigator-loading-skeleton__detail {
    border-right: 0;
    background: var(--panel-bg-alt);
  }

  .column-navigator-loading-skeleton__column::after,
  .column-navigator-loading-skeleton__detail::after {
    position: absolute;
    inset: 18px 14px;
    content: '';
    background: linear-gradient(90deg, #e9e8e3 25%, #f6f5f1 38%, #e9e8e3 63%);
    background-size: 400% 100%;
    animation: column-navigator-skeleton-shimmer 1.4s ease-in-out infinite;
  }

  @keyframes column-navigator-skeleton-shimmer {
    from { background-position: 100% 0; }
    to { background-position: 0 0; }
  }

  .column-navigator-pane--entering {
    animation: column-navigator-pane-enter 220ms cubic-bezier(0.22, 1, 0.36, 1) both;
  }

  .column-navigator-pane--exiting {
    pointer-events: none;
    animation: column-navigator-pane-exit 180ms ease-out both;
  }

  @keyframes column-navigator-pane-enter {
    from { opacity: 0; transform: translateX(24px); }
    to { opacity: 1; transform: translateX(0); }
  }

  @keyframes column-navigator-pane-exit {
    from { opacity: 1; }
    to { opacity: 0; }
  }

  @media (prefers-reduced-motion: reduce) {
    .column-navigator-pane--entering,
    .column-navigator-pane--exiting {
      animation-duration: 1ms;
    }
  }

  .column-navigator-pane {
    display: grid;
    width: 288px;
    min-width: 288px;
    flex: 0 0 288px;
    min-height: 0;
    grid-template-rows: minmax(0, 1fr);
    border-right: 1px solid var(--border-strong);
    background: var(--panel-bg);
  }

  .column-navigator-detail {
    position: absolute;
    z-index: 4;
    top: 0;
    right: 0;
    bottom: 0;
    display: grid;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    grid-template-rows: minmax(0, 1fr);
    border-left: 1px solid var(--border-strong);
    background: var(--panel-bg-alt);
    transition: width 180ms cubic-bezier(0.22, 1, 0.36, 1), border-color 160ms ease;
    will-change: width;
  }

  .column-navigator-detail--instant {
    transition: none;
  }

  .column-navigator-detail--collapsed {
    border-left-color: transparent;
    background: transparent;
  }

  .column-navigator-detail__content {
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    transition: opacity 160ms ease;
  }

  .column-navigator-detail--collapsed .column-navigator-detail__content {
    opacity: 0;
    pointer-events: none;
  }

  .column-navigator-detail-divider {
    z-index: 5;
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
    border-radius: 6px;
    background: transparent;
    color: var(--text-primary);
    font-family: inherit;
    text-align: left;
    cursor: default;
    transition:
      border-color 110ms ease,
      background-color 110ms ease,
      box-shadow 110ms ease;
  }

  .column-navigator-item:hover {
    background: var(--panel-bg-alt);
  }

  .column-navigator-item.selected {
    border-color: color-mix(in srgb, var(--accent) 32%, var(--border-strong));
    background: var(--accent-soft);
    box-shadow: inset 2px 0 0 var(--accent);
  }

  .column-navigator-item.path-ancestor:not(.selected) {
    border-color: var(--border-strong);
    background: color-mix(in srgb, var(--panel-bg-alt) 72%, var(--accent-soft));
    box-shadow: none;
  }

  .column-navigator-item.index {
    grid-template-columns: 18px fit-content(38%) minmax(48px, 1fr) auto;
  }

  .column-navigator-item.index .column-navigator-item__dot {
    background: var(--accent);
    opacity: 1;
  }

  .column-navigator-item:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 42%, transparent);
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
    color: var(--text-primary);
    font-weight: 550;
  }

  .column-navigator-item__preview {
    text-align: right;
    opacity: 0.82;
  }

  .column-navigator-item__preview.container-preview {
    color: var(--text-muted);
    opacity: 1;
  }

  .column-navigator-item__chevron {
    display: block;
    color: var(--text-muted);
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
    background: var(--panel-bg-alt);
  }

  .column-navigator-pane__content-editor {
    min-height: 0;
    padding: 8px;
  }

  .column-navigator-pane__content-editor :global([data-testid='column-navigator-monaco-editor']) {
    height: 100%;
    border: 0;
    border-radius: 0;
    background: var(--panel-bg);
    overflow: hidden;
    box-shadow: none;
  }

  .column-navigator-pane__placeholder {
    display: grid;
    place-items: center;
    min-height: 0;
    padding: 16px;
    color: var(--text-muted);
    font-size: 12px;
    text-align: center;
  }

  .column-navigator-pane__placeholder--error {
    color: var(--danger);
  }

</style>
