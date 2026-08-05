<script lang="ts">
  import { tick } from 'svelte'
  import { toast } from 'svelte-sonner'
  import {
    compareEditToken,
    documentKey as documentKeyStore,
    languageId as languageIdStore,
    sourceText,
  } from '../store/document-session-store'
  import { activeSidecarTempModel } from '../store/active-sidecar-state'
  import { editorWorkspace, getWorkspaceState } from '../store/workspace-store'
  import { jsonBlockSelection } from '../store/full-edit-ui-store'
  import type { PathSeg } from '../store/tree-path'
  import type { ColumnNavigatorState } from './graph-viewer/column-navigator/types'
  import { readImportSourceSample, resolveImportSourceFormat } from '../import/resolve-import-source'
  import { callSharedWasmWorker } from '../wasm/wasm-worker-singleton'
  import {
    editorLanguageFallback,
    supportedEditorLanguageSet,
    type SupportedEditorLanguageId,
  } from '../monaco/language-support'
  import { buildDiffPlans, type DiffPair, type DiffPlan } from '../graph/diff-plan'
  import type { RuntimeStateEventDetail } from '../runtime-loading'
  import GraphViewer from './GraphViewer.svelte'
  import GraphSearchPanel from './GraphSearchPanel.svelte'
  import SidecarEditor from './Editor/SidecarEditor.svelte'
  import {
    Search,
    ZoomIn,
    ZoomOut,
    GitCompareArrows,
    ArrowRightLeft,
    FileInput,
    ImageDown
  } from 'lucide-svelte'
  import * as ButtonGroup from './ui/button-group'
  import { Button, IconButton } from './ui/button'
  import { escapeHtml } from '../preview/utils'
  import { trackEvent } from '../analytics/ga4'
  import type { UsageBlock } from '../billing/entitlement-gate'
  import EntitlementOverlay from './EntitlementOverlay.svelte'
  import type { PricingUsageNotice } from './PricingPlanGrid.svelte'
  import type { BillingPriceId, PricingPlan } from '$lib/config/pricing'
  import type { SharedWorkspaceMutationTarget } from '../share/share-workspace-lifecycle'
  import { tabTargetStatus, type TabTarget } from '../store/tab-target'
  import {
    captureActiveSidecarTarget,
    captureSidecarTarget,
    updateSidecarCompareOutcome,
    updateSidecarCompareScroll,
  } from '../store/sidecar-tab-state'

  type PricingPlanGridComponent = typeof import('./PricingPlanGrid.svelte').default

  export let viewMode: 'graph' | 'text' = 'graph'
  export let onRevealError: (line: number, column: number) => void = () => {}
  export let onGraphNavigation: (payload: { path: PathSeg[]; target?: 'key' | 'value' | 'node'; trigger?: 'click' | 'search-preview' | 'search-commit' | 'breadcrumb' }) => void =
    () => {}
  export let onApplyDiff: (plan: DiffPlan) => void = () => {}
  export let onTextScroll: (payload: { scrollTop: number; scrollLeft: number }) => void = () => {}
  export let onSwap: (payload: { rightText: string; rightLanguage: SupportedEditorLanguageId }) => void = () => {}
  export let onGraphRuntimeState: (payload: RuntimeStateEventDetail) => void = () => {}
  export let sidecarTabId = ''
  export let onColumnNavigatorState: (payload: { tabId: string; state: ColumnNavigatorState }) => void = () => {}
  export let onGraphViewportState: (payload: { tabId: string; viewport: { x: number; y: number; scaleX: number; scaleY: number } | null }) => void = () => {}
  export let synchronizedRuntimeLoading = false
  export let graphOnly = false
  export let readonlyGraph = false
  export let pricingPlanGridComponent: PricingPlanGridComponent | null = null
  export let pricingUsageNotice: PricingUsageNotice | null = null
  export let onPricingSelectPlan: (priceId: BillingPriceId) => void = () => {}
  export let pricingActionDisabled: (plan: PricingPlan) => boolean = () => false
  export let pricingActionLabel: (plan: PricingPlan) => string = (plan) => plan.ctaLabel
  export let onEntitlementBlocked: (block: UsageBlock) => void = () => {}
  export let onFileDrop: (event: DragEvent) => void | Promise<void> = () => {}
  export let onRequestImportFile: (payload: { sourceFormat: string; targetFormat: string; accept: string[] }) => void | Promise<void> = () => {}
  export let onLoadExample: (example: string, language: SupportedEditorLanguageId) => void | Promise<void> = () => {}
  export let ensureSharedWorkspacePromoted: (target: SharedWorkspaceMutationTarget) => Promise<boolean> = async () => true
  export let hideGraphToolbar = false
  export let emptyDocument = false

  type GraphSearchResult = {
    nodeId?: number
    target: 'node' | 'key' | 'value'
    label: string
    path: PathSeg[]
    pathText: string
  }

  type DiffResponse = {
    mode: 'tree' | 'text'
    equal: boolean
    result: { pairs: DiffPair[]; leftFillRanges: { startLineNumber: number; endLineNumber: number }[]; rightFillRanges: { startLineNumber: number; endLineNumber: number }[] }
  }

  let diffError = ''
  let rightCompareHighlightCount = 0
  let languageIdValue: SupportedEditorLanguageId = editorLanguageFallback
  let scratchText = ''
  let scratchLanguage: SupportedEditorLanguageId = editorLanguageFallback
  let lastSourceText: string | null = null
  let lastCompareEditToken: number | null = null
  let lastCompareLeftLanguage: SupportedEditorLanguageId | null = null
  let lastCompareRightLanguage: SupportedEditorLanguageId | null = null
  let compareGeneration = 0
  let rightPanelFileInput: HTMLInputElement | null = null
  let sidecarEditor: SidecarEditor | null = null
  let graphViewer: any = null
  let graphSearchPanel: GraphSearchPanel | null = null
  let effectiveViewMode: 'graph' | 'text' = 'graph'
  let entitlementOverlay: UsageBlock | null = null
  let pricingOverlayVisible = false
  let entitlementDocumentKey = ''
  let activeSidecarTabId = ''
  let activeSidecarLanguage: SupportedEditorLanguageId = editorLanguageFallback
  let restoredCompareSidecarTabId = ''
  $: visibleGraphDiagnostics = $jsonBlockSelection ? [] : ($activeSidecarTempModel?.diagnostics ?? []).slice(0, 2)
  $: {
    const activeMainTab = $editorWorkspace.tabsById[$editorWorkspace.activeTabId]
    const sidecar = activeMainTab?.sidecarTabId ? $editorWorkspace.tabsById[activeMainTab.sidecarTabId] : null
    activeSidecarTabId = sidecar?.id ?? ''
    activeSidecarLanguage = sidecar?.languageId ?? languageIdValue
  }
  $: effectiveViewMode = graphOnly ? 'graph' : viewMode
  $: if ($documentKeyStore !== entitlementDocumentKey) {
    entitlementDocumentKey = $documentKeyStore
    entitlementOverlay = null
    pricingOverlayVisible = false
    pricingUsageNotice = null
  }

  function sanitizeContextText(text: string) {
    const MAX_LINE_LEN = 100;
    const cleaned = text.replace(/\bon:\w+\b/g, '');
    if (cleaned.length <= MAX_LINE_LEN) return cleaned;
    return cleaned.slice(0, MAX_LINE_LEN - 3) + '...';
  }


  function renderContextLineHtml(text: string, isErrorLine: boolean, startColumn: number, endColumn: number): string {
    const cleaned = sanitizeContextText(text);
    if (!cleaned) return '';
    if (!isErrorLine || startColumn <= 0 || startColumn > cleaned.length) {
      return escapeHtml(cleaned);
    }
    const start = Math.max(0, startColumn - 1);
    const end = Math.min(cleaned.length, Math.max(start, (endColumn ?? startColumn) - 1));
    const before = escapeHtml(cleaned.slice(0, start));
    const errorToken = escapeHtml(cleaned.slice(start, end));
    const after = escapeHtml(cleaned.slice(end));
    return `${before}<span class="text-[rgb(203,42,47)]">${errorToken}</span>${after}`;
  }
  function hasRightCompareHighlights(): boolean {
    return rightCompareHighlightCount > 0
  }

  function handleGraphViewerRuntimeState(event: CustomEvent<RuntimeStateEventDetail>): void {
    onGraphRuntimeState(event.detail)
  }

  function normalizeCompareText(text: string): string {
    return text.replace(/\r\n?/g, '\n')
  }

  async function ensureSidecarEditorReady(): Promise<SidecarEditor | null> {
    await tick()
    await sidecarEditor?.ensureReady()
    return sidecarEditor
  }

  async function waitForSidecarStoreSync(): Promise<void> {
    await ensureSidecarEditorReady()
  }

  function getSidecarText(): string {
    return sidecarEditor?.getText() ?? scratchText
  }

  function getSidecarLanguage(): SupportedEditorLanguageId {
    return sidecarEditor?.getLanguage() ?? scratchLanguage
  }

  function clearRightDiff() {
    sidecarEditor?.clearDiffPlan()
    rightCompareHighlightCount = 0
  }

  function clearCompareHighlights(): void {
    if (hasRightCompareHighlights()) {
      clearRightDiff()
    }
    onApplyDiff({ decorations: [], fillRanges: [] })
  }

  function invalidateCompare(target: TabTarget | null = null): void {
    compareGeneration += 1
    const currentTarget = target ?? captureActiveSidecarTarget()
    if (currentTarget) updateSidecarCompareOutcome(currentTarget, { kind: 'none' })
    clearCompareHighlights()
  }

  function applyRightDiffPlan(plan: DiffPlan) {
    rightCompareHighlightCount = sidecarEditor?.applyDiffPlan(plan) ?? 0
  }

  async function waitForDecorationPaint(): Promise<void> {
    await tick()
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
  }

  async function runDiffCompare() {
    if (graphOnly || effectiveViewMode !== 'text') return
    let runGeneration = 0
    let target: TabTarget | null = null
    try {
      await waitForSidecarStoreSync()
      // The compare operation starts only after the mounted editor is ready.
      // Capture its complete revision-bearing target here; a component prop is
      // a view projection and must not be an operation credential.
      target = captureActiveSidecarTarget()
      if (!target) return
      const rightText = normalizeCompareText(getSidecarText())
      const leftText = normalizeCompareText($sourceText)
      const rightLanguage = getSidecarLanguage()
      const compareInput = {
        leftText,
        rightText,
        leftLanguage: languageIdValue,
        rightLanguage,
      }
      runGeneration = compareGeneration + 1
      compareGeneration = runGeneration
      const readySidecarEditor = await ensureSidecarEditorReady()
      diffError = ''
      const data = await callSharedWasmWorker<DiffResponse>('compare', {
        language: languageIdValue,
        leftLanguage: languageIdValue,
        rightLanguage,
        left: leftText,
        right: rightText
      })
      if (!isCurrentCompareInput(compareInput, runGeneration, target)) return
      // A background pair remains document-current and stores its result.
      // Only the visible pair may touch Monaco, toasts, or loading UI.
      if (tabTargetStatus(getWorkspaceState(), target) !== 'current') return
      if (!updateSidecarCompareOutcome(target, data.equal
        ? { kind: 'equal', mode: data.mode }
        : { kind: 'different', mode: data.mode })) return
      if (activeSidecarTabId !== target.tabId) return
      const monaco = readySidecarEditor?.getMonaco()
      if (monaco) {
        const plans = buildDiffPlans(monaco, data.result.pairs ?? [], leftText, rightText, {
          left: data.result.leftFillRanges ?? [],
          right: data.result.rightFillRanges ?? [],
        })
        applyRightDiffPlan(plans.right)
        onApplyDiff(plans.left)
        await waitForDecorationPaint()
      } else {
        clearCompareHighlights()
      }
      if (data.equal) {
        toast.success('Compare completed (no differences)')
      } else {
        toast.warning('Compare completed (differences found)')
      }
      trackEvent('compare_document', { mode: data.mode, result: 'success' });
    } catch {
      if (runGeneration === 0 || compareGeneration !== runGeneration || tabTargetStatus(getWorkspaceState(), target) !== 'current') return
      updateSidecarCompareOutcome(target, { kind: 'none' })
      if (activeSidecarTabId !== target.tabId) return
      diffError = 'Compare failed'
      invalidateCompare()
      trackEvent('compare_document', { result: 'failure' });
    }
  }

  function isCurrentCompareInput(
    input: { leftText: string; rightText: string; leftLanguage: SupportedEditorLanguageId; rightLanguage: SupportedEditorLanguageId },
    generation: number,
    target: TabTarget,
  ): boolean {
    const workspace = getWorkspaceState()
    const sidecar = workspace.tabsById[target.tabId]
    const main = sidecar?.ownerMainTabId ? workspace.tabsById[sidecar.ownerMainTabId] : null
    return tabTargetStatus(workspace, target) === 'current'
      && compareGeneration === generation
      && normalizeCompareText(main?.sourceText ?? '') === input.leftText
      && normalizeCompareText(sidecar?.sourceText ?? '') === input.rightText
      && main?.languageId === input.leftLanguage
      && sidecar?.languageId === input.rightLanguage
  }

  $: languageIdValue = $languageIdStore
  $: if (activeSidecarTabId) {
    const sidecar = $editorWorkspace.tabsById[activeSidecarTabId]
    if (sidecar && sidecar.sourceText !== scratchText) scratchText = sidecar.sourceText
  }
  $: if (lastSourceText === null) {
    lastSourceText = $sourceText
  } else if ($sourceText !== lastSourceText) {
    lastSourceText = $sourceText
    invalidateCompare()
  }
  $: if ($compareEditToken !== lastCompareEditToken) {
    lastCompareEditToken = $compareEditToken
    invalidateCompare()
  }
  $: if (lastCompareLeftLanguage === null) {
    lastCompareLeftLanguage = languageIdValue
  } else if (languageIdValue !== lastCompareLeftLanguage) {
    lastCompareLeftLanguage = languageIdValue
    invalidateCompare()
  }
  $: if (lastCompareRightLanguage === null) {
    lastCompareRightLanguage = scratchLanguage
  } else if (scratchLanguage !== lastCompareRightLanguage) {
    lastCompareRightLanguage = scratchLanguage
    invalidateCompare()
  }
  $: if (effectiveViewMode !== 'text' && hasRightCompareHighlights()) {
    clearCompareHighlights()
  }
  $: if (effectiveViewMode === 'text' && activeSidecarTabId && restoredCompareSidecarTabId !== activeSidecarTabId) {
    restoredCompareSidecarTabId = activeSidecarTabId
    void restoreSidecarScroll(activeSidecarTabId)
  }

  function updateScratchText(payload: { tabId: string; text: string }) {
    if (payload.tabId === activeSidecarTabId) scratchText = payload.text
  }

  function updateScratchLanguage(value: SupportedEditorLanguageId) {
    scratchLanguage = value
  }

  function handleSidecarScroll(payload: { tabId: string; scrollTop: number; scrollLeft: number }): void {
    const target = captureSidecarTarget(payload.tabId)
    if (target) updateSidecarCompareScroll(target, { scrollTop: payload.scrollTop, scrollLeft: payload.scrollLeft })
    onTextScroll({ scrollTop: payload.scrollTop, scrollLeft: payload.scrollLeft })
  }

  async function restoreSidecarScroll(sidecarTabId: string): Promise<void> {
    const sidecar = $editorWorkspace.tabsById[sidecarTabId]
    await ensureSidecarEditorReady()
    if (activeSidecarTabId !== sidecarTabId || !sidecar?.sidecarState) return
    sidecarEditor?.setScrollPosition(sidecar.sidecarState.compare)
  }

  async function loadRightPanelFile(file: File) {
    if (graphOnly) return
    const [sample, text] = await Promise.all([readImportSourceSample(file), file.text()])
    const sourceFormat = await resolveImportSourceFormat(file.name, sample, editorLanguageFallback)
    const nextLanguage = supportedEditorLanguageSet.has(sourceFormat as SupportedEditorLanguageId)
      ? (sourceFormat as SupportedEditorLanguageId)
      : editorLanguageFallback
    await showRightPanelText(text, nextLanguage)
  }

  async function handleRightPanelDrop(event: DragEvent) {
    event.preventDefault()
    if (graphOnly) return
    const files = event.dataTransfer?.files
    if (!files || files.length === 0) return
    await loadRightPanelFile(files[0])
  }

  function handleRightPanelDragOver(event: DragEvent) {
    event.preventDefault()
  }

  function openRightPanelFilePicker() {
    if (graphOnly) return
    rightPanelFileInput?.click()
  }

  async function handleRightPanelFileChange(event: Event) {
    if (graphOnly) return
    const input = event.currentTarget as HTMLInputElement | null
    const file = input?.files?.[0]
    if (!file) return
    await loadRightPanelFile(file)
    input.value = ''
  }

  async function showRightPanelText(
    value: string,
    nextLanguage: SupportedEditorLanguageId = languageIdValue,
  ) {
    if (graphOnly) return
    viewMode = 'text'
    invalidateCompare()
    updateScratchLanguage(nextLanguage)
    const readySidecarEditor = await ensureSidecarEditorReady()
    await readySidecarEditor?.showText(value, nextLanguage)
  }

  export async function showTextPreview(
    value: string,
    nextLanguage: SupportedEditorLanguageId = languageIdValue,
  ): Promise<void> {
    if (graphOnly) return
    await showRightPanelText(value, nextLanguage)
  }

  export async function waitForIdle(): Promise<void> {
    if (graphOnly) return
    const readySidecarEditor = await ensureSidecarEditorReady()
    await readySidecarEditor?.waitForIdle()
  }

  export async function runCompare(): Promise<void> {
    if (graphOnly) return
    await runDiffCompare()
  }

  export function openGraphSearch(): void {
    graphSearchPanel?.openPanel?.()
  }

  export function openCompareFile(): void {
    openRightPanelFilePicker()
  }

  export function swapCompareEditors(): void {
    void onSwap({ rightText: getActiveText(), rightLanguage: getActiveLanguage() })
  }

  export async function compareEditors(): Promise<void> {
    await runDiffCompare()
  }

  export function zoomGraphIn(): void {
    graphViewer?.zoomIn?.()
  }

  export function zoomGraphOut(): void {
    graphViewer?.zoomOut?.()
  }

  export async function exportGraphImage(): Promise<void> {
    await graphViewer?.exportImage?.()
  }

  export function previewGraphSearchResult(result: any): void {
    graphViewer?.previewSearchResult?.(result)
  }

  export function cancelGraphSearchPreview(): Promise<void> {
    return graphViewer?.cancelSearchPreview?.() ?? Promise.resolve()
  }

  export function revealGraphSearchResult(result: any): void {
    graphViewer?.commitSearchPreview?.()
    graphViewer?.revealSearchResult?.(result)
    trackEvent('graph_search', { surface: 'graph', result_count: 1 })
  }

  function handleGraphSearchPreview(result: GraphSearchResult): void {
    graphViewer?.previewSearchResult?.(result)
  }

  function handleGraphSearchCancel(): void {
    void graphViewer?.cancelSearchPreview?.()
  }

  function handleGraphSearchSelect(event: CustomEvent<GraphSearchResult>): void {
    graphViewer?.commitSearchPreview?.()
    revealGraphSearchResult(event.detail)
  }

  function handleGraphNavigation(event: CustomEvent<any>): void {
    const path = event?.detail?.path ?? []
    if (path.length) onGraphNavigation(event.detail)
  }

  export function revealPath(path: PathSeg[], options: { target?: 'key' | 'value' | 'node' } | undefined): Promise<boolean> {
    if (!path.length) return Promise.resolve(false)
    return graphViewer?.revealPath?.(path, { ...options, navigate: true }) ?? Promise.resolve(false)
  }

  export async function waitForGraphReady(): Promise<boolean> {
    return await graphViewer?.waitForGraphReady?.() ?? false;
  }

  export function isGraphInteractive(): boolean {
    return graphViewer?.isGraphInteractive?.() ?? false;
  }

  export function showEntitlementOverlay(block: UsageBlock): void {
    entitlementOverlay = block
    pricingOverlayVisible = true
    onEntitlementBlocked(block)
  }

  export function showPricingOverlay(usageNotice: PricingUsageNotice | null): void {
    entitlementOverlay = null
    pricingOverlayVisible = true
    pricingUsageNotice = usageNotice
  }

  function handleEntitlementBlocked(block: UsageBlock): void {
    showEntitlementOverlay(block)
  }

  export function getColumnNavigatorActivePath(): PathSeg[] {
    return graphViewer?.getColumnNavigatorActivePath?.() ?? [];
  }

  export async function restoreColumnNavigatorPath(path: PathSeg[]): Promise<boolean> {
    return await graphViewer?.restoreColumnNavigatorPath?.(path) ?? false;
  }

  export async function restoreColumnNavigatorNavigationState(state: {
    activePath: PathSeg[];
    history: PathSeg[][];
    historyIndex: number;
    collapsed: boolean;
  }): Promise<void> {
    await graphViewer?.restoreColumnNavigatorNavigationState?.(state);
  }

  export function restoreGraphViewport(state: { x: number; y: number; scaleX: number; scaleY: number } | null): void {
    graphViewer?.restoreGraphViewport?.(state)
  }

  export function collapseColumnNavigator(): void {
    graphViewer?.collapseColumnNavigator?.();
  }

  export function expandColumnNavigator(): void {
    graphViewer?.expandColumnNavigator?.();
  }

  export function pinColumnNavigatorCollapsed(): void {
    graphViewer?.pinColumnNavigatorCollapsed?.();
  }

  export async function goColumnNavigatorBack(): Promise<void> {
    await graphViewer?.goColumnNavigatorBack?.();
  }

  export async function goColumnNavigatorForward(): Promise<void> {
    await graphViewer?.goColumnNavigatorForward?.();
  }

  export async function selectColumnNavigatorPath(path: PathSeg[]): Promise<void> {
    await graphViewer?.selectColumnNavigatorPath?.(path);
  }

  export async function applyColumnNavigatorNavigationPath(path: PathSeg[]): Promise<void> {
    await graphViewer?.applyColumnNavigatorNavigationPath?.(path);
  }

  export function setTextScrollPosition(position: { scrollTop: number; scrollLeft: number }) {
    if (graphOnly) return
    sidecarEditor?.setScrollPosition(position)
  }

  export function getViewportAnchor(): { topLine: number; scrollLeft: number } | null {
    return sidecarEditor?.getViewportAnchor() ?? null
  }

  export function restoreViewportAnchor(anchor: { topLine: number; scrollLeft: number }): void {
    sidecarEditor?.restoreViewportAnchor(anchor)
  }

  export function getActiveText(): string {
    if (graphOnly) return $sourceText
    return getSidecarText()
  }

  export function getActiveLanguage(): SupportedEditorLanguageId {
    if (graphOnly) return languageIdValue
    return getSidecarLanguage()
  }
</script>

<div
  class="relative grid h-full w-full bg-[var(--panel-bg-alt)]"
  data-testid="right-panel-dropzone"
  data-compare-highlight-count={rightCompareHighlightCount}
  role="region"
  aria-label="Right editor panel"
  on:drop={handleRightPanelDrop}
  on:dragover={handleRightPanelDragOver}
>
  {#if !graphOnly}
    <input
      class="hidden"
      type="file"
      bind:this={rightPanelFileInput}
      aria-label="Right panel file input"
      on:change={handleRightPanelFileChange}
    />
  {/if}
  {#if !hideGraphToolbar}
  <div class="absolute right-3 top-2.5 z-[2] inline-flex flex-col items-end gap-2.5">
    {#if effectiveViewMode === 'graph' && (!synchronizedRuntimeLoading || graphOnly)}
      <ButtonGroup.Root
        orientation="vertical"
        variant="segmented-outline"
        class="shadow-none"
      >
        <div class="relative" data-button-group-item>
            <IconButton
              class="text-[var(--text-primary)]"
              aria-label="Search graph"
              disabled={emptyDocument}
            title="Search"
            data-testid="graph-search-trigger"
            on:click={() => {
              if (graphViewer?.isGraphInteractive?.()) graphSearchPanel?.openPanel?.()
            }}
          >
            <Search size={12} />
          </IconButton>
          <GraphSearchPanel
            bind:this={graphSearchPanel}
            documentKey={$documentKeyStore}
            language={languageIdValue}
            text={$sourceText}
            panelClass="absolute right-0 top-[calc(100%+8px)]"
            previewResultCallback={handleGraphSearchPreview}
            cancelCallback={handleGraphSearchCancel}
            on:select={handleGraphSearchSelect}
          />
        </div>
        <IconButton
          class="text-[var(--text-primary)]"
          aria-label="Zoom in"
          disabled={emptyDocument}
          title="Zoom in"
          data-testid="zoom-in-button"
          on:click={() => graphViewer?.zoomIn?.()}
        >
          <ZoomIn size={12} />
        </IconButton>
        <IconButton
          class="text-[var(--text-primary)]"
          aria-label="Zoom out"
          disabled={emptyDocument}
          title="Zoom out"
          data-testid="zoom-out-button"
          on:click={() => graphViewer?.zoomOut?.()}
        >
          <ZoomOut size={12} />
        </IconButton>
        <IconButton
          class="text-[var(--text-primary)]"
          aria-label="Export image"
          title="Export image"
          disabled={emptyDocument}
          on:click={() => graphViewer?.exportImage?.()}
        >
          <ImageDown size={12} />
        </IconButton>
      </ButtonGroup.Root>
    {:else if !graphOnly}
      <ButtonGroup.Root
        orientation="vertical"
        variant="segmented-outline"
        class="shadow-none"
      >
        <IconButton
          class="text-[var(--text-primary)]"
          aria-label="Load compare file"
          title="Load compare file"
          on:click={openRightPanelFilePicker}
        >
          <FileInput size={12} />
        </IconButton>
        <IconButton
          class="text-[var(--text-primary)]"
          aria-label="Swap editors"
          title="Swap editors"
          on:click={() => onSwap({ rightText: getActiveText(), rightLanguage: getActiveLanguage() })}
        >
          <ArrowRightLeft size={12} />
        </IconButton>
        <IconButton
          class="text-[var(--text-primary)]"
          aria-label="Run comparison"
          title="Run comparison"
          on:click={runDiffCompare}
        >
          <GitCompareArrows size={12} />
        </IconButton>
      </ButtonGroup.Root>
    {/if}
  </div>
  {/if}

  {#if effectiveViewMode === 'graph' && visibleGraphDiagnostics.length}
    <div class="pointer-events-auto absolute left-3 right-[140px] top-4 z-[1] flex flex-col gap-2.5">
      {#each visibleGraphDiagnostics as diag}
        <button
          data-testid={diag.code === 'syntax-error' ? 'graph-diagnostic-syntax-error' : 'graph-diagnostic-missing-node'}
          data-diagnostic-code={diag.code}
          class="relative cursor-pointer overflow-hidden rounded-[12px] border border-[rgba(203,42,47,0.25)] bg-white px-3 py-2.5 text-left text-[var(--text-primary)] shadow-[0_8px_24px_rgba(15,23,42,0.08)] w-fit"
          on:click={() => onRevealError(diag.startLineNumber, diag.startColumn)}
        >
          <div class="flex items-start justify-start gap-2 text-[12px]">
            <span class="relative z-[1] inline-flex min-w-0 items-baseline gap-2 leading-[1.4]">
              {diag.message}
            </span>
          </div>
          <div class="mt-2 flex flex-col gap-1 font-mono text-[11px]">
            {#each diag.context as line}
              <div
                class={`grid grid-cols-[36px_1fr] gap-2 ${
                  line.lineNumber === diag.startLineNumber ? 'bg-[#ff9b9533]' : 'text-[var(--text-muted)]'
                }`}
              >
                <span class="text-right">{line.lineNumber}</span>
                <span class="break-words whitespace-pre-wrap">{@html renderContextLineHtml(line.text, line.lineNumber === diag.startLineNumber, diag.startColumn, diag.endColumn)}</span>
              </div>
            {/each}
          </div>
        </button>
      {/each}
    </div>
  {/if}

  {#if !graphOnly && effectiveViewMode === 'text'}
    <div
      class="absolute inset-0 flex h-full min-h-0 min-w-0 flex-col bg-[var(--panel-bg)]"
    >
      {#if diffError}
        <div class="border-b border-[var(--border-muted)] px-3 py-2 text-[12px] text-[#f87171]">
          {diffError}
        </div>
      {/if}
      {#if activeSidecarTabId}
        {#key activeSidecarTabId}
          <SidecarEditor
            bind:this={sidecarEditor}
            tabId={activeSidecarTabId}
            language={activeSidecarLanguage}
            placeholderTitle="Enter content to compare"
            onScroll={handleSidecarScroll}
            onContentChange={(payload) => {
              const target = captureSidecarTarget(payload.tabId)
              updateScratchText(payload)
              invalidateCompare(target)
            }}
            onRequestImportFile={() => openRightPanelFilePicker()}
          />
        {/key}
      {/if}
    </div>
  {/if}

  <div
    class="absolute inset-0 h-full min-h-0 min-w-0 w-full"
    class:invisible={!graphOnly && effectiveViewMode !== 'graph'}
    class:pointer-events-none={!graphOnly && effectiveViewMode !== 'graph'}
    aria-hidden={!graphOnly && effectiveViewMode !== 'graph'}
  >
    {#key sidecarTabId || 'no-sidecar'}
      <GraphViewer
        bind:this={graphViewer}
        {sidecarTabId}
        active={graphOnly || effectiveViewMode === 'graph'}
        {synchronizedRuntimeLoading}
        readonly={readonlyGraph}
        {onFileDrop}
        {onRequestImportFile}
        {onLoadExample}
        onEntitlementBlocked={handleEntitlementBlocked}
        {ensureSharedWorkspacePromoted}
        on:navigation={handleGraphNavigation}
        on:runtime-state={handleGraphViewerRuntimeState}
        on:column-navigator-state={(event) => onColumnNavigatorState(event.detail)}
        on:graph-viewport-state={(event) => onGraphViewportState(event.detail)}
      />
    {/key}
  </div>
  {#if pricingOverlayVisible}
    <EntitlementOverlay
      block={entitlementOverlay}
      {pricingPlanGridComponent}
      usageNotice={pricingUsageNotice}
      onSelectPlan={onPricingSelectPlan}
      actionDisabled={pricingActionDisabled}
      actionLabel={pricingActionLabel}
    />
  {/if}
</div>
