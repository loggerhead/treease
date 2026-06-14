<!-- 职责：Editor 路由页面：Editor/Viewport/TopBar/BottomBar 组件装配、跨组件事件编排、DOM 交互 -->
<script lang="ts">
  import { onMount, tick } from 'svelte';
  import Editor from '../../lib/components/Editor.svelte';
  import TopBar from '../../lib/components/TopBar.svelte';
  import BottomBar from '../../lib/components/BottomBar.svelte';
  import ViewportPanel from '../../lib/components/ViewportPanel.svelte';
  import SettingsDialog from '../../lib/components/SettingsDialog.svelte';
  import YqExpressionInput from '../../lib/components/YqExpressionInput.svelte';
  import { settings, settingsStore } from '../../lib/settings/settings-store';
  import {
    activeTempModel,
    editorRevision,
    graphAppliedRevision,
    languageId as languageIdStore,
    type GraphHighlightTarget,
    type TreeSelectionSource,
  } from '../../lib/store/editor-store';
  import { toast } from 'svelte-sonner';
  import { readImportSourceSample, resolveImportSourceFormat } from '../../lib/import/resolve-import-source';
  import { callSharedWasmWorker, getSharedWasmWorkerClient } from '../../lib/wasm/wasm-worker-singleton';
  import wasmUrl from '@core-wasm/pkg/core.wasm?url';
  import { runYqPreview } from './yq-preview-controller';
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
  import { importFormatOptions, supportedEditorLanguageSet, editorLanguageFallback, type SupportedEditorLanguageId } from '../../lib/monaco/language-support';
  import { computeSynchronizedRuntimeLoading, type RuntimeStateEventDetail } from '../../lib/runtime-loading';
  import { breadcrumbTargetForPath, type PathSeg } from '../../lib/store/tree-path';
  import type { DiffPlan } from '../../lib/graph/diff-plan';
  import {
    SquareChevronLeft,
    ArrowLeftToLine,
    ArrowRightToLine,
    Braces,
    Share2,
    GripVertical,
    Link2,
    Link2Off,
  } from 'lucide-svelte';
  import * as ButtonGroup from '../../lib/components/ui/button-group';
  import { IconButton } from '../../lib/components/ui/button';

  let editorRef: Editor | null = null;
  let viewerRef: ViewportPanel | null = null;
  let yqInputRef: YqExpressionInput | null = null;
  let splitLayoutContainer: HTMLDivElement | null = null;
  let containerWidth = 0;
  let tabSummaries: Array<{ id: string; name: string; languageId: SupportedEditorLanguageId }> = [];
  let activeTabId = '';
  let scrollSyncLock: 'editor' | 'viewer' | null = null;
  let splitLayoutState = createSplitLayoutState(0.28);
  let layoutMode = splitLayoutState.layoutMode;
  let splitRatio = splitLayoutState.splitRatio;
  let lastSplitRatio = splitLayoutState.lastSplitRatio;
  let leftPaneWidthPx = 0;
  let rightPaneWidthPx = 0;
  let splitterLeftPx = 0;
  let splitterControlLeftPx = 0;
  let isDraggingSplitter = false;
  let settingsOpen = false;
  let viewerViewMode: 'graph' | 'text' = 'graph';
  let editorRuntimeLoading = true;
  let viewerRuntimeLoading = true;
  let synchronizedRuntimeLoading = true;
  let syncScrollEnabled = true;
  let yqInputOpen = false;
  let yqExpression = '';
  let yqBusy = false;
  let yqError = '';
  const maxTabs = 9;
  type ScrollPosition = { scrollTop: number; scrollLeft: number };
  type ScrollSyncOwner = 'editor' | 'viewer';
  type SplitLayoutState = ReturnType<typeof createSplitLayoutState>;

  const formatOptions = importFormatOptions;
  const defaultSplitRatio = 0.28;
  const splitLayoutConfig: SplitLayoutConfig = {
    defaultSplitRatio,
    minPaneWidthPx: 200,
    dividerWidthPx: 10,
    collapsedControlInsetPx: 16,
  };

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

  function showViewerTextPreview(text: string) {
    viewerRef?.showTextPreview(text);
  }

  function handleEditorRuntimeEvent(event: CustomEvent<RuntimeStateEventDetail>) {
    editorRuntimeLoading = event.detail.loading;
  }

  function handleViewerRuntimeState(payload: RuntimeStateEventDetail) {
    viewerRuntimeLoading = payload.loading;
  }

  $: synchronizedRuntimeLoading = computeSynchronizedRuntimeLoading({
    viewMode: viewerViewMode,
    editorRuntimeLoading,
    graphRuntimeLoading: viewerRuntimeLoading,
  });
  $: syncScrollEnabled = $settings?.interaction?.enableSyncScroll ?? true;

  async function handleImportFileStream(payload: {
    file: File;
    sourceFormat: string;
    targetFormat: string;
    fileName: string;
  }) {
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
      await editorRef?.importStream(payload.file, effectiveSource, targetFormat);
      toast.success(`Imported ${payload.fileName}`);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      console.error('[import] file import failed', { fileName: payload.fileName, error: message });
      toast.error(`Import failed: ${message}`);
    }
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
    showViewerTextPreview(text);
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
    downloadText(text, download.fileName);
    for (const message of download.toastMessages) {
      toast.success(message);
    }
  }

  function downloadText(text: string, fileName: string) {
    const blob = new Blob([text], { type: 'text/plain;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = fileName;
    link.click();
    URL.revokeObjectURL(url);
  }

  function handleShowYqInput() {
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

  async function handleSubmitYq(expression: string) {
    const nextExpression = expression.trim();
    yqExpression = nextExpression;
    yqBusy = true;
    yqError = '';
    const result = await runYqPreview({
      expression: nextExpression,
      text: editorRef?.getActiveText() ?? '',
      language: editorRef?.getActiveLanguage() ?? $languageIdStore,
      formatting: $settings.formatting,
      enableNest: $settings.parser.enableNest,
      callWorker: callSharedWasmWorker,
    });
    if ('error' in result) {
      yqError = result.error;
    } else {
      showViewerTextPreview(result.result);
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

  function handleSplitterPointerDown(event: PointerEvent) {
    if (layoutMode !== 'split' || !splitLayoutContainer) return;
    event.preventDefault();
    const rect = splitLayoutContainer.getBoundingClientRect();
    const handleTarget = event.currentTarget as HTMLElement | null;
    handleTarget?.setPointerCapture?.(event.pointerId);
    isDraggingSplitter = true;

    const updateFromClientX = (clientX: number) => {
      const offsetX = clamp(clientX - rect.left, 0, rect.width);
      syncSplitLayoutState({ ...splitLayoutState, splitRatio: getClampedSplitRatio(offsetX / rect.width, rect.width) });
    };

    const handlePointerMove = (moveEvent: PointerEvent) => {
      updateFromClientX(moveEvent.clientX);
    };

    const stopDragging = () => {
      window.removeEventListener('pointermove', handlePointerMove);
      window.removeEventListener('pointerup', stopDragging);
      window.removeEventListener('pointercancel', stopDragging);
      isDraggingSplitter = false;
      syncSplitLayoutState({ ...splitLayoutState, lastSplitRatio: splitRatio });
    };

    window.addEventListener('pointermove', handlePointerMove);
    window.addEventListener('pointerup', stopDragging);
    window.addEventListener('pointercancel', stopDragging);
    updateFromClientX(event.clientX);
  }

  function handleApplyDiff(plan: DiffPlan) {
    editorRef?.applyDiffPlan(plan);
  }

  async function handleSwapEditors(payload: { rightText: string; rightLanguage: SupportedEditorLanguageId }) {
    const leftText = editorRef?.getActiveText() ?? '';
    await editorRef?.importAs(payload.rightLanguage, payload.rightText, payload.rightLanguage);
    showViewerTextPreview(leftText);
  }

  async function toggleSyncScroll() {
    const nextEnabled = !syncScrollEnabled;
    scrollSyncLock = null;
    await settingsStore.save({
      interaction: {
        enableSyncScroll: nextEnabled,
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
    options?: { target?: GraphHighlightTarget; source?: TreeSelectionSource },
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
    editorRef?.closeTab(id);
  }

  function handleActivateTab(id: string) {
    editorRef?.activateTab(id);
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
  $: ({ leftPaneWidthPx, rightPaneWidthPx, splitterLeftPx, splitterControlLeftPx } = computePaneWidths(
    splitLayoutState,
    containerWidth,
    splitLayoutConfig,
  ));

  onMount(() => {
    settingsStore.load();
    void getSharedWasmWorkerClient().catch(() => {});
    const handleResize = () => {
      syncSplitLayoutState(syncSplitRatio(splitLayoutState, getContainerWidth(), splitLayoutConfig));
    };
    window.addEventListener('resize', handleResize);
    return () => {
      window.removeEventListener('resize', handleResize);
    };
  });
</script>
<svelte:head>
  <link rel="preload" as="fetch" href={wasmUrl} crossorigin="anonymous" />
</svelte:head>

<main class="grid h-screen w-screen bg-[var(--app-bg)] text-[var(--text-primary)]">
  <div class="grid min-h-0 min-w-0 grid-rows-[var(--topbar-height)_1fr_var(--bottombar-height)] overflow-hidden">
    <TopBar
      tabs={tabSummaries}
      {activeTabId}
      canAddTab={tabSummaries.length < maxTabs}
      showTabs={true}
      showRightActions={false}
      {formatOptions}
      onAddTab={handleAddTab}
      onCloseTab={handleCloseTab}
      onActivateTab={handleActivateTab}
      onImportFileStream={handleImportFileStream}
      onExportPreview={handleExportPreview}
      onExportDownload={handleExportDownload}
      onOpenSettings={() => (settingsOpen = true)}
    />
    <div bind:this={splitLayoutContainer} bind:clientWidth={containerWidth} class="app-split-layout">
      {#if layoutMode !== 'right-only'}
        <section
          class="app-split-pane app-split-pane--left flex flex-col border-r border-[var(--border-strong)] bg-[var(--panel-bg)]"
          data-testid="left-pane"
          style:width={`${leftPaneWidthPx}px`}
        >
          <div class="min-h-0 flex-1">
            <Editor
              bind:this={editorRef}
              bind:tabSummaries
              bind:activeTabId
              enableRevealSync={syncScrollEnabled}
              {synchronizedRuntimeLoading}
              on:reveal={handleEditorReveal}
              on:runtime-state={handleEditorRuntimeEvent}
              onScroll={handleEditorScroll}
            />
          </div>
          {#if yqInputOpen}
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
          {/if}
        </section>
      {/if}

      {#if layoutMode === 'split'}
        <div
          class={`app-split-divider ${isDraggingSplitter ? 'app-split-divider--dragging' : ''}`}
          data-testid="splitter-divider"
          role="separator"
          aria-label="Resize panels"
          aria-orientation="vertical"
          style:left={`${splitterLeftPx}px`}
          on:pointerdown={handleSplitterPointerDown}
        >
          {#if !synchronizedRuntimeLoading}
            <div class="splitter-control splitter-control--split" role="presentation">
              <div class="splitter-control__drag-hint" aria-hidden="true">
                <GripVertical size={14} />
              </div>
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
                      <Braces size={12} />
                    {:else}
                      <Share2 size={12} />
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
                    <ArrowRightToLine size={12} />
                  </IconButton>
                  <IconButton
                    class="text-[var(--text-primary)]"
                    aria-label="Collapse editor"
                    title="Collapse editor"
                    on:click={collapseEditor}
                  >
                    <ArrowLeftToLine size={12} />
                  </IconButton>
                </ButtonGroup.Root>
              </div>
            </div>
          {/if}
        </div>
      {/if}

      {#if layoutMode !== 'left-only'}
        <section
          class="app-split-pane app-split-pane--right bg-[var(--panel-bg-alt)]"
          data-testid="right-pane"
          style:width={`${rightPaneWidthPx}px`}
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

      {#if layoutMode !== 'split'}
        <div class="splitter-control" style:left={`${splitterControlLeftPx}px`}>
          {#if layoutMode === 'left-only'}
            <ButtonGroup.Root orientation="vertical" variant="segmented-outline" class="shadow-none">
              <IconButton
                class="text-[var(--text-primary)]"
                aria-label="Expand viewer"
                title="Expand viewer"
                on:click={expandSplitLayout}
              >
                <SquareChevronLeft size={12} />
              </IconButton>
            </ButtonGroup.Root>
          {:else if layoutMode === 'right-only'}
            <ButtonGroup.Root orientation="vertical" variant="segmented-outline" class="shadow-none">
              <IconButton
                class="text-[var(--text-primary)]"
                aria-label="Expand editor"
                title="Expand editor"
                on:click={expandSplitLayout}
              >
                <ArrowRightToLine size={12} />
              </IconButton>
            </ButtonGroup.Root>
          {/if}
        </div>
      {/if}
    </div>
    <BottomBar
      onFormat={() => editorRef?.formatActive()}
      onMinify={() => editorRef?.minifyActive()}
      onSort={() => editorRef?.sortActive()}
      onShowYqInput={handleShowYqInput}
      onEscape={() => editorRef?.escapeActive()}
      onUnescape={() => editorRef?.unescapeActive()}
      onTreePathSelect={handleTreePathSelect}
    />
  </div>
</main>
<SettingsDialog bind:open={settingsOpen} />
