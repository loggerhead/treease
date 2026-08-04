<!-- Responsibility: assemble the editor and graph surfaces, coordinate cross-component events, and handle DOM interaction. -->
<script lang="ts">
  import type { PageData } from './$types';
  import { get } from 'svelte/store';
  import { onMount, tick } from 'svelte';
  import { cubicOut } from 'svelte/easing';
  import { fly } from 'svelte/transition';
  import Editor from '../../lib/components/Editor.svelte';
  import GraphTopBar from '../../lib/components/GraphTopBar.svelte';
  import TabSwitcher from '../../lib/components/TabSwitcher.svelte';
  import TreePathBar from '../../lib/components/TreePathBar.svelte';
  import ColumnNavigatorControls from '../../lib/components/ColumnNavigatorControls.svelte';
  import FunctionBar from '../../lib/components/FunctionBar.svelte';
  import Sidebar from '../../lib/components/Sidebar.svelte';
  import ViewportPanel from '../../lib/components/ViewportPanel.svelte';
  import StructGenerationInput from '../../lib/components/StructGenerationInput.svelte';
  import LoginDialog from '../../lib/components/LoginDialog.svelte';
  import AiInputPanel from '../../lib/components/AiInputPanel.svelte';
  import type { PricingUsageNotice } from '../../lib/components/PricingPlanGrid.svelte';
  import YqInputBox from '../../lib/components/YqInputBox.svelte';
  import { settings, settingsStore } from '../../lib/settings/settings-store';
  import { DEFAULT_EDITOR_SPLIT_RATIO, DEFAULT_SIDEBAR_EXPANDED } from '../../lib/settings/editor-layout-state';
  import {
    activeTempModel,
    initialTempModel,
  } from '../../lib/store/graph-selection-store';
  import { initialFullEditUiState } from '../../lib/store/full-edit-ui-store';
  import {
    documentKey as documentKeyStore,
    languageId as languageIdStore,
    sourceText as sourceTextStore,
    editorRevision,
    graphAppliedRevision,
  } from '../../lib/store/document-session-store';
  import { toast } from 'svelte-sonner';
  import { fetchUrlPresetSource, type UrlPresetSource } from './fetch-url-preset-source';
  import { readImportSourceSample, resolveImportSourceFormat } from '../../lib/import/resolve-import-source';
  import { callSharedWasmWorker, getSharedWasmWorkerClient } from '../../lib/wasm/wasm-worker-singleton';
  import { getDefaultWasmURL } from '../../lib/wasm/wasm-worker-singleton';
  import { runYqPreview } from './yq-preview-controller';
  import { prepareStructGenerationSource } from './struct-generation-controller';
  import {
    canExecuteUrlCommandForLanguage,
    isEditorResetRequested,
    resolveEditorUrlPreset,
    summarizeEditorUrlPresetWarnings,
    type EditorUrlActionCommandId,
    type ResolvedEditorUrlPreset,
  } from './url-preset';
  import { resolveExportDownloadDetails, resolveExportPreviewDetails } from './export-controller';
  import {
    collapseEditor as collapseEditorLayout,
    collapseViewer as collapseViewerLayout,
    computePaneWidths,
    createSplitLayoutDragController,
    createSplitLayoutState,
    expandSplit,
    syncSplitRatio,
    type SplitLayoutConfig,
  } from './split-layout-controller';
  import { resolveSplitLayoutMotion } from './split-layout-motion';
  import { splitLayoutDrag } from '../../lib/components/ui/split-layout';
  import { importFormatOptions, supportedEditorLanguageSet, editorLanguageFallback, findSupportedLanguageByExtension, type SupportedEditorLanguageId } from '../../lib/monaco/language-support';
  import { computeSynchronizedRuntimeLoading, type RuntimeStateEventDetail } from '../../lib/runtime-loading';
  import { getActiveDocumentText } from '../../lib/store/active-document-authority';
  import { getLanguageExample } from '../../lib/monaco/language-examples';
  import { breadcrumbTargetForPath, isPathSegIndex, pathSegKeyValue, type PathSeg } from '../../lib/store/tree-path';
  import { PathSegTag } from '@core-wasm/index';
  import { markPreviewCompleted, markPreviewRequested } from '../../lib/test-bridge/runtime-readiness';
  import { setTreeaseUrlPresetState } from '../../lib/test-bridge/window-treease';
  import { resetBrowserLocalState } from '../../lib/workspace-host/browser-storage';
  import type { DiffPlan } from '../../lib/graph/diff-plan';
  import type { ColumnNavigatorState } from '../../lib/components/graph-viewer/column-navigator/types';
  import { serializePath } from '../../shared/document-anchor-utils';
  import {
    GitCompareArrows,
    GitGraph,
    Check,
    CircleAlert,
  } from 'lucide-svelte';
  import Tooltip from '../../lib/components/Tooltip.svelte';
  import { SplitLayoutCollapsedControl, SplitLayoutCollapseHint } from '../../lib/components/ui/split-layout';
  import { trackEvent } from '../../lib/analytics/ga4';
  import { startBillingCheckout } from '../../lib/billing/checkout-flow';
  import { getUsageClientId } from '../../lib/billing/client-id';
  import { runPostpaidCapability } from '../../lib/billing/entitlement-gate';
  import { workspaceHost } from '../../lib/workspace-host';
  import { exchangeAuthCode, signOut } from '../../lib/auth/supabase-auth';
  import { editorWorkspace, getWorkspaceState, setWorkspaceState, updateWorkspaceTab } from '../../lib/store/workspace-store';
  import {
    activateWorkspaceTabTransition,
    createEditorWorkspaceState,
    createWorkspaceTabTransition,
    isWorkspaceTabDirty,
    summarizeWorkspaceTabs,
    type EditorWorkspaceTabSummary,
  } from '../../lib/store/editor-workspace';
  import { clearCompareState, compareState } from '../../lib/store/compare-state';
  import {
    generateStruct,
    getUsageSummary,
    getPublicShare,
    suggestYq,
    TreeaseServerError,
    type StructLanguage,
  } from '../../lib/services/treease-server';
  import { createShareResource as createResourceFromState, type ShareInteraction, type SharePathSegment, type ShareResource } from '../../lib/share/share-resource';
  import { restoreShareResource } from '../../lib/share/share-restore';
  import {
    createSharedWorkspaceLifecycle,
    type SharedWorkspaceLifecycle,
    type SharedWorkspaceMutationTarget,
  } from '../../lib/share/share-workspace-lifecycle';
  import { workspaceSessionFromWorkspace } from '../../lib/workspace-host/workspace-session';
  import type { WorkspaceCommand, WorkspaceSession } from '../../lib/workspace-host';
  import { createViewRuntimeOperation } from '../../lib/guards/view-runtime-operation';
  import { LARGE_FILE_PROCESSING_THRESHOLD_BYTES } from '../../lib/config/large-file';
  import type { CommandId } from '../../lib/command-registry';
  import {
    createWorkspaceNavigationRuntime,
    type GraphRuntimeReadinessBinding,
  } from '../../lib/navigation/workspace-navigation-runtime';
  import type { NavigationResult, NavigationTarget, NavigationUserEvent } from '../../lib/navigation/navigation-contract';

  export let data: PageData;

  let editorRef: Editor | null = null;
  let sidebarRef: Sidebar | null = null;
  let viewerRef: ViewportPanel | null = null;
  let yqInputBoxRef: YqInputBox | null = null;
  let splitLayoutContainer: HTMLDivElement | null = null;
  let containerWidth = 0;
  let tabSummaries: EditorWorkspaceTabSummary[] = [];
  let activeTabId = '';
  let workspaceBootstrapReady = false;
  let workspaceCommandReady = false;
  let resolveWorkspaceBootstrap: (() => void) | null = null;
  const workspaceBootstrapComplete = new Promise<void>((resolve) => {
    resolveWorkspaceBootstrap = resolve;
  });
  let scrollSyncLock: 'editor' | 'viewer' | null = null;
  const serverSplitRatio = data.editorSplitRatio;
  const serverSidebarExpanded = data.sidebarExpanded;
  let sidebarExpanded = serverSidebarExpanded ?? DEFAULT_SIDEBAR_EXPANDED;
  let splitLayoutState = createSplitLayoutState(serverSplitRatio ?? DEFAULT_EDITOR_SPLIT_RATIO);
  let layoutReady = false;
  let layoutMode = splitLayoutState.layoutMode;
  let splitRatio = splitLayoutState.splitRatio;
  let lastSplitRatio = splitLayoutState.lastSplitRatio;
  let leftPaneWidthPx = 0;
  let rightPaneWidthPx = 0;
  let splitterLeftPx = 0;
  let splitterControlLeftPx = 0;
  let leftPaneCollapsed = false;
  let rightPaneCollapsed = false;
  let collapsedControlFlyX = 0;
  let isDraggingSplitter = false;
  let splitterCollapseHint: 'editor' | 'viewer' | null = null;
  let settingsOpen = false;
  let shareOpen = false;
  let feedbackOpen = false;
  let structGenerationOpen = false;
  let structGenerationTarget: StructLanguage = 'typescript';
  let structGenerationRootName = 'Root';
  let structGenerationBusy = false;
  let structGenerationError = '';
  let loginOpen = false;
  type GraphSurfaceMode = 'graph' | 'compare';
  let viewerViewMode: 'graph' | 'text' = 'graph';
  let graphSurfaceMode: GraphSurfaceMode = 'graph';
  let editorRuntimeLoading = true;
  let viewerRuntimeLoading = true;
  let columnNavigatorState: ColumnNavigatorState | null = null;
  let synchronizedRuntimeLoading = true;
  let syncScrollEnabled = true;
  let aiInputOpen = false;
  let aiInstruction = '';
  let aiBusy = false;
  let aiError = '';
  let aiSuccess = '';
  let aiQuotaExhausted = false;
  let aiUpgradeBusy = false;
  let documentInvalid = false;
  type PricingPlanGridComponent = typeof import('../../lib/components/PricingPlanGrid.svelte').default;
  let pricingPlanGridComponent: PricingPlanGridComponent | null = null;
  let pricingPlanGridLoad: Promise<PricingPlanGridComponent> | null = null;
  let aiUsageNotice: PricingUsageNotice | null = null;
  let yqInputOpen = false;
  let yqExpression = '';
  let yqBusy = false;
  let yqError = '';
  let previewRequestId = 0;
  let showEditorPane = true;
  let showViewerPane = true;
  let showTopBar = true;
  let urlPreset: ResolvedEditorUrlPreset | null = null;
  let mirrorViewerFromSource = false;
  let externalFileConflict: { tabId: string; name: string; externalText: string; localText: string; languageId: SupportedEditorLanguageId } | null = null;
  let autoSaveTimer: ReturnType<typeof setTimeout> | null = null;
  let sessionSaveTimer: ReturnType<typeof setTimeout> | null = null;
  let sessionRestoring = false;
  let stopWorkspaceSession: (() => void) | null = null;
  let sharedWorkspaceLifecycle: SharedWorkspaceLifecycle | null = null;
  let lastMirroredViewerText = '';
  let lastTrackedGraphViewRevision = -1;
  let shareLoadError = '';
  let shareLoading = false;
  const maxTabs = 9;
  const wasmUrl = getDefaultWasmURL();
  type ScrollPosition = { scrollTop: number; scrollLeft: number };
  type ScrollSyncOwner = 'editor' | 'viewer';
  type SplitLayoutState = ReturnType<typeof createSplitLayoutState>;

  const formatOptions = importFormatOptions;
  const defaultSplitRatio = DEFAULT_EDITOR_SPLIT_RATIO;
  const splitLayoutConfig: SplitLayoutConfig = {
    defaultSplitRatio,
    minPaneWidthPx: 200,
    dividerWidthPx: 10,
    collapsedControlInsetPx: 16,
    collapsedPaneWidthPx: 44,
  };
  const splitLayoutDragController = createSplitLayoutDragController(splitLayoutConfig);
  const urlCommandHandlers: Record<Exclude<EditorUrlActionCommandId, 'compare'>, () => Promise<void>> = {
    format: async () => {
      await editorRef?.formatActive();
    },
    minify: async () => {
      await editorRef?.minifyActive();
    },
    sort: async () => {
      await editorRef?.sortActive();
    },
    escape: async () => {
      await editorRef?.escapeActive();
    },
    unescape: async () => {
      await editorRef?.unescapeActive();
    },
  };

  function shouldMirrorCommandResultToViewer(nextPreset: ResolvedEditorUrlPreset): boolean {
    return !nextPreset.ui.editor && nextPreset.ui.viewer;
  }


  function selectGraphSurfaceMode(nextMode: GraphSurfaceMode): void {
    graphSurfaceMode = nextMode;
    viewerViewMode = nextMode === 'graph' ? 'graph' : 'text';
  }

  function applyUrlPresetUi(nextPreset: ResolvedEditorUrlPreset): void {
    showEditorPane = nextPreset.ui.editor;
    showViewerPane = nextPreset.ui.viewer;
    showTopBar = nextPreset.ui.topbar;
    viewerViewMode = nextPreset.initialViewerMode;
    graphSurfaceMode = nextPreset.initialViewerMode === 'graph' ? 'graph' : 'compare';
    mirrorViewerFromSource = false;
    lastMirroredViewerText = '';
  }

  function setUrlPresetBridgeState(nextPreset: ResolvedEditorUrlPreset): void {
    setTreeaseUrlPresetState({
      ...nextPreset.telemetry,
      warnings: [...nextPreset.telemetry.ignored, ...nextPreset.notes],
      viewerMode: nextPreset.initialViewerMode,
    });
  }

  async function waitForEditorCommandResult(previousText: string): Promise<string> {
    let nextText = getActiveDocumentText() || editorRef?.getActiveText() || previousText;
    if (nextText !== previousText) return nextText;
    const deadline = Date.now() + 10_000;
    while (Date.now() < deadline) {
      await tick();
      await new Promise<void>((resolve) => setTimeout(resolve, 16));
      nextText = getActiveDocumentText() || editorRef?.getActiveText() || previousText;
      if (nextText !== previousText) return nextText;
    }
    return nextText;
  }

  async function fetchUrlPresetSourceOrReport(rawUrl: string): Promise<UrlPresetSource | null> {
    try {
      return await fetchUrlPresetSource(rawUrl);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      console.error('[editor] failed to apply url preset', { error: message });
      toast.error(`Editor URL preset failed: ${message}`);
      return null;
    }
  }

  async function applyEditorUrlPreset(nextPreset: ResolvedEditorUrlPreset): Promise<void> {
    applyUrlPresetUi(nextPreset);
    setUrlPresetBridgeState(nextPreset);
    await workspaceBootstrapComplete;
    await tick();
    if (nextPreset.nest !== null) {
      await settingsStore.save({ parser: { enableNest: nextPreset.nest } });
    }
    if (nextPreset.autoFormat !== null) {
      await settingsStore.save({ formatting: { ...settingsStore.get().settings.formatting, smart: nextPreset.autoFormat } });
    }
    void getSharedWasmWorkerClient().catch(() => {});
    await tick();
    await editorRef?.ensureReady?.();

    let presetText = nextPreset.text.present ? nextPreset.text.value : null;
    let presetTextLanguage = nextPreset.language;
    if (presetText === null && nextPreset.textUrl.effective) {
      const resolved = await fetchUrlPresetSourceOrReport(nextPreset.textUrl.value);
      if (!resolved) return;
      presetText = resolved.text;
      presetTextLanguage = presetTextLanguage ?? resolved.inferredLanguage;
    }

    if (presetText !== null) {
      const nextLanguage = presetTextLanguage ?? (editorRef?.getActiveLanguage() ?? $languageIdStore);
      await editorRef?.importAs(nextLanguage, presetText, nextLanguage);
      await editorRef?.waitForIdle?.();
    } else if (nextPreset.language) {
      languageIdStore.set(nextPreset.language);
      await tick();
    }

    if (nextPreset.rightText.effective) {
      const nextLanguage = nextPreset.language ?? (editorRef?.getActiveLanguage() ?? $languageIdStore);
      await showViewerTextPreview(nextPreset.rightText.value, nextLanguage);
    } else if (nextPreset.rightTextUrl.effective) {
      const resolved = await fetchUrlPresetSourceOrReport(nextPreset.rightTextUrl.value);
      if (!resolved) return;
      const nextLanguage = nextPreset.language ?? resolved.inferredLanguage ?? (editorRef?.getActiveLanguage() ?? $languageIdStore);
      await showViewerTextPreview(resolved.text, nextLanguage);
    }

    if (nextPreset.yq.effective) {
      yqExpression = nextPreset.yq.value;
      await handleSubmitYq(nextPreset.yq.value);
    } else if (nextPreset.command) {
      const effectiveLanguage = editorRef?.getActiveLanguage() ?? $languageIdStore;
      if (!canExecuteUrlCommandForLanguage(nextPreset.command, effectiveLanguage)) {
        nextPreset.notes.push(`Ignored command=${nextPreset.command} for language=${effectiveLanguage}.`);
      } else if (nextPreset.command === 'compare') {
        if (!nextPreset.rightText.effective) {
          await showViewerTextPreview(viewerRef?.getActiveText() ?? '', viewerRef?.getActiveLanguage() ?? effectiveLanguage);
        }
        await viewerRef?.waitForIdle?.();
        await viewerRef?.runCompare?.();
      } else {
        const previousText = getActiveDocumentText();
        await urlCommandHandlers[nextPreset.command]();
        await editorRef?.waitForIdle?.();
        if (shouldMirrorCommandResultToViewer(nextPreset)) {
          mirrorViewerFromSource = true;
          const nextText = await waitForEditorCommandResult(previousText);
          lastMirroredViewerText = nextText;
          await showViewerTextPreview(nextText, editorRef?.getActiveLanguage() ?? effectiveLanguage);
        }
      }
    }

    setUrlPresetBridgeState(nextPreset);
    const warningMessage = summarizeEditorUrlPresetWarnings(nextPreset);
    if (warningMessage) {
      toast.warning(warningMessage);
    }
  }

  async function restoreShare(shareID: string): Promise<void> {
    shareLoading = true;
    shareLoadError = '';
    try {
      const { resource } = await getPublicShare(shareID);
      await tick();
      if (!editorRef || !viewerRef) throw new Error('Editor is not ready.');
      await restoreShareResource(resource, {
        editor: editorRef,
        viewer: viewerRef,
        setViewMode: (mode) => {
          viewerViewMode = mode;
          graphSurfaceMode = mode === 'graph' ? 'graph' : 'compare';
        },
        clearCompareState,
        restoreTreePath: (path) => {
          activeTempModel.update((current) => ({ ...current, treePath: fromSharePath(path) }));
          return true;
        },
        restoreGraphFocus: (path, target) => {
          const localPath = fromSharePath(path);
          return viewerRef?.revealPath(localPath, { target }) ?? Promise.resolve(false);
        },
        waitForGraphReady: () => viewerRef?.waitForGraphReady() ?? Promise.resolve(false),
        restoreColumnNavigator: async (activePath) => await viewerRef.restoreColumnNavigatorPath(fromSharePath(activePath)),
        reportNavigationWarning: () => toast.warning('The shared document opened, but part of its saved navigation could not be restored.'),
      });
      sharedWorkspaceLifecycle?.completeRestore();
    } catch (error) {
      sharedWorkspaceLifecycle?.failRestore(error);
      shareLoadError = error instanceof Error ? error.message : 'Unable to load shared content.';
      clearCompareState();
    } finally {
      shareLoading = false;
    }
  }

  async function createShareResource(): Promise<ShareResource> {
    if (!editorRef) throw new Error('The editor is not ready yet.');
    const left = { text: editorRef.getActiveText() ?? '', languageId: editorRef.getActiveLanguage() ?? $languageIdStore };
    const rightText = viewerRef?.getActiveText();
    const rightLanguage = viewerRef?.getActiveLanguage();
    const treePath = toSharePath(get(activeTempModel).treePath);
    const graphHighlight = get(activeTempModel).graphHighlight;
    const selection = editorRef.getSelection();
    const interaction: ShareInteraction = {
      treePath,
      focus: graphHighlight && selection
        ? { type: 'graph', path: toSharePath(graphHighlight.path), target: graphHighlight.target, editorSelection: selection }
        : selection ? { type: 'editor', selection } : null,
      columnNavigator: { activePath: viewerRef ? toSharePath(viewerRef.getColumnNavigatorActivePath()) : [] },
    };
    return createResourceFromState({
      compareKind: $compareState.kind,
      left,
      right: viewerViewMode !== 'text' || rightText === undefined || !rightLanguage ? null : { text: rightText, languageId: rightLanguage },
      layout: { viewMode: viewerViewMode, activePane: 'left' },
      viewport: { left: editorRef.getViewportAnchor() ?? { topLine: 1, scrollLeft: 0 }, right: viewerRef?.getViewportAnchor() ?? { topLine: 1, scrollLeft: 0 } },
      interaction,
    });
  }

  function toSharePath(path: PathSeg[]): SharePathSegment[] {
    return path.map((segment) => isPathSegIndex(segment)
      ? { type: 'index' as const, index: segment.index }
      : { type: 'key' as const, key: pathSegKeyValue(segment) });
  }

  function fromSharePath(path: SharePathSegment[]): PathSeg[] {
    return path.map((segment) => segment.type === 'index'
      ? { tag: PathSegTag.INDEX, index: segment.index }
      : { tag: PathSegTag.KEY, key: segment.key, index: 0 }) as PathSeg[];
  }

  if (typeof window !== 'undefined') {
    urlPreset = resolveEditorUrlPreset(window.location.search);
    if (urlPreset.shareID.present && urlPreset.shareID.valid) shareLoading = true;
    if (urlPreset.shareID.present && !urlPreset.shareID.valid) shareLoadError = 'The share link contains an invalid share ID.';
    if (!urlPreset.shareID.present) {
      applyUrlPresetUi(urlPreset);
      setUrlPresetBridgeState(urlPreset);
    }
  }

  function getContainerWidth() {
    return splitLayoutContainer?.clientWidth ?? 0;
  }

  function syncSplitLayoutState(nextState = splitLayoutState) {
    splitLayoutState = nextState;
    layoutMode = nextState.layoutMode;
    splitRatio = nextState.splitRatio;
    lastSplitRatio = nextState.lastSplitRatio;
  }

  function formatPaneWidth(ratio: number): string {
    return `${(ratio * 100).toFixed(1)}%`;
  }

  function formatVisiblePaneWidth(ratio: number, paneWidthPx: number): string {
    return visibleLayoutMode === 'split' ? formatPaneWidth(ratio) : `${paneWidthPx}px`;
  }

  function updateSplitLayout(mutator: (state: SplitLayoutState) => SplitLayoutState) {
    syncSplitLayoutState(mutator(splitLayoutState));
  }

  function resetViewerTextScroll() {
    queueMicrotask(() => {
      viewerRef?.setTextScrollPosition?.({ scrollTop: 0, scrollLeft: 0 });
    });
  }

  async function showViewerTextPreview(
    text: string,
    language: SupportedEditorLanguageId | undefined = undefined,
  ): Promise<void> {
    graphSurfaceMode = 'compare';
    viewerViewMode = 'text';
    await viewerRef?.showTextPreview(text, language);
  }

  async function showViewerTextPreviewForRevision(
    text: string,
    language: SupportedEditorLanguageId,
    sourceRevision: number,
  ) {
    const requestId = ++previewRequestId;
    markPreviewRequested({
      requestId,
      sourceRevision,
    });
    await showViewerTextPreview(text, language);
    markPreviewCompleted({
      requestId,
      sourceRevision,
      completedRevision: sourceRevision,
    });
  }

  function handleEditorRuntimeEvent(event: CustomEvent<RuntimeStateEventDetail>) {
    editorRuntimeLoading = event.detail.loading;
  }

  function handleViewerRuntimeState(payload: RuntimeStateEventDetail) {
    viewerRuntimeLoading = payload.loading;
    if (payload.interactiveReady === true && !payload.error) void graphReadinessBinding?.reportInteractive();
    if (!payload.loading && !payload.error && viewerViewMode === 'graph' && $editorRevision !== lastTrackedGraphViewRevision) {
      lastTrackedGraphViewRevision = $editorRevision;
      trackEvent('graph_view', { language: editorRef?.getActiveLanguage() ?? $languageIdStore });
    }
  }

  function handleColumnNavigatorState(payload: ColumnNavigatorState): void {
    columnNavigatorState = payload;
  }

  $: runtimeGateViewMode = showViewerPane ? viewerViewMode : 'text';
  $: synchronizedRuntimeLoading = computeSynchronizedRuntimeLoading({
    viewMode: runtimeGateViewMode,
    editorRuntimeLoading,
    graphRuntimeLoading: viewerRuntimeLoading,
  });
  $: if (
    mirrorViewerFromSource &&
    showViewerPane &&
    !showEditorPane &&
    viewerViewMode === 'text' &&
    viewerRef &&
    $sourceTextStore &&
    $sourceTextStore !== lastMirroredViewerText
  ) {
    lastMirroredViewerText = $sourceTextStore;
    void showViewerTextPreview($sourceTextStore, editorRef?.getActiveLanguage() ?? $languageIdStore);
  }
  $: syncScrollEnabled = $settings?.interaction?.enableSyncScroll ?? true;
  $: autoSaveMode = $settings?.interaction?.autoSave ?? 'off';
  $: if (autoSaveMode === 'afterDelay' && $sourceTextStore) {
    if (autoSaveTimer) clearTimeout(autoSaveTimer);
    autoSaveTimer = setTimeout(() => void saveActiveDocument(false, true), 1_000);
  }

  async function handleImportFileStream(payload: {
    file: File;
    sourceFormat: string;
    targetFormat: string;
    fileName: string;
  }) {
    const largeFileOperationId = crypto.randomUUID();
    try {
      const sample = await readImportSourceSample(payload.file);
      const effectiveSource = await resolveImportSourceFormat(
        payload.fileName,
        sample,
        payload.sourceFormat as SupportedEditorLanguageId,
      );
      const targetFormat =
        payload.targetFormat === editorLanguageFallback &&
        supportedEditorLanguageSet.has(effectiveSource as SupportedEditorLanguageId)
          ? (effectiveSource as SupportedEditorLanguageId)
          : (payload.targetFormat as SupportedEditorLanguageId);
      const importFile = () => editorRef?.importStream(payload.file, effectiveSource, targetFormat);
      if (payload.file.size >= LARGE_FILE_PROCESSING_THRESHOLD_BYTES) {
        await runPostpaidCapability({
          capability: 'large_file_processing',
          idempotencyKey: largeFileOperationId,
          metadata: { byteLength: payload.file.size },
          surface: 'file_import',
          execute: importFile,
          onBlocked: (block) => viewerRef?.showEntitlementOverlay(block),
        });
      } else {
        await importFile();
      }
      trackEvent('document_import', { source: 'file', language: targetFormat, result: 'success' });
      toast.success(`Imported ${payload.fileName}`);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      console.error('[import] file import failed', { fileName: payload.fileName, error: message });
      trackEvent('document_import', { source: 'file', result: 'failure' });
      toast.error(`Import failed: ${message}`);
    }
  }

  async function handleRequestImportFile(payload: { sourceFormat: string; targetFormat: string; accept: string[] }) {
    try {
      const file = await (await workspaceHost).openFile({ accept: payload.accept });
      if (!file) return;
      await handleImportFileStream({ ...payload, file, fileName: file.name });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(`Import failed: ${message}`);
    }
  }

  function languageForFileName(fileName: string): SupportedEditorLanguageId {
    const extension = fileName.split('.').pop()?.toLowerCase() ?? '';
    const language = findSupportedLanguageByExtension(extension);
    return supportedEditorLanguageSet.has(language as SupportedEditorLanguageId)
      ? (language as SupportedEditorLanguageId)
      : editorLanguageFallback;
  }

  function activeWorkspaceTab() {
    const workspace = getWorkspaceState();
    return workspace.tabsById[workspace.activeTabId] ?? null;
  }

  function createActivePreviewOperation() {
    const context = () => {
      const workspace = getWorkspaceState();
      const tab = workspace.tabsById[workspace.activeTabId];
      return {
        documentKey: tab?.documentKey ?? '',
        revision: tab?.revision ?? -1,
        languageId: tab?.languageId ?? '',
        sessionId: workspace.activeTabId,
      };
    };
    return createViewRuntimeOperation({ captured: context(), getCurrent: context });
  }

  async function openWorkspaceFile(file: Awaited<ReturnType<Awaited<typeof workspaceHost>['openFile']>>): Promise<void> {
    if (!file) return;
    const text = await file.text();
    const languageId = languageForFileName(file.name);
    const tabId = editorRef?.openDocument({
      name: file.name,
      text,
      languageId,
      fileLinkedDocument: file.fileAccessGrant ? { grantId: file.fileAccessGrant.id, name: file.fileAccessGrant.name } : undefined,
    });
    if (!tabId) {
      toast.error('Cannot open another document while all tabs are in use.');
      return;
    }
    if (file.fileAccessGrant) {
      await watchFileForExternalChanges(tabId, file.fileAccessGrant);
    }
    toast.success(`Opened ${file.name}`);
  }

  async function handleOpenDocument(): Promise<void> {
    const file = await (await workspaceHost).openFile({ accept: ['.json', '.jsonl', '.ndjson', '.yaml', '.yml', '.toml', '.csv'] });
    await openWorkspaceFile(file);
  }

  async function handleOpenRecentFile(grant: { id: string; name: string }): Promise<void> {
    await openWorkspaceFile(await (await workspaceHost).openRecentFile(grant));
  }

  async function handleClearRecentFiles(): Promise<void> {
    await (await workspaceHost).clearRecentFiles();
  }

  async function saveActiveDocument(forceSaveAs = false, automatic = false): Promise<void> {
    const tab = activeWorkspaceTab();
    if (!tab) return;
    const host = await workspaceHost;
    const text = editorRef?.getActiveText() ?? tab.sourceText;
    if (automatic && !tab.fileLinkedDocument) return;
    const extension = formatOptions.find((option) => option.id === tab.languageId)?.extensions[0] ?? `.${tab.languageId}`;
    const defaultName = tab.fileLinkedDocument?.name ?? `${tab.name}${extension}`;
    if (!forceSaveAs && tab.fileLinkedDocument) {
      await host.saveFile({ id: tab.fileLinkedDocument.grantId, name: tab.fileLinkedDocument.name }, text);
      updateWorkspaceTab(tab.id, { savedText: text });
      toast.success(`Saved ${tab.fileLinkedDocument.name}`);
      return;
    }
    const grant = await host.saveFileAs({ fileName: defaultName, text, mimeType: 'text/plain;charset=utf-8' });
    if (!grant) return;
    editorRef?.renameDocument(tab.id, grant.name);
    updateWorkspaceTab(tab.id, { name: grant.name, fileLinkedDocument: { grantId: grant.id, name: grant.name }, savedText: text });
    await watchFileForExternalChanges(tab.id, grant);
    toast.success(`Saved ${grant.name}`);
  }

  const fileWatchUnsubscribers = new Map<string, () => void | Promise<void>>();

  async function watchFileForExternalChanges(tabId: string, grant: { id: string; name: string }): Promise<void> {
    const previous = fileWatchUnsubscribers.get(tabId);
    if (previous) await previous();
    const stop = await (await workspaceHost).watchFile(grant, async () => {
      const workspace = getWorkspaceState();
      const tab = workspace.tabsById[tabId];
      if (!tab?.fileLinkedDocument) return;
      const opened = await (await workspaceHost).readFile({ id: tab.fileLinkedDocument.grantId, name: tab.fileLinkedDocument.name });
      if (!opened || opened.text === tab.savedText) return;
      if (tab.sourceText !== tab.savedText) {
        externalFileConflict = {
          tabId,
          name: tab.fileLinkedDocument.name,
          externalText: opened.text,
          localText: tab.sourceText,
          languageId: tab.languageId,
        };
        return;
      }
      if (!(await editorRef?.replaceDocumentFromFile({ tabId, text: opened.text, languageId: tab.languageId }))) return;
      updateWorkspaceTab(tabId, { sourceText: opened.text, savedText: opened.text });
      toast.info(`Reloaded external changes from ${tab.fileLinkedDocument.name}`);
    });
    fileWatchUnsubscribers.set(tabId, stop);
  }

  async function compareExternalFileChange(): Promise<void> {
    if (!externalFileConflict) return;
    viewerViewMode = 'text';
    await showViewerTextPreview(externalFileConflict.externalText);
  }

  async function overwriteExternalFileChange(): Promise<void> {
    const conflict = externalFileConflict;
    if (!conflict) return;
    const tab = getWorkspaceState().tabsById[conflict.tabId];
    if (!tab?.fileLinkedDocument) return;
    await (await workspaceHost).saveFile({ id: tab.fileLinkedDocument.grantId, name: tab.fileLinkedDocument.name }, conflict.localText);
    updateWorkspaceTab(conflict.tabId, { savedText: conflict.localText });
    externalFileConflict = null;
    toast.success(`Overwrote external changes in ${tab.fileLinkedDocument.name}`);
  }

  async function discardLocalFileChange(): Promise<void> {
    const conflict = externalFileConflict;
    if (!conflict) return;
    if (!(await editorRef?.replaceDocumentFromFile({ tabId: conflict.tabId, text: conflict.externalText, languageId: conflict.languageId }))) return;
    updateWorkspaceTab(conflict.tabId, { sourceText: conflict.externalText, savedText: conflict.externalText });
    externalFileConflict = null;
    toast.info(`Reloaded ${conflict.name}`);
  }

  async function resolveExportText(format: string): Promise<string | null> {
    const text = await editorRef?.exportAs(format);
    return typeof text === 'string' ? text : null;
  }

  async function handleExportPreview(format: string) {
    const text = await resolveExportText(format);
    if (text == null) return;
    const preview = resolveExportPreviewDetails({
      sourceLanguage: $languageIdStore,
      targetFormat: format,
      formatOptions,
    });
    await showViewerTextPreviewForRevision(text, preview.previewLanguage, $editorRevision);
    trackEvent('document_export', { source: 'preview', format, result: 'success' });
    toast.success(preview.toastMessage);
  }

  async function handleExportDownload(format: string) {
    const text = await resolveExportText(format);
    if (text == null) return;
    const download = resolveExportDownloadDetails({
      sourceLanguage: $languageIdStore,
      targetFormat: format,
      tabName: tabSummaries.find((tab) => tab.id === activeTabId)?.name,
      formatOptions,
    });
    try {
      await (await workspaceHost).saveText({
        text,
        fileName: download.fileName,
        mimeType: 'text/plain;charset=utf-8',
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(`Export failed: ${message}`);
      return;
    }
    trackEvent('document_export', { source: 'download', format, result: 'success' });
    for (const message of download.toastMessages) {
      toast.success(message);
    }
  }

  function handleShowAiInputPanel() {
    if (aiInputOpen) {
      handleCloseAiInputPanel();
      return;
    }
    structGenerationOpen = false;
    yqInputOpen = false;
    aiInputOpen = true;
    aiError = '';
    aiSuccess = '';
    aiQuotaExhausted = false;
    aiUsageNotice = null;
  }

  async function ensurePricingPlanGrid(): Promise<PricingPlanGridComponent> {
    if (pricingPlanGridComponent) return pricingPlanGridComponent;
    pricingPlanGridLoad ??= import('../../lib/components/PricingPlanGrid.svelte').then((module) => module.default);
    pricingPlanGridComponent = await pricingPlanGridLoad;
    return pricingPlanGridComponent;
  }

  async function openPricingOverlay(): Promise<void> {
    if (!aiUsageNotice) await refreshAiUsageNotice();
    await ensurePricingPlanGrid();
    viewerRef?.showPricingOverlay(aiUsageNotice);
  }

  function handleEntitlementBlocked(): void {
    void ensurePricingPlanGrid();
  }

  function handleCloseAiInputPanel() {
    if (aiBusy) return;
    aiInputOpen = false;
    aiError = '';
    aiSuccess = '';
    aiQuotaExhausted = false;
  }

  async function handleSubmitAi(instruction: string) {
    const sourceText = getActiveDocumentText();
    const operation = createActivePreviewOperation();
    aiInstruction = instruction;
    aiBusy = true;
    aiError = '';
    aiSuccess = '';
    aiQuotaExhausted = false;
    if (!sourceText.trim()) {
      aiError = 'The active document is empty.';
      aiBusy = false;
      return;
    }

    try {
      const currentPath = get(activeTempModel).treePath;
      const suggestion = await operation.step(() => suggestYq({
        instruction,
        editorTextSnapshot: sourceText,
        treePathSet: currentPath.length ? [serializePath(currentPath)] : undefined,
      }));
      if (!suggestion) return;
      const result = await operation.step(() => runYqPreview({
        expression: suggestion.expression,
        text: sourceText,
        language: editorRef?.getActiveLanguage() ?? $languageIdStore,
        formatting: $settings.formatting,
        enableNest: $settings.parser.enableNest,
        callWorker: callSharedWasmWorker,
      }));
      if (!result) return;
      if ('error' in result) {
        aiError = result.error;
        return;
      }
      if (!operation.isCurrent()) return;
      await showViewerTextPreviewForRevision(result.result, result.previewLanguage, $editorRevision);
      if (!operation.isCurrent()) return;
      aiSuccess = suggestion.expression;
    } catch (error) {
      if (!operation.isCurrent()) return;
      if (error instanceof TreeaseServerError && error.code === 'quota_exhausted') {
        aiQuotaExhausted = true;
        aiError = error.message;
        void (async () => {
          await refreshAiUsageNotice();
          await openPricingOverlay();
        })();
      } else {
        aiError = error instanceof Error ? error.message : 'Unable to generate a yq expression.';
      }
    } finally {
      aiBusy = false;
    }
  }

  async function refreshAiUsageNotice(): Promise<void> {
    try {
      const summary = await getUsageSummary(await getUsageClientId());
      const limit = summary.limits.aiProcessingMonthly;
      if (limit.kind !== 'limited') return;
      aiUsageNotice = {
        capability: 'AI processing',
        used: summary.usage.ai_suggestion ?? 0,
        limit: limit.limit,
        periodLabel: 'this month',
      };
    } catch {
      // The quota response still explains the block if a follow-up usage read fails.
    }
  }

  async function handleAiQuotaUpgrade(priceId: 'monthly' | 'yearly' = 'monthly'): Promise<void> {
    if (aiUpgradeBusy) return;
    aiUpgradeBusy = true;
    try {
      const outcome = await startBillingCheckout(priceId, { successUrl: window.location.href });
      if (outcome.status === 'login-required') loginOpen = true;
      if (outcome.status === 'failed') toast.error(outcome.message);
    } finally {
      aiUpgradeBusy = false;
    }
  }

  function handleShowYqInput() {
    structGenerationOpen = false;
    aiInputOpen = false;
    yqInputOpen = true;
    yqError = '';
    void tick().then(() => {
      void yqInputBoxRef?.focus();
    });
  }

  function handleCloseYqInput() {
    yqInputOpen = false;
    yqError = '';
  }

  function handleShowStructGeneration(): void {
    aiInputOpen = false;
    yqInputOpen = false;
    structGenerationError = '';
    structGenerationOpen = true;
  }

  function handleCloseStructGeneration(): void {
    if (structGenerationBusy) return;
    structGenerationOpen = false;
    structGenerationError = '';
  }

  // The right pane's document runtime currently accepts Treease structured languages only.
  // Keep generated code visible while using the closest lexical mode until code-only languages are supported.
  function rightEditorLanguageForStruct(language: StructLanguage): SupportedEditorLanguageId {
    return language === 'python' ? 'python' : 'javascript';
  }

  async function handleSubmitStructGeneration(): Promise<void> {
    const sourceText = getActiveDocumentText() || editorRef?.getActiveText() || '';
    const sourceLanguage = editorRef?.getActiveLanguage() ?? $languageIdStore;
    const operation = createActivePreviewOperation();
    if (!sourceText.trim()) {
      structGenerationError = 'The active document is empty.';
      return;
    }

    structGenerationBusy = true;
    structGenerationError = '';
    try {
      const sourceJson = await operation.step(() => prepareStructGenerationSource({
        text: sourceText,
        language: sourceLanguage,
        formatting: $settings.formatting,
        callWorker: callSharedWasmWorker,
      }));
      if (!sourceJson) return;
      const result = await operation.step(() => generateStruct({
        sourceJson,
        targetLanguage: structGenerationTarget,
        rootName: structGenerationRootName.trim() || 'Root',
      }));
      if (!result || !operation.isCurrent()) return;
      await showViewerTextPreview(result.code, rightEditorLanguageForStruct(result.language));
      if (!operation.isCurrent()) return;
    } catch (error) {
      if (operation.isCurrent()) structGenerationError = error instanceof Error ? error.message : 'Unable to generate the structure definition.';
    } finally {
      structGenerationBusy = false;
    }
  }

  async function handleSubmitYq(expression: string) {
    const nextExpression = expression.trim();
    yqExpression = nextExpression;
    yqBusy = true;
    yqError = '';
    const result = await runYqPreview({
      expression: nextExpression,
      text: getActiveDocumentText(),
      language: editorRef?.getActiveLanguage() ?? $languageIdStore,
      formatting: $settings.formatting,
      enableNest: $settings.parser.enableNest,
      callWorker: callSharedWasmWorker,
    });
    if ('error' in result) {
      yqError = result.error;
    } else {
      await showViewerTextPreviewForRevision(result.result, result.previewLanguage, $editorRevision);
    }
    yqBusy = false;
  }

  function collapseViewer() {
    updateSplitLayout(collapseViewerLayout);
  }

  function collapseEditor() {
    updateSplitLayout(collapseEditorLayout);
  }

  function expandSplitLayout() {
    updateSplitLayout((state) => expandSplit(state, getContainerWidth(), splitLayoutConfig));
    resetViewerTextScroll();
  }

  function updateSplitFromClientX(clientX: number, start = false) {
    const update = start
      ? splitLayoutDragController.start(splitLayoutState, clientX, splitLayoutContainer!.getBoundingClientRect())
      : splitLayoutDragController.move(splitLayoutState, clientX);
    if (!update) return;
    splitterCollapseHint = update.collapseSide === 'left'
      ? 'editor'
      : update.collapseSide === 'right'
        ? 'viewer'
        : null;
    syncSplitLayoutState(update.state);
  }

  function handleSplitterDragStart(clientX: number) {
    if (!showEditorPane || !showViewerPane || !splitLayoutContainer) return;
    isDraggingSplitter = true;
    updateSplitFromClientX(clientX, true);
  }

  function handleSplitterDragMove(clientX: number) {
    updateSplitFromClientX(clientX);
  }

  function handleSplitterDragEnd() {
    if (!isDraggingSplitter) return;
    splitLayoutDragController.end();
    isDraggingSplitter = false;
    splitterCollapseHint = null;
    syncSplitLayoutState({ ...splitLayoutState, lastSplitRatio: splitRatio });
    void settingsStore.saveEditorSplitRatio(splitRatio);
  }

  function handleSidebarToggle(expanded: boolean): void {
    sidebarExpanded = expanded;
    void settingsStore.saveSidebarExpanded(expanded);
  }

  function handleApplyDiff(plan: DiffPlan) {
    editorRef?.applyDiffPlan(plan);
  }

  async function handleSwapEditors(payload: { rightText: string; rightLanguage: SupportedEditorLanguageId }) {
    const leftText = getActiveDocumentText();
    const leftLanguage = editorRef?.getActiveLanguage() ?? $languageIdStore;
    await editorRef?.importAs(payload.rightLanguage, payload.rightText, payload.rightLanguage);
    await showViewerTextPreviewForRevision(leftText, leftLanguage, $editorRevision);
  }

  async function toggleSyncScroll() {
    const nextEnabled = !syncScrollEnabled;
    scrollSyncLock = null;
    await settingsStore.save({
      interaction: {
        enableSyncScroll: nextEnabled,
        autoSave: $settings.interaction.autoSave,
      },
    });
  }

  function syncScroll(
    owner: ScrollSyncOwner,
    blockedBy: ScrollSyncOwner,
    position: ScrollPosition,
    apply: (position: ScrollPosition) => void,
  ) {
    if (!syncScrollEnabled || graphSurfaceMode !== 'compare') return;
    if (scrollSyncLock === blockedBy) return;
    scrollSyncLock = owner;
    apply(position);
    queueMicrotask(() => {
      if (scrollSyncLock === owner) scrollSyncLock = null;
    });
  }

  function handleEditorScroll(position: ScrollPosition) {
    syncScroll('editor', 'viewer', position, (nextPosition) => viewerRef?.setTextScrollPosition?.(nextPosition));
  }

  function handleViewerScroll(position: ScrollPosition) {
    syncScroll('viewer', 'editor', position, (nextPosition) => editorRef?.setScrollPosition?.(nextPosition));
  }

  let graphRevealToken = 0;
  let activeSearchPreviewId: string | null = null;
  let navigationRuntime: ReturnType<typeof createWorkspaceNavigationRuntime> | null = null;
  let graphReadinessBinding: GraphRuntimeReadinessBinding | null = null;

  function navigationTabs() {
    const workspace = getWorkspaceState();
    return workspace.tabOrder
      .map((id) => workspace.tabsById[id])
      .filter((tab): tab is NonNullable<typeof tab> => Boolean(tab))
      .map((tab) => ({ id: tab.id, documentKey: tab.documentKey, revision: tab.revision }));
  }

  function navigationTarget(): NavigationTarget | null {
    if (workspaceBootstrapReady) ensureNavigationRuntime($editorWorkspace);
    return navigationRuntime?.target(getWorkspaceState().activeTabId) ?? null;
  }

  function syncGraphReadinessBinding(_workspaceVersion: unknown): void {
    ensureNavigationRuntime(_workspaceVersion);
    const target = navigationRuntime?.target(getWorkspaceState().activeTabId) ?? null;
    if (
      target
      && graphReadinessBinding?.target.tabId === target.tabId
      && graphReadinessBinding.target.documentKey === target.documentKey
      && graphReadinessBinding.target.generation === target.generation
      && graphReadinessBinding.target.revision === target.revision
    ) return;

    graphReadinessBinding?.dispose();
    graphReadinessBinding = target ? navigationRuntime?.bindGraphRuntime(target) ?? null : null;
  }

  function navigationResult(applied: boolean): NavigationResult {
    return applied ? { kind: 'applied' } : { kind: 'no-op' };
  }

  function ensureNavigationRuntime(_workspaceVersion: unknown): void {
    if (navigationRuntime) {
      navigationRuntime.sync(navigationTabs(), getWorkspaceState().activeTabId);
      return;
    }
    const workspace = getWorkspaceState();
    navigationRuntime = createWorkspaceNavigationRuntime('editor-workspace', navigationTabs(), workspace.activeTabId, {
      // Read the settings source at dispatch time so an immediately persisted
      // interaction preference cannot race Svelte's reactive assignment.
      completeNavigationEnabled: () => get(settings).interaction.enableSyncScroll,
      isVisible: (target) => getWorkspaceState().activeTabId === target.tabId,
      editor: {
        locate: async (context, command, options) => {
          if (!context.isCurrent()) return { kind: 'stale' };
          const revealed = await editorRef?.revealPath([...command.path], {
            target: command.cellTarget,
            focus: options.focus,
            isCurrent: context.isCurrent,
          });
          return navigationResult(revealed !== false);
        },
      },
      graph: {
        isInteractive: (target) => getWorkspaceState().activeTabId === target.tabId && viewerRef?.isGraphInteractive() === true,
        capturePreviewBaseline: async () => ({ selection: get(activeTempModel).graphHighlight, viewport: null }),
        highlight: async (context, command) => {
          if (!context.isCurrent()) return { kind: 'stale' };
          activeTempModel.update((current) => ({
            ...current,
            graphHighlight: {
              path: [...command.path], target: command.cellTarget,
              revision: Math.max($editorRevision, $graphAppliedRevision), source: command.origin === 'editor' ? 'editor' : 'graph', navigate: false,
              revealToken: ++graphRevealToken,
            },
          }));
          return { kind: 'applied' };
        },
        reveal: async (context, command) => {
          if (!context.isCurrent()) return { kind: 'stale' };
          return navigationResult(await viewerRef?.revealPath([...command.path], { target: command.cellTarget }) === true);
        },
        restoreSelection: async (context, baseline) => {
          if (!context.isCurrent()) return { kind: 'stale' };
          activeTempModel.update((current) => ({ ...current, graphHighlight: baseline.selection as typeof current.graphHighlight }));
          return { kind: 'applied' };
        },
        restoreViewport: async () => ({ kind: 'no-op' }),
        cancelViewportTransition: async () => ({ kind: 'applied' }),
      },
      navigator: {
        apply: async (command) => {
          const workspace = getWorkspaceState();
          const tab = workspace.tabsById[command.target.tabId];
          if (
            workspace.activeTabId !== command.target.tabId ||
            !tab ||
            tab.documentKey !== command.target.documentKey ||
            tab.revision !== command.target.revision
          ) return { kind: 'stale' };
          activeTempModel.update((current) => ({ ...current, treePath: [...command.path] }));
          if (!command.materializeColumns) return { kind: 'applied' };
          await viewerRef?.applyColumnNavigatorNavigationPath([...command.path]);
          return { kind: 'applied' };
        },
      },
    });
  }

  function handleEditorNavigation(path: PathSeg[], cellTarget: 'key' | 'value' | 'node'): void {
    const target = navigationTarget();
    if (!path.length || !target) return;
    void navigationRuntime?.dispatch({ kind: 'editor-selection', target, path, cellTarget });
  }
  function handleGraphReveal(payload: {
    path: PathSeg[];
    target?: 'key' | 'value' | 'node';
    trigger?: 'click' | 'search-preview' | 'search-commit' | 'breadcrumb';
  }) {
    const path = payload?.path ?? [];
    const target = navigationTarget();
    if (!path.length || !target) return;
    const cellTarget = payload.target ?? 'node';
    const kind = payload.trigger === 'breadcrumb'
      ? 'navigator-tree-path'
      : payload.trigger === 'search-preview'
        ? 'search-preview'
        : payload.trigger === 'search-commit'
          ? 'search-commit'
          : 'graph-cell';
    const previewId = `search:${serializePath(path)}`;
    if (kind === 'search-preview') activeSearchPreviewId = previewId;
    if (kind === 'search-commit') activeSearchPreviewId = null;
    const event: NavigationUserEvent = kind === 'search-preview' || kind === 'search-commit'
      ? { kind, target, path, cellTarget, previewId }
      : { kind, target, path, cellTarget };
    void navigationRuntime?.dispatch(event);
  }

  function handleTreePathSelect(path: PathSeg[]) {
    const target = navigationTarget();
    if (!path.length || !target) return;
    void navigationRuntime?.dispatch({ kind: 'navigator-tree-path', target, path, cellTarget: 'value' });
  }

  function cancelSearchPreview(): void {
    const target = navigationTarget();
    const previewId = activeSearchPreviewId;
    activeSearchPreviewId = null;
    if (target && previewId) void navigationRuntime?.dispatch({ kind: 'search-cancel', target, previewId });
    void viewerRef?.cancelGraphSearchPreview();
  }

  function handleAddTab() {
    editorRef?.addTab();
  }

  function handleCloseTab(id: string) {
    if (!workspaceCommandReady) return;
    const tab = getWorkspaceState().tabsById[id];
    if (tab && isWorkspaceTabDirty(tab) && !window.confirm(`Close ${tab.name} without saving local changes?`)) {
      return;
    }
    const stop = fileWatchUnsubscribers.get(id);
    if (stop) {
      void stop();
      fileWatchUnsubscribers.delete(id);
    }
    editorRef?.closeTab(id);
  }

  function handleActivateTab(id: string) {
    if (!workspaceCommandReady) return;
    editorRef?.activateTab(id);
  }

  function handleRenameTab(id: string, name: string) {
    if (!workspaceCommandReady) return;
    editorRef?.renameDocument(id, name);
  }

  function sessionFromWorkspace(): WorkspaceSession {
    return workspaceSessionFromWorkspace(getWorkspaceState());
  }

  function scheduleWorkspaceSessionSave(): void {
    if (sessionRestoring) return;
    if (sessionSaveTimer) clearTimeout(sessionSaveTimer);
    sessionSaveTimer = setTimeout(() => {
      void (async () => {
        const host = await workspaceHost;
        await host.saveSession(sessionFromWorkspace());
      })();
    }, 300);
  }

  function enableWorkspaceSessionPersistence(): void {
    if (stopWorkspaceSession) return;
    let initialProjection = true;
    stopWorkspaceSession = editorWorkspace.subscribe(() => {
      if (initialProjection) {
        initialProjection = false;
        return;
      }
      scheduleWorkspaceSessionSave();
    });
    // Persist the latest projection once after attaching. Direct typing may
    // continue while the first promote save is in flight.
    scheduleWorkspaceSessionSave();
  }

  function observeSharedDraftMutation(target: SharedWorkspaceMutationTarget): void {
    sharedWorkspaceLifecycle?.observeDirectDraftMutation(target);
  }

  function ensureSharedWorkspacePromoted(target: SharedWorkspaceMutationTarget): Promise<boolean> {
    return sharedWorkspaceLifecycle?.ensurePromoted(target) ?? Promise.resolve(true);
  }

  function clearShareUrlAfterPromotion(): void {
    const url = new URL(window.location.href);
    url.searchParams.delete('shareID');
    window.history.replaceState(window.history.state, '', `${url.pathname}${url.search}${url.hash}`);
  }

  /**
   * Session data is host-owned input, not a sequence of interactive editor
   * commands. Build its complete left-tab topology before Monaco mounts so
   * the first model, authority state, and header all start on one document.
   */
  function bootstrapWorkspaceSession(session: WorkspaceSession): void {
    if (!session.tabs.length) return;
    const current = getWorkspaceState();
    const currentPrimary = current.tabsById[current.primaryTabId];
    if (!currentPrimary) throw new Error('Workspace bootstrap requires a primary tab.');

    const [first, ...remaining] = session.tabs;
    const firstLanguage = languageForFileName(`recovery.${first.languageId}`);
    const firstOrigin = first.origin ?? (first.sourceText === getLanguageExample(firstLanguage) ? 'example' : 'user');
    let workspace = createEditorWorkspaceState({
      ...currentPrimary,
      id: 'session-tab-0',
      role: 'primary',
      name: first.name,
      documentKey: 'session-tab-0:0',
      languageId: firstLanguage,
      sourceText: first.sourceText,
      origin: firstOrigin,
      revision: 0,
      graphAppliedRevision: 0,
      snapshotId: null,
      savedText: first.savedText,
      fileLinkedDocument: undefined,
    });

    for (const [index, tab] of remaining.entries()) {
      const id = `session-tab-${index + 1}`;
      const languageId = languageForFileName(`recovery.${tab.languageId}`);
      const transition = createWorkspaceTabTransition(workspace, {
        id,
        name: tab.name,
        documentKey: `${id}:0`,
        languageId,
        sourceText: tab.sourceText,
        origin: tab.origin ?? 'user',
        savedText: tab.savedText,
      });
      if (!transition) throw new Error(`Invalid recovered tab at index ${index + 1}.`);
      workspace = transition.workspace;
    }

    const activeIndex = Math.max(0, Math.min(session.activeTabIndex, workspace.tabOrder.length - 1));
    const activeTabId = workspace.tabOrder[activeIndex];
    const activeTransition = activeTabId ? activateWorkspaceTabTransition(workspace, activeTabId) : null;
    if (!activeTransition) throw new Error('Recovered workspace has no active left tab.');
    setWorkspaceState(activeTransition.workspace);
  }

  /**
   * Establish the product's fresh-workspace document before Monaco mounts.
   * Restored sessions and URL presets are applied as explicit later transitions;
   * an empty source is therefore never used as an implicit "first tab" signal.
   */
  function bootstrapFreshWorkspace(): void {
    const current = getWorkspaceState();
    const currentPrimary = current.tabsById[current.primaryTabId];
    if (!currentPrimary) throw new Error('Fresh workspace requires a primary tab.');
    const sourceText = getLanguageExample(editorLanguageFallback);
    setWorkspaceState(
      createEditorWorkspaceState({
        ...currentPrimary,
        id: 'primary',
        role: 'primary',
        name: 'Untitled',
        documentKey: 'primary:0',
        languageId: editorLanguageFallback,
        sourceText,
        origin: 'example',
        revision: 0,
        graphAppliedRevision: 0,
        snapshotId: null,
        tempModel: { ...initialTempModel, scratchText: sourceText },
        fullEditUiState: initialFullEditUiState,
        fileLinkedDocument: undefined,
        savedText: undefined,
      }),
    );
  }

  async function handleDesktopDeepLinks(urls: URL[]): Promise<void> {
    for (const url of urls) {
      if (url.hostname === 'editor') {
        const nextPreset = resolveEditorUrlPreset(url.search);
        const allowedParameters = new Set([
          'ui', 'lang', 'text', 'textUrl', 'rightText', 'rightTextUrl', 'command', 'nest', 'autoFormat', 'yq',
        ]);
        const hasUnknownParameter = [...url.searchParams.keys()].some((key) => !allowedParameters.has(key));
        if (hasUnknownParameter || nextPreset.telemetry.ignored.length > 0) {
          toast.error('Desktop editor link contains unsupported parameters.');
          continue;
        }
        urlPreset = nextPreset;
        await applyEditorUrlPreset(nextPreset);
        continue;
      }
      if (url.hostname === 'auth' && url.pathname === '/callback') {
        const code = url.searchParams.get('code');
        if (!code) {
          toast.error('Login callback did not include an authorization code.');
          continue;
        }
        const session = await exchangeAuthCode(code);
        if (!session?.refresh_token) throw new Error('Login did not return a refresh token.');
        await (await workspaceHost).storeRefreshToken(session.refresh_token);
        loginOpen = false;
        toast.success('You are now logged in.');
      }
    }
  }

  async function handleLogout(): Promise<void> {
    try {
      await signOut();
      toast.success('You are now logged out.');
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(`Logout failed: ${message}`);
    }
  }

  async function handleCheckForUpdates(): Promise<void> {
    const host = await workspaceHost;
    if (host.surface !== 'desktop') return;
    const update = await host.checkForUpdate();
    if (!update) {
      toast.info('Treease is up to date.');
      return;
    }
    toast.info(`Treease ${update.version} is ready to download.`, {
      action: { label: 'Download and restart', onClick: () => void host.installCheckedUpdate() },
    });
  }

  async function handleCommandExecute(id: CommandId): Promise<void> {
    const handlers: Record<CommandId, () => void | Promise<void>> = {
      'workspace:new': () => workspaceCommands['workspace:new'](),
      'workspace:open': () => workspaceCommands['workspace:open'](),
      'workspace:save': () => workspaceCommands['workspace:save'](),
      'workspace:save-as': () => workspaceCommands['workspace:save-as'](),
      'workspace:close-tab': () => workspaceCommands['workspace:close-tab'](),
      format: () => editorRef?.formatActive(),
      minify: () => editorRef?.minifyActive(),
      compact: () => editorRef?.compactActive(),
      sort: () => editorRef?.sortActive(),
      'show-yq-input': () => handleShowYqInput(),
      'generate-struct': () => handleShowStructGeneration(),
      escape: () => editorRef?.escapeActive(),
      unescape: () => editorRef?.unescapeActive(),
    };
    await handlers[id]?.();
  }

  type StaticWorkspaceCommand = Exclude<WorkspaceCommand, `workspace:open-recent:${string}`>;

  const workspaceCommands: Record<StaticWorkspaceCommand, () => void | Promise<void>> = {
    'workspace:new': () => workspaceCommandReady && handleAddTab(),
    'workspace:open': () => workspaceCommandReady && handleOpenDocument(),
    'workspace:save': () => saveActiveDocument(),
    'workspace:save-as': () => saveActiveDocument(true),
    'workspace:import': () => sidebarRef?.openImportPanel(),
    'workspace:export': () => sidebarRef?.openExportPanel(),
    'workspace:clear-recent': handleClearRecentFiles,
    'workspace:close-tab': () => workspaceCommandReady && activeTabId && handleCloseTab(activeTabId),
    'workspace:toggle-viewer': () => { showViewerPane = !showViewerPane; },
    'workspace:help': () => void (async () => (await workspaceHost).openExternal(new URL('https://treease.io')) )(),
  };

  function isOpenRecentCommand(command: WorkspaceCommand): command is `workspace:open-recent:${string}` {
    return command.startsWith('workspace:open-recent:');
  }

  async function handleWorkspaceCommand(command: WorkspaceCommand): Promise<void> {
    try {
      if (isOpenRecentCommand(command)) {
        const recentId = command.slice('workspace:open-recent:'.length);
        await handleOpenRecentFile({ id: recentId, name: '' });
        return;
      }
      await workspaceCommands[command]();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(`Desktop command failed: ${message}`);
    }
  }

  $: {
    const nextSplitLayoutState = syncSplitRatio(splitLayoutState, containerWidth, splitLayoutConfig);
    if (
      nextSplitLayoutState.layoutMode !== splitLayoutState.layoutMode ||
      nextSplitLayoutState.splitRatio !== splitLayoutState.splitRatio ||
      nextSplitLayoutState.lastSplitRatio !== splitLayoutState.lastSplitRatio
    ) {
      syncSplitLayoutState(nextSplitLayoutState);
    }
  }
  $: visibleLayoutMode = !showEditorPane ? 'right-only' : !showViewerPane ? 'left-only' : layoutMode;
  $: renderLayoutControls = showEditorPane && showViewerPane && visibleLayoutMode !== 'split';
  $: visibleSplitLayoutState = { ...splitLayoutState, layoutMode: visibleLayoutMode };
  $: ({ leftPaneWidthPx, rightPaneWidthPx, splitterLeftPx, splitterControlLeftPx } = computePaneWidths(
    visibleSplitLayoutState,
    containerWidth,
    splitLayoutConfig,
  ));
  $: ({ leftPaneCollapsed, rightPaneCollapsed, collapsedControlFlyX } = resolveSplitLayoutMotion(visibleLayoutMode));
  $: topBarVisible = showTopBar && workspaceCommandReady;
  $: documentInvalid = Boolean($activeTempModel?.error) || ($activeTempModel?.diagnostics?.length ?? 0) > 0;
  $: tabSummaries = summarizeWorkspaceTabs($editorWorkspace);
  $: activeTabId = $editorWorkspace.activeTabId;
  $: if (workspaceBootstrapReady) ensureNavigationRuntime($editorWorkspace);
  $: if (workspaceBootstrapReady && viewerRef) syncGraphReadinessBinding($editorWorkspace);
  $: if (!viewerRef && graphReadinessBinding) {
    graphReadinessBinding.dispose();
    graphReadinessBinding = null;
  }

  onMount(() => {
    const resetRequested = isEditorResetRequested(window.location.search);
    if (resetRequested) {
      void (async () => {
        try {
          await settingsStore.closePersistence();
          const host = await workspaceHost;
          if (host.surface === 'desktop') await resetBrowserLocalState();
          await host.resetLocalState();
          await settingsStore.reset();
          syncSplitLayoutState(createSplitLayoutState(DEFAULT_EDITOR_SPLIT_RATIO));
          layoutReady = true;
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error);
          layoutReady = true;
          console.error('[editor] failed to reset local application state', { error: message });
          toast.error(`Editor reset failed: ${message}`);
        }
      })();
      return;
    }

    urlPreset ??= resolveEditorUrlPreset(window.location.search);
    const shareID = urlPreset.shareID;
    bootstrapFreshWorkspace();
    let stopWorkspaceCommands: (() => void) | null = null;
    let stopDeepLinks: (() => void) | null = null;
    void (async () => {
      const host = await workspaceHost;
      if (shareID.present) {
        sharedWorkspaceLifecycle = createSharedWorkspaceLifecycle({
          loadSession: () => host.loadSession(),
          saveSession: (session) => host.saveSession(session),
          getWorkspace: getWorkspaceState,
          publishWorkspace: setWorkspaceState,
          onTopologyPublished: () => {
            clearShareUrlAfterPromotion();
            workspaceCommandReady = true;
          },
          enableSessionPersistence: enableWorkspaceSessionPersistence,
          reportError: (message) => toast.error(message),
        });
        sharedWorkspaceLifecycle.beginRestore();
        workspaceBootstrapReady = true;
        resolveWorkspaceBootstrap?.();
        resolveWorkspaceBootstrap = null;
        return;
      }
      const session = await host.loadSession();
      sessionRestoring = true;
      try {
        if (session) bootstrapWorkspaceSession(session);
      } finally {
        sessionRestoring = false;
      }
      workspaceBootstrapReady = true;
      await tick();
      await editorRef?.ensureReady();
      resolveWorkspaceBootstrap?.();
      resolveWorkspaceBootstrap = null;
      if (host.surface === 'desktop') {
        for (const file of await host.takeStartupFiles()) await openWorkspaceFile(file);
        await handleDesktopDeepLinks(await host.getInitialDeepLinks());
        stopWorkspaceCommands = await host.onCommand((command) => void handleWorkspaceCommand(command));
        stopDeepLinks = await host.onDeepLinks((urls) => void handleDesktopDeepLinks(urls).catch((error) => {
          const message = error instanceof Error ? error.message : String(error);
          toast.error(`Desktop deep link failed: ${message}`);
        }));
      }
      enableWorkspaceSessionPersistence();
      scheduleWorkspaceSessionSave();
      workspaceCommandReady = true;
    })().catch((error) => {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(`Desktop workspace initialization failed: ${message}`);
    });
    void (async () => {
      await settingsStore.load();
      const savedSplitRatio = settingsStore.getEditorSplitRatio();
      if (serverSplitRatio === null && savedSplitRatio !== null) {
        syncSplitLayoutState(createSplitLayoutState(savedSplitRatio));
        // IndexedDB is the legacy source only until this bootstrap value reaches the SSR cookie.
        void settingsStore.saveEditorSplitRatio(savedSplitRatio);
      }
      const savedSidebarExpanded = settingsStore.getSidebarExpanded();
      if (serverSidebarExpanded === null && savedSidebarExpanded !== null) {
        sidebarExpanded = savedSidebarExpanded;
        // IndexedDB is the legacy source only until this bootstrap value reaches the SSR cookie.
        void settingsStore.saveSidebarExpanded(savedSidebarExpanded);
      }
      layoutReady = true;
      await tick();
      if (shareID.present) {
        if (shareID.valid && shareID.value) await restoreShare(shareID.value);
      } else {
        await applyEditorUrlPreset(urlPreset);
      }
    })().catch((error) => {
        const message = error instanceof Error ? error.message : String(error);
        console.error('[editor] failed to initialize settings and layout', { error: message });
        layoutReady = true;
        toast.error(`Editor initialization failed: ${message}`);
      });
    const handleResize = () => {
      syncSplitLayoutState(syncSplitRatio(splitLayoutState, getContainerWidth(), splitLayoutConfig));
    };
    window.addEventListener('resize', handleResize);
    let stopDroppedFiles: (() => void) | null = null;
    void (async () => {
      stopDroppedFiles = await (await workspaceHost).onFilesDropped((files) => {
        for (const file of files) void openWorkspaceFile(file);
      });
    })();
    const saveOnFocusChange = () => {
      if (autoSaveMode === 'onFocusChange') void saveActiveDocument(false, true);
    };
    const saveOnWindowChange = () => {
      if (document.visibilityState === 'hidden' && autoSaveMode === 'onWindowChange') void saveActiveDocument(false, true);
    };
    window.addEventListener('blur', saveOnFocusChange);
    document.addEventListener('visibilitychange', saveOnWindowChange);
    return () => {
      window.removeEventListener('resize', handleResize);
      stopDroppedFiles?.();
      stopWorkspaceSession?.();
      stopWorkspaceSession = null;
      sharedWorkspaceLifecycle?.dispose();
      sharedWorkspaceLifecycle = null;
      stopWorkspaceCommands?.();
      stopDeepLinks?.();
      window.removeEventListener('blur', saveOnFocusChange);
      document.removeEventListener('visibilitychange', saveOnWindowChange);
      if (autoSaveTimer) clearTimeout(autoSaveTimer);
      if (sessionSaveTimer) clearTimeout(sessionSaveTimer);
    };
  });
</script>
<svelte:head>
  <title>Treease Editor | Structured Text Workspace</title>
  <meta name="robots" content="noindex,follow" />
  <link rel="preload" as="fetch" href={wasmUrl} crossorigin="anonymous" />
</svelte:head>

<main class="grid h-screen w-screen bg-[var(--app-bg)] text-[var(--text-primary)]">
  <h1 class="sr-only">Treease editor</h1>
  {#if shareLoadError}
    <section class="m-auto max-w-md rounded-[18px] border border-[var(--border-muted)] bg-white p-8 text-center shadow-sm" data-testid="share-load-error" role="alert" aria-live="assertive">
      <h1 class="text-xl font-semibold">Unable to open share link</h1>
      <p class="mt-2 text-sm text-[var(--text-muted)]">{shareLoadError}</p>
      <a class="mt-6 inline-flex rounded-[9px] bg-[var(--accent)] px-4 py-2 text-sm text-white" href="/editor" data-sveltekit-reload>Open a blank editor</a>
    </section>
  {:else}
  <div class="flex h-full min-h-0 min-w-0 overflow-hidden">
    <Sidebar
      bind:this={sidebarRef}
      expanded={sidebarExpanded}
      onToggleSidebar={handleSidebarToggle}
      {formatOptions}
      onRequestImportFile={handleRequestImportFile}
      onImportFileStream={handleImportFileStream}
      onExportPreview={handleExportPreview}
      onExportDownload={handleExportDownload}
      bind:feedbackOpen
      bind:shareOpen
      bind:settingsOpen
      createShareResource={createShareResource}
      onLogin={() => (loginOpen = true)}
      onLogout={handleLogout}
      onCheckForUpdates={handleCheckForUpdates}
    />
    <div class="relative h-full min-h-0 min-w-0 flex-1 overflow-hidden">
      <div bind:this={splitLayoutContainer} bind:clientWidth={containerWidth} class="app-split-layout">
      {#if showEditorPane}
        <section
          class="app-split-pane app-split-pane--left flex flex-col bg-[var(--panel-bg)]"
          class:app-split-pane--collapsed={leftPaneCollapsed}
          class:app-split-pane--instant={isDraggingSplitter || !layoutReady}
          data-testid="left-pane"
          aria-hidden={leftPaneCollapsed}
          style:width={formatVisiblePaneWidth(visibleLayoutMode === 'right-only' ? 0 : visibleLayoutMode === 'left-only' ? 1 : splitRatio, leftPaneWidthPx)}
          style:opacity={leftPaneCollapsed ? 0 : 1}
        >
          {#if splitterCollapseHint === 'editor'}
            <SplitLayoutCollapseHint side="left" />
          {/if}
          <FunctionBar
            aiInputOpen={aiInputOpen}
            onShowAiInputPanel={handleShowAiInputPanel}
            onFormat={() => editorRef?.formatActive()}
            onMinify={() => editorRef?.minifyActive()}
            onCommandExecute={handleCommandExecute}
          />
          <div class="min-h-0 flex-1">
            {#if workspaceBootstrapReady}
              <Editor
                bind:this={editorRef}
                {synchronizedRuntimeLoading}
                onDirectDraftMutation={observeSharedDraftMutation}
                {ensureSharedWorkspacePromoted}
                onRequestImportFile={handleRequestImportFile}
                onNavigation={handleEditorNavigation}
                on:runtime-state={handleEditorRuntimeEvent}
                onScroll={handleEditorScroll}
              />
            {/if}
          </div>
          <div class="auxiliary-input-container" data-testid="auxiliary-input-container">
            {#if aiInputOpen}
              <AiInputPanel
              value={aiInstruction}
              busy={aiBusy}
              error={aiError}
              success={aiSuccess}
              quotaExhausted={aiQuotaExhausted}
              upgradeBusy={aiUpgradeBusy}
              onChange={(value) => {
                aiInstruction = value;
                aiError = '';
                aiSuccess = '';
                aiQuotaExhausted = false;
              }}
              onSubmit={handleSubmitAi}
              onUpgrade={() => void openPricingOverlay()}
              onClose={handleCloseAiInputPanel}
              />
            {:else if yqInputOpen}
              <YqInputBox
              bind:this={yqInputBoxRef}
              value={yqExpression}
              busy={yqBusy}
              error={yqError}
              onChange={(value) => {
                yqExpression = value;
                yqError = '';
              }}
              onSubmit={handleSubmitYq}
              onClose={handleCloseYqInput}
              />
            {:else if structGenerationOpen}
              <StructGenerationInput
              targetLanguage={structGenerationTarget}
              rootName={structGenerationRootName}
              busy={structGenerationBusy}
              error={structGenerationError}
              onChangeTargetLanguage={(value) => (structGenerationTarget = value)}
              onChangeRootName={(value) => (structGenerationRootName = value)}
              onSubmit={handleSubmitStructGeneration}
              onClose={handleCloseStructGeneration}
              />
            {/if}
            <div class="editor-status" data-testid="editor-status">
              <span>{$activeTempModel?.cursor ?? 'Ln 1, Col 1'}{#if ($activeTempModel?.selectionLength ?? 0) > 0} ({$activeTempModel?.selectionLength} selected){/if}</span>
              <Tooltip content={documentInvalid ? 'Document has errors' : 'Document is valid'} side="top"><button type="button" class:editor-status--invalid={documentInvalid} disabled={!documentInvalid} on:click={() => { const diagnostic = $activeTempModel?.diagnostics?.[0]; if (diagnostic) editorRef?.revealError(diagnostic.startLineNumber, diagnostic.startColumn) }}>{#if documentInvalid}<CircleAlert size={13} />Invalid{:else}<Check size={13} />Valid{/if}</button></Tooltip>
            </div>
          </div>
          <TabSwitcher
            placement="bottom"
            tabs={tabSummaries}
            activeTabId={activeTabId}
            showTabDirty={true}
            canAddTab={tabSummaries.length < maxTabs}
            onAddTab={workspaceCommands['workspace:new']}
            onActivateTab={handleActivateTab}
            onRenameTab={handleRenameTab}
            onCloseTab={(id) => handleCloseTab(id)}
            showCommandSearch={false}
          />
        </section>
      {/if}

      {#if showEditorPane && showViewerPane}
        <div
          class={`app-split-divider app-split-divider--vertical ${isDraggingSplitter ? 'app-split-divider--dragging' : ''} ${visibleLayoutMode !== 'split' ? 'app-split-divider--collapsed' : ''} ${visibleLayoutMode === 'left-only' ? 'app-split-divider--right-edge' : ''}`}
          data-testid="splitter-divider"
          role="separator"
          aria-label="Resize panels"
          aria-orientation="vertical"
          style:left={`${splitterLeftPx}px`}
          use:splitLayoutDrag={{
            onDragStart: ({ clientX }) => handleSplitterDragStart(clientX),
            onDragMove: ({ clientX }) => handleSplitterDragMove(clientX),
            onDragEnd: () => handleSplitterDragEnd(),
          }}
        ></div>
      {/if}

      {#if showViewerPane}
        <section
          class="app-split-pane app-split-pane--right flex flex-col bg-[var(--panel-bg-alt)]"
          class:app-split-pane--split-right={visibleLayoutMode === 'split'}
          class:app-split-pane--collapsed={rightPaneCollapsed}
          class:app-split-pane--instant={isDraggingSplitter || !layoutReady}
          data-testid="right-pane"
          aria-hidden={rightPaneCollapsed}
          style:width={formatVisiblePaneWidth(visibleLayoutMode === 'left-only' ? 0 : visibleLayoutMode === 'right-only' ? 1 : 1 - splitRatio, rightPaneWidthPx)}
          style:opacity={rightPaneCollapsed ? 0 : 1}
        >
          {#if topBarVisible}
            <div class="graph-workspace-toolbar" data-testid="graph-workspace-toolbar">
              <div class="graph-surface-switcher" data-testid="graph-surface-switcher" role="tablist" aria-label="Graph surface">
                <button
                  type="button"
                  class:active={graphSurfaceMode === 'graph'}
                  role="tab"
                  aria-selected={graphSurfaceMode === 'graph'}
                  data-testid="graph-surface-graph"
                  on:click={() => void selectGraphSurfaceMode('graph')}
                  ><GitGraph size={12} />Graph</button>
                <button
                  type="button"
                  class:active={graphSurfaceMode === 'compare'}
                  role="tab"
                  aria-selected={graphSurfaceMode === 'compare'}
                  data-testid="graph-surface-compare"
                  on:click={() => void selectGraphSurfaceMode('compare')}
                  ><GitCompareArrows size={12} />Compare</button>
              </div>
              <GraphTopBar
                showGlobal={false}
                viewMode={viewerViewMode}
                surfaceMode={graphSurfaceMode}
                documentKey={$documentKeyStore}
                language={$languageIdStore}
                text={$sourceTextStore}
                isGraphInteractive={() => viewerRef?.isGraphInteractive() === true}
                onSearchSelect={(event) => viewerRef?.revealGraphSearchResult(event.detail)}
                onSearchPreview={(result) => viewerRef?.previewGraphSearchResult(result)}
                onSearchCancel={cancelSearchPreview}
                onOpenCompareFile={() => viewerRef?.openCompareFile()}
                onSwapEditors={() => viewerRef?.swapCompareEditors()}
                onCompare={() => void viewerRef?.compareEditors()}
                onZoomIn={() => viewerRef?.zoomGraphIn()}
                onZoomOut={() => viewerRef?.zoomGraphOut()}
                onExportImage={() => void viewerRef?.exportGraphImage()}
                onShare={() => (shareOpen = true)}
                onLogin={() => (loginOpen = true)}
                onLogout={handleLogout}
                onCheckForUpdates={handleCheckForUpdates}
                onOpenSettings={() => (settingsOpen = true)}
              />
            </div>
          {/if}
          {#if splitterCollapseHint === 'viewer'}
            <SplitLayoutCollapseHint side="right" />
          {/if}
          <div class="min-h-0 flex-1">
          <ViewportPanel
            bind:this={viewerRef}
            bind:viewMode={viewerViewMode}
            {synchronizedRuntimeLoading}
            onRevealError={(line, column) => editorRef?.revealError(line, column)}
            onGraphNavigation={handleGraphReveal}
            onGraphRuntimeState={handleViewerRuntimeState}
            onColumnNavigatorState={handleColumnNavigatorState}
            onTextScroll={handleViewerScroll}
            onApplyDiff={handleApplyDiff}
            onSwap={handleSwapEditors}
            onFileDrop={(event) => editorRef?.handleFileDrop(event)}
            onRequestImportFile={handleRequestImportFile}
            onLoadExample={(example, language) => {
              void editorRef?.replaceActiveFromFile({ text: example, languageId: language, origin: 'example' })
            }}
            {ensureSharedWorkspacePromoted}
            {pricingPlanGridComponent}
            pricingUsageNotice={null}
            onPricingSelectPlan={(priceId) => void handleAiQuotaUpgrade(priceId)}
            pricingActionDisabled={() => aiUpgradeBusy}
            pricingActionLabel={(plan) => aiUpgradeBusy ? 'Opening checkout…' : plan.ctaLabel}
            onEntitlementBlocked={handleEntitlementBlocked}
            hideGraphToolbar={topBarVisible}
          />
          </div>
          {#if graphSurfaceMode === 'graph'}
            <div class="graph-pane__bottom" data-testid="graph-bottom-surfaces">
              <TreePathBar
                value={$activeTempModel?.treePath ?? []}
                on:select={(event) => handleTreePathSelect(event.detail)}
              />
              <ColumnNavigatorControls
                state={columnNavigatorState}
                onBack={() => viewerRef?.goColumnNavigatorBack()}
                onForward={() => viewerRef?.goColumnNavigatorForward()}
                onCollapse={() => viewerRef?.collapseColumnNavigator()}
                onPinCollapsed={() => viewerRef?.pinColumnNavigatorCollapsed()}
                onExpand={() => viewerRef?.expandColumnNavigator()}
              />
            </div>
          {/if}
        </section>
      {/if}

      {#if renderLayoutControls}
        <div transition:fly={{ x: collapsedControlFlyX, duration: 150, opacity: 0.08, easing: cubicOut }}>
          <SplitLayoutCollapsedControl
            mode={visibleLayoutMode}
            leftPx={splitterControlLeftPx}
            expandLeftLabel="Expand editor"
            expandRightLabel="Expand viewer"
            onExpand={expandSplitLayout}
          />
        </div>
      {/if}
      {#if !layoutReady}
        <section class="editor-page-loading-skeleton" aria-label="Loading editor layout" aria-busy="true">
          <div class="editor-page-loading-skeleton__line editor-page-loading-skeleton__line--wide"></div>
          <div class="editor-page-loading-skeleton__line"></div>
          <div class="editor-page-loading-skeleton__line editor-page-loading-skeleton__line--short"></div>
        </section>
      {/if}
    </div>
  </div>
  </div>
  {#if shareLoading}
    <div class="fixed inset-0 z-50 grid place-items-center bg-[var(--app-bg)]" data-testid="share-loading" aria-live="polite">
      <span class="text-sm text-[var(--text-muted)]">Restoring shared content…</span>
    </div>
  {/if}
  {/if}
</main>
{#if externalFileConflict}
  <div class="fixed inset-0 z-50 grid place-items-center bg-slate-950/30 p-6" role="presentation">
    <div class="w-full max-w-lg rounded-xl border border-[var(--border-strong)] bg-white p-5 shadow-xl" role="dialog" aria-modal="true" aria-labelledby="external-file-conflict-title">
      <h2 id="external-file-conflict-title" class="text-base font-semibold">External file change</h2>
      <p class="mt-2 text-sm text-[var(--text-muted)]">{externalFileConflict.name} changed outside Treease while this tab has unsaved edits.</p>
      <div class="mt-5 flex flex-wrap justify-end gap-2">
        <button class="rounded-md border px-3 py-1.5 text-sm" on:click={() => void compareExternalFileChange()}>Compare</button>
        <button class="rounded-md border px-3 py-1.5 text-sm" on:click={discardLocalFileChange}>Discard local and reload</button>
        <button class="rounded-md bg-[var(--accent)] px-3 py-1.5 text-sm text-white" on:click={() => void overwriteExternalFileChange()}>Overwrite file</button>
      </div>
    </div>
  </div>
{/if}
{#if !showEditorPane && workspaceBootstrapReady}
  <div class="pointer-events-none absolute -left-[10000px] top-0 h-px w-px overflow-hidden opacity-0" aria-hidden="true">
    <Editor
      bind:this={editorRef}
      {synchronizedRuntimeLoading}
      onDirectDraftMutation={observeSharedDraftMutation}
      {ensureSharedWorkspacePromoted}
      onRequestImportFile={handleRequestImportFile}
      onNavigation={handleEditorNavigation}
      on:runtime-state={handleEditorRuntimeEvent}
      onScroll={handleEditorScroll}
    />
  </div>
{/if}
<LoginDialog bind:open={loginOpen} />
