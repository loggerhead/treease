<!-- Responsibility: assemble Editor/Viewport/TopBar/BottomBar, coordinate cross-component events, and handle DOM interaction. -->
<script lang="ts">
  import { get } from 'svelte/store';
  import { onMount, tick } from 'svelte';
  import { cubicOut } from 'svelte/easing';
  import { fly } from 'svelte/transition';
  import Editor from '../../lib/components/Editor.svelte';
  import TopBar from '../../lib/components/TopBar.svelte';
  import BottomBar from '../../lib/components/BottomBar.svelte';
  import ViewportPanel from '../../lib/components/ViewportPanel.svelte';
  import SettingsDialog from '../../lib/components/SettingsDialog.svelte';
  import ShareDialog from '../../lib/components/ShareDialog.svelte';
  import FeedbackDialog from '../../lib/components/FeedbackDialog.svelte';
  import StructGenerationInput from '../../lib/components/StructGenerationInput.svelte';
  import LoginDialog from '../../lib/components/LoginDialog.svelte';
  import AiInput from '../../lib/components/AiInput.svelte';
  import PricingPlanGrid, { type PricingUsageNotice } from '../../lib/components/PricingPlanGrid.svelte';
  import YqExpressionInput from '../../lib/components/YqExpressionInput.svelte';
  import { Dialog, DialogContent } from '../../lib/components/ui/dialog';
  import { settings, settingsStore } from '../../lib/settings/settings-store';
  import { DEFAULT_EDITOR_SPLIT_RATIO } from '../../lib/settings/editor-layout-state';
  import {
    activeTempModel,
    type GraphHighlightTarget,
    type TreeSelectionSource,
  } from '../../lib/store/graph-selection-store';
  import {
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
    resolveEditorUrlPreset,
    summarizeEditorUrlPresetWarnings,
    type EditorUrlActionCommandId,
    type ResolvedEditorUrlPreset,
  } from './url-preset';
  import { resolveExportDownloadDetails, resolveExportPreviewDetails } from './export-controller';
  import {
    clamp,
    collapseEditor as collapseEditorLayout,
    collapseViewer as collapseViewerLayout,
    computePaneWidths,
    createSplitLayoutState,
    expandSplit,
    getClampedSplitRatio as getClampedSplitRatioValue,
    syncSplitRatio,
    type SplitLayoutConfig,
  } from './split-layout-controller';
  import { resolveSplitLayoutMotion } from './split-layout-motion';
  import { splitLayoutDrag } from '../../lib/components/ui/split-layout';
  import { importFormatOptions, supportedEditorLanguageSet, editorLanguageFallback, findSupportedLanguageByExtension, type SupportedEditorLanguageId } from '../../lib/monaco/language-support';
  import { computeSynchronizedRuntimeLoading, type RuntimeStateEventDetail } from '../../lib/runtime-loading';
  import { getActiveDocumentText } from '../../lib/store/active-document-authority';
  import { breadcrumbTargetForPath, isPathSegIndex, pathSegKeyValue, type PathSeg } from '../../lib/store/tree-path';
  import { PathSegTag } from '@core-wasm/index';
  import { markPreviewCompleted, markPreviewRequested } from '../../lib/test-bridge/runtime-readiness';
  import { setTreeaseUrlPresetState } from '../../lib/test-bridge/window-treease';
  import type { DiffPlan } from '../../lib/graph/diff-plan';
  import { serializePath } from '../../shared/document-anchor-utils';
  import {
    FileCode,
    GitGraph,
    Link2,
    Link2Off,
    PanelLeftClose,
    PanelLeftOpen,
    PanelRightClose,
    PanelRightOpen,
  } from 'lucide-svelte';
  import * as ButtonGroup from '../../lib/components/ui/button-group';
  import { IconButton } from '../../lib/components/ui/button';
  import { trackEvent } from '../../lib/analytics/ga4';
  import { startBillingCheckout } from '../../lib/billing/checkout-flow';
  import { getUsageClientId } from '../../lib/billing/client-id';
  import { runPostpaidCapability } from '../../lib/billing/entitlement-gate';
  import { workspaceHost } from '../../lib/workspace-host';
  import { exchangeAuthCode, signOut } from '../../lib/auth/supabase-auth';
  import { editorWorkspace, getWorkspaceState, updateWorkspaceTab } from '../../lib/store/workspace-store';
  import { isWorkspaceTabDirty, type EditorWorkspaceTabSummary } from '../../lib/store/editor-workspace';
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
  import type { WorkspaceCommand, WorkspaceSession } from '../../lib/workspace-host';

  const LARGE_FILE_PROCESSING_THRESHOLD_BYTES = 256 * 1024;

  let editorRef: Editor | null = null;
  let topBarRef: TopBar | null = null;
  let viewerRef: ViewportPanel | null = null;
  let yqInputRef: YqExpressionInput | null = null;
  let splitLayoutContainer: HTMLDivElement | null = null;
  let containerWidth = 0;
  let tabSummaries: EditorWorkspaceTabSummary[] = [];
  let showTabDirty = false;
  let activeTabId = '';
  let scrollSyncLock: 'editor' | 'viewer' | null = null;
  let splitLayoutState = createSplitLayoutState(DEFAULT_EDITOR_SPLIT_RATIO);
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
  let splitterDragRect: DOMRect | null = null;
  let settingsOpen = false;
  let shareOpen = false;
  let feedbackOpen = false;
  let structGenerationOpen = false;
  let structGenerationTarget: StructLanguage = 'typescript';
  let structGenerationRootName = 'Root';
  let structGenerationBusy = false;
  let structGenerationError = '';
  let loginOpen = false;
  let viewerViewMode: 'graph' | 'text' = 'graph';
  let editorRuntimeLoading = true;
  let viewerRuntimeLoading = true;
  let synchronizedRuntimeLoading = true;
  let syncScrollEnabled = true;
  let aiInputOpen = false;
  let aiInstruction = '';
  let aiBusy = false;
  let aiError = '';
  let aiSuccess = '';
  let aiQuotaExhausted = false;
  let aiUpgradeBusy = false;
  let pricingOpen = false;
  let aiUsageNotice: PricingUsageNotice | null = null;
  const aiPricingPlanIds = ['pro-monthly', 'pro-yearly'];
  const aiPricingDialogMaxWidth = aiPricingPlanIds.length === 1 ? '620px' : aiPricingPlanIds.length === 2 ? '1040px' : '1440px';
  let yqInputOpen = false;
  let yqExpression = '';
  let yqBusy = false;
  let yqError = '';
  let previewRequestId = 0;
  let showEditorPane = true;
  let showViewerPane = true;
  let showTopBar = true;
  let showBottomBar = true;
  let urlPreset: ResolvedEditorUrlPreset | null = null;
  let mirrorViewerFromSource = false;
  let externalFileConflict: { tabId: string; name: string; externalText: string; localText: string; languageId: SupportedEditorLanguageId } | null = null;
  let autoSaveTimer: ReturnType<typeof setTimeout> | null = null;
  let sessionSaveTimer: ReturnType<typeof setTimeout> | null = null;
  let sessionRestoring = false;
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
  };
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

  function applyUrlPresetUi(nextPreset: ResolvedEditorUrlPreset): void {
    showEditorPane = nextPreset.ui.editor;
    showViewerPane = nextPreset.ui.viewer;
    showTopBar = nextPreset.ui.topbar;
    showBottomBar = nextPreset.ui.bottombar;
    viewerViewMode = nextPreset.initialViewerMode;
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
      await viewerRef?.showTextPreview(nextPreset.rightText.value, nextLanguage);
    } else if (nextPreset.rightTextUrl.effective) {
      const resolved = await fetchUrlPresetSourceOrReport(nextPreset.rightTextUrl.value);
      if (!resolved) return;
      const nextLanguage = nextPreset.language ?? resolved.inferredLanguage ?? (editorRef?.getActiveLanguage() ?? $languageIdStore);
      await viewerRef?.showTextPreview(resolved.text, nextLanguage);
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
          await viewerRef?.showTextPreview(viewerRef?.getActiveText() ?? '', viewerRef?.getActiveLanguage() ?? effectiveLanguage);
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
          viewerViewMode = 'text';
          await viewerRef?.showTextPreview(nextText, editorRef?.getActiveLanguage() ?? effectiveLanguage);
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
        setViewMode: (mode) => { viewerViewMode = mode; },
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
        rebuildSubgraphWorkspace: async (panePaths) => await viewerRef.restoreSubgraphWorkspacePaths(panePaths.map(fromSharePath)),
        reportNavigationWarning: () => toast.warning('The shared document opened, but part of its saved navigation could not be restored.'),
      });
    } catch (error) {
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
      subgraphWorkspace: { panePaths: viewerRef?.getSubgraphWorkspacePaths().map(toSharePath) ?? [] },
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

  function getClampedSplitRatio(nextRatio: number, containerWidth = getContainerWidth()) {
    return getClampedSplitRatioValue(nextRatio, containerWidth, splitLayoutConfig.minPaneWidthPx);
  }

  function syncSplitLayoutState(nextState = splitLayoutState) {
    splitLayoutState = nextState;
    layoutMode = nextState.layoutMode;
    splitRatio = nextState.splitRatio;
    lastSplitRatio = nextState.lastSplitRatio;
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
    if (!payload.loading && !payload.error && viewerViewMode === 'graph' && $editorRevision !== lastTrackedGraphViewRevision) {
      lastTrackedGraphViewRevision = $editorRevision;
      trackEvent('graph_view', { language: editorRef?.getActiveLanguage() ?? $languageIdStore });
    }
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
    void viewerRef.showTextPreview($sourceTextStore, editorRef?.getActiveLanguage() ?? $languageIdStore);
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

  async function runEditorFullEditUsage<T>(
    source: string,
    execute: () => Promise<T>,
    reason = '',
  ): Promise<T> {
    // File imports already reserve large-file usage in handleImportFileStream.
    if (reason === 'import-file' || reason === 'drop-file') return execute();
    const byteLength = new TextEncoder().encode(source).byteLength;
    const capability = byteLength >= LARGE_FILE_PROCESSING_THRESHOLD_BYTES
      ? 'large_file_processing'
      : 'bidirectional_edit';
    return runPostpaidCapability({
      capability,
      idempotencyKey: crypto.randomUUID(),
      metadata: { byteLength, surface: 'editor_full_edit' },
      surface: 'graph_edit',
      execute,
      onBlocked: (block) => viewerRef?.showEntitlementOverlay(block),
    });
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
      editorRef?.replaceDocumentFromFile({ tabId, text: opened.text, languageId: tab.languageId });
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

  function discardLocalFileChange(): void {
    const conflict = externalFileConflict;
    if (!conflict) return;
    editorRef?.replaceDocumentFromFile({ tabId: conflict.tabId, text: conflict.externalText, languageId: conflict.languageId });
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

  function handleShowAiInput() {
    structGenerationOpen = false;
    yqInputOpen = false;
    aiInputOpen = true;
    aiError = '';
    aiSuccess = '';
    aiQuotaExhausted = false;
    aiUsageNotice = null;
  }

  function handleCloseAiInput() {
    if (aiBusy) return;
    aiInputOpen = false;
    aiError = '';
    aiSuccess = '';
    aiQuotaExhausted = false;
  }

  async function handleSubmitAi(instruction: string) {
    const sourceText = getActiveDocumentText();
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
      const suggestion = await suggestYq({
        instruction,
        editorTextSnapshot: sourceText,
        treePathSet: currentPath.length ? [serializePath(currentPath)] : undefined,
      });
      const result = await runYqPreview({
        expression: suggestion.expression,
        text: sourceText,
        language: editorRef?.getActiveLanguage() ?? $languageIdStore,
        formatting: $settings.formatting,
        enableNest: $settings.parser.enableNest,
        callWorker: callSharedWasmWorker,
      });
      if ('error' in result) {
        aiError = result.error;
        return;
      }
      await showViewerTextPreviewForRevision(result.result, result.previewLanguage, $editorRevision);
      aiSuccess = suggestion.expression;
    } catch (error) {
      if (error instanceof TreeaseServerError && error.code === 'quota_exhausted') {
        aiQuotaExhausted = true;
        aiError = error.message;
        pricingOpen = true;
        void refreshAiUsageNotice();
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
      void yqInputRef?.focus();
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
    if (!sourceText.trim()) {
      structGenerationError = 'The active document is empty.';
      return;
    }

    structGenerationBusy = true;
    structGenerationError = '';
    try {
      const sourceJson = await prepareStructGenerationSource({
        text: sourceText,
        language: sourceLanguage,
        formatting: $settings.formatting,
        callWorker: callSharedWasmWorker,
      });
      const result = await generateStruct({
        sourceJson,
        targetLanguage: structGenerationTarget,
        rootName: structGenerationRootName.trim() || 'Root',
      });
      await showViewerTextPreview(result.code, rightEditorLanguageForStruct(result.language));
    } catch (error) {
      structGenerationError = error instanceof Error ? error.message : 'Unable to generate the structure definition.';
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

  function updateSplitFromClientX(clientX: number) {
    if (!splitterDragRect) return;
    const offsetX = clamp(clientX - splitterDragRect.left, 0, splitterDragRect.width);
    syncSplitLayoutState({ ...splitLayoutState, splitRatio: getClampedSplitRatio(offsetX / splitterDragRect.width, splitterDragRect.width) });
  }

  function handleSplitterDragStart(clientX: number) {
    if (visibleLayoutMode !== 'split' || !splitLayoutContainer) return;
    splitterDragRect = splitLayoutContainer.getBoundingClientRect();
    isDraggingSplitter = true;
    updateSplitFromClientX(clientX);
  }

  function handleSplitterDragMove(clientX: number) {
    if (visibleLayoutMode !== 'split') return;
    updateSplitFromClientX(clientX);
  }

  function handleSplitterDragEnd() {
    if (visibleLayoutMode !== 'split') return;
    splitterDragRect = null;
    isDraggingSplitter = false;
    syncSplitLayoutState({ ...splitLayoutState, lastSplitRatio: splitRatio });
    void settingsStore.saveEditorSplitRatio(splitRatio);
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
    if (!syncScrollEnabled) return;
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

  function updateTreeSelection(
    path: PathSeg[],
    options: { target?: GraphHighlightTarget; source?: TreeSelectionSource } | undefined,
  ) {
    if (!path.length) return;
    activeTempModel.update((current) => ({
      ...current,
      treePath: path,
      graphHighlight: {
        path,
        target: options?.target,
        revision: Math.max($editorRevision, $graphAppliedRevision),
        source: options?.source ?? 'graph',
      },
    }));
  }

  function handleEditorReveal(event: CustomEvent<{ path: PathSeg[]; target?: 'key' | 'value' | 'node' }>) {
    const path = event?.detail?.path ?? [];
    if (!path.length || !syncScrollEnabled) return;
    viewerRef?.revealPath?.(path, { target: event.detail?.target });
  }
  function handleGraphReveal(payload: {
    path: PathSeg[];
    target?: 'key' | 'value' | 'node';
    trigger?: 'click' | 'search';
  }) {
    const path = payload?.path ?? [];
    if (!path.length || !syncScrollEnabled) return;

    // `emitReveal` in graph-text-linkage already sets graphHighlight via
    // syncTreeSelection before dispatching the reveal event. Skip this
    // redundant update when the path matches — otherwise EditorCore.revealPath
    // fires twice with different object references, causing duplicate
    // resolvePathSelectionRangeSafe calls that race on the WASM worker.
    const currentPath = get(activeTempModel)?.treePath ?? [];
    if (currentPath.length && serializePath(currentPath) === serializePath(path)) {
      return;
    }

    updateTreeSelection(path, {
      target: payload?.target,
      source: payload?.trigger === 'search' ? 'search' : 'graph',
    });
  }

  function handleTreePathSelect(path: PathSeg[]) {
    if (!path.length) return;
    updateTreeSelection(path, { target: breadcrumbTargetForPath(path), source: 'breadcrumb' });
  }

  function handleAddTab() {
    editorRef?.addTab();
  }

  function handleCloseTab(id: string) {
    const tab = getWorkspaceState().tabsById[id];
    if (showTabDirty && tab && isWorkspaceTabDirty(tab) && !window.confirm(`Close ${tab.name} without saving local changes?`)) {
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
    editorRef?.activateTab(id);
  }

  function sessionFromWorkspace(): WorkspaceSession {
    const workspace = getWorkspaceState();
    return {
      version: 1,
      activeTabIndex: Math.max(0, workspace.tabOrder.indexOf(workspace.activeTabId)),
      tabs: workspace.tabOrder.flatMap((tabId) => {
        const tab = workspace.tabsById[tabId];
        return tab && tab.role !== 'sidecar' ? [{
          name: tab.name,
          languageId: tab.languageId,
          sourceText: tab.sourceText,
          savedText: tab.savedText,
          linkedFileName: tab.fileLinkedDocument?.name,
        }] : [];
      }),
    };
  }

  function scheduleWorkspaceSessionSave(): void {
    if (sessionRestoring) return;
    if (sessionSaveTimer) clearTimeout(sessionSaveTimer);
    sessionSaveTimer = setTimeout(() => {
      void (async () => {
        const host = await workspaceHost;
        if (host.surface === 'desktop') await host.saveSession(sessionFromWorkspace());
      })();
    }, 300);
  }

  async function restoreWorkspaceSession(session: WorkspaceSession): Promise<void> {
    if (!session.tabs.length || !editorRef) return;
    sessionRestoring = true;
    try {
      await editorRef.ensureReady();
      const [first, ...remaining] = session.tabs;
      const firstLanguage = languageForFileName(`recovery.${first.languageId}`);
      await editorRef.replaceActiveFromFile({ text: first.sourceText, languageId: firstLanguage });
      const firstTab = activeWorkspaceTab();
      if (firstTab) {
        editorRef.renameDocument(firstTab.id, first.name);
        updateWorkspaceTab(firstTab.id, { name: first.name, sourceText: first.sourceText, savedText: first.savedText });
      }
      for (const tab of remaining) {
        editorRef.openDocument({
          name: tab.name,
          text: tab.sourceText,
          languageId: languageForFileName(`recovery.${tab.languageId}`),
        });
      }
      await tick();
      const restored = getWorkspaceState().tabOrder;
      const active = restored[Math.min(session.activeTabIndex, restored.length - 1)];
      if (active) editorRef.activateTab(active);
      toast.info('Recovered the previous desktop workspace as local drafts.');
    } finally {
      sessionRestoring = false;
    }
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

  type StaticWorkspaceCommand = Exclude<WorkspaceCommand, `workspace:open-recent:${string}`>;

  const workspaceCommands: Record<StaticWorkspaceCommand, () => void | Promise<void>> = {
    'workspace:new': handleAddTab,
    'workspace:open': handleOpenDocument,
    'workspace:save': () => saveActiveDocument(),
    'workspace:save-as': () => saveActiveDocument(true),
    'workspace:import': () => topBarRef?.openImportPanel(),
    'workspace:export': () => topBarRef?.openExportPanel(),
    'workspace:clear-recent': handleClearRecentFiles,
    'workspace:close-tab': () => activeTabId && handleCloseTab(activeTabId),
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
  $: shellRowsClass = showTopBar
    ? showBottomBar
      ? 'var(--topbar-height) minmax(0, 1fr) var(--bottombar-height)'
      : 'var(--topbar-height) minmax(0, 1fr)'
    : showBottomBar
      ? 'minmax(0, 1fr) var(--bottombar-height)'
      : 'minmax(0, 1fr)';

  onMount(() => {
    urlPreset ??= resolveEditorUrlPreset(window.location.search);
    const shareID = urlPreset.shareID;
    let stopWorkspaceSession: (() => void) | null = null;
    let stopWorkspaceCommands: (() => void) | null = null;
    let stopDeepLinks: (() => void) | null = null;
    void (async () => {
      const host = await workspaceHost;
      showTabDirty = host.surface === 'desktop';
      if (shareID.present) return;
      if (host.surface === 'desktop') {
        const session = await host.loadSession();
        if (session) await restoreWorkspaceSession(session);
        for (const file of await host.takeStartupFiles()) await openWorkspaceFile(file);
        await handleDesktopDeepLinks(await host.getInitialDeepLinks());
        stopWorkspaceCommands = await host.onCommand((command) => void handleWorkspaceCommand(command));
        stopDeepLinks = await host.onDeepLinks((urls) => void handleDesktopDeepLinks(urls).catch((error) => {
          const message = error instanceof Error ? error.message : String(error);
          toast.error(`Desktop deep link failed: ${message}`);
        }));
      }
      stopWorkspaceSession = editorWorkspace.subscribe(() => scheduleWorkspaceSessionSave());
      scheduleWorkspaceSessionSave();
    })().catch((error) => {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(`Desktop workspace initialization failed: ${message}`);
    });
    void (async () => {
      await settingsStore.load();
      const savedSplitRatio = settingsStore.getEditorSplitRatio();
      if (savedSplitRatio !== null) {
        syncSplitLayoutState(createSplitLayoutState(savedSplitRatio));
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
  <div class="grid h-full min-h-0 min-w-0 overflow-hidden" style:grid-template-rows={shellRowsClass}>
    {#if showTopBar}
      <TopBar
        bind:this={topBarRef}
        tabs={tabSummaries}
        {showTabDirty}
        {activeTabId}
        canAddTab={tabSummaries.length < maxTabs}
        showTabs={true}
        showRightActions={true}
        {formatOptions}
        onAddTab={workspaceCommands['workspace:new']}
        onCloseTab={handleCloseTab}
        onActivateTab={handleActivateTab}
        onRequestImportFile={handleRequestImportFile}
        onImportFileStream={handleImportFileStream}
        onExportPreview={handleExportPreview}
        onExportDownload={handleExportDownload}
        onShare={() => (shareOpen = true)}
        onFeedback={() => (feedbackOpen = true)}
        onLogin={() => (loginOpen = true)}
        onLogout={handleLogout}
        onCheckForUpdates={handleCheckForUpdates}
        onOpenSettings={() => (settingsOpen = true)}
      />
    {/if}
    <div bind:this={splitLayoutContainer} bind:clientWidth={containerWidth} class="app-split-layout" class:invisible={!layoutReady}>
      {#if showEditorPane}
        <section
          class="app-split-pane app-split-pane--left flex flex-col border-r border-[var(--border-strong)] bg-[var(--panel-bg)]"
          class:app-split-pane--collapsed={leftPaneCollapsed}
          class:app-split-pane--instant={isDraggingSplitter}
          data-testid="left-pane"
          aria-hidden={leftPaneCollapsed}
          style:width={`${leftPaneWidthPx}px`}
          style:opacity={leftPaneCollapsed ? 0 : 1}
        >
          <div class="min-h-0 flex-1">
            <Editor
              bind:this={editorRef}
              bind:tabSummaries
              bind:activeTabId
              enableRevealSync={syncScrollEnabled}
              {synchronizedRuntimeLoading}
              runBidirectionalEdit={runEditorFullEditUsage}
              on:reveal={handleEditorReveal}
              on:runtime-state={handleEditorRuntimeEvent}
              onScroll={handleEditorScroll}
            />
          </div>
          {#if aiInputOpen}
            <AiInput
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
              onUpgrade={() => { pricingOpen = true; }}
              onClose={handleCloseAiInput}
            />
          {:else if yqInputOpen}
            <YqExpressionInput
              bind:this={yqInputRef}
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
        </section>
      {/if}

      {#if visibleLayoutMode === 'split'}
        <div
          class={`app-split-divider app-split-divider--vertical ${isDraggingSplitter ? 'app-split-divider--dragging' : ''}`}
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
        >
          {#if !synchronizedRuntimeLoading}
            <div class="splitter-control splitter-control--split" role="presentation">
              <div class="splitter-control__buttons" role="presentation" on:pointerdown|stopPropagation>
                <ButtonGroup.Root orientation="vertical" variant="segmented-outline" class="shadow-none">
                  <IconButton
                    class="text-[var(--text-primary)]"
                    aria-label={viewerViewMode === 'graph' ? 'Text mode' : 'Graph mode'}
                    title={viewerViewMode === 'graph' ? 'Text mode' : 'Graph mode'}
                    data-testid={viewerViewMode === 'graph' ? 'text-mode-button' : 'graph-mode-button'}
                    on:click={() => (viewerViewMode = viewerViewMode === 'graph' ? 'text' : 'graph')}
                  >
                    {#if viewerViewMode === 'graph'}
                      <FileCode size={12} />
                    {:else}
                      <GitGraph size={12} />
                    {/if}
                  </IconButton>
                  <IconButton
                    class={syncScrollEnabled ? 'text-[var(--accent)]' : 'text-[var(--text-muted)]'}
                    aria-label={syncScrollEnabled ? 'Disable synchronized scrolling' : 'Enable synchronized scrolling'}
                    title={syncScrollEnabled ? 'Disable synchronized scrolling' : 'Enable synchronized scrolling'}
                    data-testid="sync-scroll-toggle"
                    on:click={toggleSyncScroll}
                  >
                    {#if syncScrollEnabled}
                      <Link2 size={12} />
                    {:else}
                      <Link2Off size={12} />
                    {/if}
                  </IconButton>
                  <IconButton
                    class="text-[var(--text-primary)]"
                    aria-label="Collapse viewer"
                    title="Collapse viewer"
                    on:click={collapseViewer}
                  >
                    <PanelRightClose size={12} />
                  </IconButton>
                  <IconButton
                    class="text-[var(--text-primary)]"
                    aria-label="Collapse editor"
                    title="Collapse editor"
                    on:click={collapseEditor}
                  >
                    <PanelLeftClose size={12} />
                  </IconButton>
                </ButtonGroup.Root>
              </div>
            </div>
          {/if}
        </div>
      {/if}

      {#if showViewerPane}
        <section
          class="app-split-pane app-split-pane--right bg-[var(--panel-bg-alt)]"
          class:app-split-pane--collapsed={rightPaneCollapsed}
          class:app-split-pane--instant={isDraggingSplitter}
          data-testid="right-pane"
          aria-hidden={rightPaneCollapsed}
          style:width={`${rightPaneWidthPx}px`}
          style:opacity={rightPaneCollapsed ? 0 : 1}
        >
          <ViewportPanel
            bind:this={viewerRef}
            bind:viewMode={viewerViewMode}
            enableRevealSync={syncScrollEnabled}
            {synchronizedRuntimeLoading}
            onRevealError={(line, column) => editorRef?.revealError(line, column)}
            onGraphReveal={handleGraphReveal}
            onGraphRuntimeState={handleViewerRuntimeState}
            onTextScroll={handleViewerScroll}
            onApplyDiff={handleApplyDiff}
            onSwap={handleSwapEditors}
          />
        </section>
      {/if}

      {#if renderLayoutControls}
        <div
          class="splitter-control"
          style:left={`${splitterControlLeftPx}px`}
          transition:fly={{ x: collapsedControlFlyX, duration: 150, opacity: 0.08, easing: cubicOut }}
        >
          {#if visibleLayoutMode === 'left-only'}
            <ButtonGroup.Root orientation="vertical" variant="segmented-outline" class="shadow-none">
              <IconButton
                class="text-[var(--text-primary)]"
                aria-label="Expand viewer"
                title="Expand viewer"
                on:click={expandSplitLayout}
              >
                <PanelRightOpen size={12} />
              </IconButton>
            </ButtonGroup.Root>
          {:else if visibleLayoutMode === 'right-only'}
            <ButtonGroup.Root orientation="vertical" variant="segmented-outline" class="shadow-none">
              <IconButton
                class="text-[var(--text-primary)]"
                aria-label="Expand editor"
                title="Expand editor"
                on:click={expandSplitLayout}
              >
                <PanelLeftOpen size={12} />
              </IconButton>
            </ButtonGroup.Root>
          {/if}
        </div>
      {/if}
    </div>
    {#if showBottomBar}
      <BottomBar
        onFormat={() => editorRef?.formatActive()}
        onMinify={() => editorRef?.minifyActive()}
        onCompact={() => editorRef?.compactActive()}
        onSort={() => editorRef?.sortActive()}
        onShowAiInput={handleShowAiInput}
        onShowYqInput={handleShowYqInput}
        onGenerateStruct={handleShowStructGeneration}
        onEscape={() => editorRef?.escapeActive()}
        onUnescape={() => editorRef?.unescapeActive()}
        onNewDocument={workspaceCommands['workspace:new']}
        onOpenDocument={workspaceCommands['workspace:open']}
        onSaveDocument={workspaceCommands['workspace:save']}
        onSaveAsDocument={workspaceCommands['workspace:save-as']}
        onCloseDocument={workspaceCommands['workspace:close-tab']}
        onTreePathSelect={handleTreePathSelect}
      />
    {/if}
  </div>
  <FeedbackDialog bind:open={feedbackOpen} />
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
{#if !showEditorPane}
  <div class="pointer-events-none absolute -left-[10000px] top-0 h-px w-px overflow-hidden opacity-0" aria-hidden="true">
    <Editor
      bind:this={editorRef}
      bind:tabSummaries
      bind:activeTabId
      enableRevealSync={syncScrollEnabled}
      {synchronizedRuntimeLoading}
      runBidirectionalEdit={runEditorFullEditUsage}
      on:reveal={handleEditorReveal}
      on:runtime-state={handleEditorRuntimeEvent}
      onScroll={handleEditorScroll}
    />
  </div>
{/if}
<SettingsDialog bind:open={settingsOpen} />
<ShareDialog bind:open={shareOpen} createResource={createShareResource} />
<LoginDialog bind:open={loginOpen} />
<Dialog bind:open={pricingOpen}>
  <DialogContent aria-labelledby="ai-pricing-title" class="max-h-[90vh] border-[#dce5f0] bg-[#f7faff] p-6" style={`width: min(${aiPricingDialogMaxWidth}, calc(100vw - 2rem)); max-width: none; overflow-y: auto; scrollbar-gutter: stable;`}>
    <PricingPlanGrid
      compact
      title="Usage limit reached"
      titleId="ai-pricing-title"
      titleNoWrap
      descriptionNoWrap
      showKicker={false}
      description="Your last action used the final monthly run. Upgrade to continue."
      visiblePlanIds={aiPricingPlanIds}
      usageNotice={aiUsageNotice}
      actionDisabled={() => aiUpgradeBusy}
      actionLabel={(plan) => aiUpgradeBusy ? 'Opening checkout…' : plan.ctaLabel}
      onSelectPlan={(priceId) => void handleAiQuotaUpgrade(priceId)}
    />
  </DialogContent>
</Dialog>
