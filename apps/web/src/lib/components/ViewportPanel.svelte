<script lang="ts">
  import { tick } from 'svelte'
  import { toast } from 'svelte-sonner'
  import {
    sourceText,
    compareEditToken,
    languageId as languageIdStore,
    activeTempModel,
    documentKey as documentKeyStore,
    jsonBlockSelection
  } from '../store/editor-store'
  import type { PathSeg } from '../store/tree-path'
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
  import GraphSearchInput from './GraphSearchInput.svelte'
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

  export let viewMode: 'graph' | 'text' = 'graph'
  export let onRevealError: (line: number, column: number) => void = () => {}
  export let onGraphReveal: (payload: { path: PathSeg[]; target?: 'key' | 'value' | 'node'; trigger?: 'click' | 'search' }) => void =
    () => {}
  export let onApplyDiff: (plan: DiffPlan) => void = () => {}
  export let onTextScroll: (payload: { scrollTop: number; scrollLeft: number }) => void = () => {}
  export let onSwap: (payload: { rightText: string; rightLanguage: SupportedEditorLanguageId }) => void = () => {}
  export let onGraphRuntimeState: (payload: RuntimeStateEventDetail) => void = () => {}
  export let enableRevealSync = true
  export let synchronizedRuntimeLoading = false
  export let graphOnly = false
  export let readonlyGraph = false

  type DiffResponse = {
    mode: 'tree' | 'text'
    equal: boolean
    result: { pairs: DiffPair[] }
  }

  let diffError = ''
  let rightCompareHighlightCount = 0
  let languageIdValue: SupportedEditorLanguageId = editorLanguageFallback
  let scratchText = ''
  let lastSourceText: string | null = null
  let lastCompareEditToken: number | null = null
  let rightPanelFileInput: HTMLInputElement | null = null
  let sidecarEditor: SidecarEditor | null = null
  let graphViewer: any = null
  let graphSearchInput: GraphSearchInput | null = null
  let effectiveViewMode: 'graph' | 'text' = 'graph'
  $: visibleGraphDiagnostics = $jsonBlockSelection ? [] : ($activeTempModel?.diagnostics ?? []).slice(0, 2)
  $: effectiveViewMode = graphOnly ? 'graph' : viewMode

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

  function getSidecarText(): string {
    return sidecarEditor?.getText() ?? scratchText
  }

  function getSidecarLanguage(): SupportedEditorLanguageId {
    return sidecarEditor?.getLanguage() ?? languageIdValue
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

  function applyRightDiffPlan(plan: DiffPlan) {
    rightCompareHighlightCount = sidecarEditor?.applyDiffPlan(plan) ?? 0
  }

  async function runDiffCompare() {
    if (graphOnly || effectiveViewMode !== 'text') return
    try {
      const readySidecarEditor = await ensureSidecarEditorReady()
      const rightText = normalizeCompareText(readySidecarEditor?.getText() ?? scratchText)
      const leftText = normalizeCompareText($sourceText)
      const rightLanguage = getSidecarLanguage()
      diffError = ''
      const data = await callSharedWasmWorker<DiffResponse>('compare', {
        language: languageIdValue,
        leftLanguage: languageIdValue,
        rightLanguage,
        left: leftText,
        right: rightText
      })
      const monaco = readySidecarEditor?.getMonaco()
      if (monaco) {
        const plans = buildDiffPlans(monaco, data.result.pairs ?? [], leftText, rightText)
        applyRightDiffPlan(plans.right)
        onApplyDiff(plans.left)
      } else {
        clearCompareHighlights()
      }
      if (data.equal) {
        toast.success('Compare completed (no differences)')
      } else {
        toast.warning('Compare completed (differences found)')
      }
    } catch {
      diffError = 'Compare failed'
      clearCompareHighlights()
    }
  }

  $: languageIdValue = $languageIdStore
  $: if ($activeTempModel && $activeTempModel.scratchText !== scratchText) scratchText = $activeTempModel.scratchText
  $: if (lastSourceText === null) {
    lastSourceText = $sourceText
  } else if ($sourceText !== lastSourceText) {
    lastSourceText = $sourceText
    clearCompareHighlights()
  }
  $: if ($compareEditToken !== lastCompareEditToken) {
    lastCompareEditToken = $compareEditToken
    if (hasRightCompareHighlights()) {
      clearCompareHighlights()
    }
  }
  $: if (effectiveViewMode !== 'text' && hasRightCompareHighlights()) {
    clearCompareHighlights()
  }

  function updateScratchText(value: string) {
    scratchText = value
    activeTempModel.update((current) => ({ ...current, scratchText: value }))
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
    clearCompareHighlights()
    updateScratchText(value)
    const readySidecarEditor = await ensureSidecarEditorReady()
    await readySidecarEditor?.showText(value, nextLanguage)
  }

  export function showTextPreview(value: string) {
    if (graphOnly) return
    void showRightPanelText(value)
  }

  function handleGraphSearchSelect(event: CustomEvent<any>): void {
    graphViewer?.revealSearchResult?.(event.detail)
  }

  function handleGraphReveal(event: CustomEvent<any>): void {
    const path = event?.detail?.path ?? []
    if (path.length) onGraphReveal(event.detail)
  }

  export function revealPath(path: PathSeg[], options?: { target?: 'key' | 'value' | 'node' }) {
    if (!path.length) return
    graphViewer?.revealPath?.(path, options)
  }

  export function setTextScrollPosition(position: { scrollTop: number; scrollLeft: number }) {
    if (graphOnly) return
    sidecarEditor?.setScrollPosition(position)
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
            title="Search"
            data-testid="graph-search-trigger"
            on:click={() => graphSearchInput?.openPanel?.()}
          >
            <Search size={12} />
          </IconButton>
          <GraphSearchInput
            bind:this={graphSearchInput}
            documentKey={$documentKeyStore}
            language={languageIdValue}
            text={$sourceText}
            panelClass="absolute right-0 top-[calc(100%+8px)]"
            on:select={handleGraphSearchSelect}
          />
        </div>
        <IconButton
          class="text-[var(--text-primary)]"
          aria-label="Zoom in"
          title="Zoom in"
          data-testid="zoom-in-button"
          on:click={() => graphViewer?.zoomIn?.()}
        >
          <ZoomIn size={12} />
        </IconButton>
        <IconButton
          class="text-[var(--text-primary)]"
          aria-label="Zoom out"
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
          aria-label="Compare"
          title="Compare"
          on:click={runDiffCompare}
        >
          <GitCompareArrows size={12} />
        </IconButton>
      </ButtonGroup.Root>
    {/if}
  </div>

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
    <div class="flex h-full min-h-0 min-w-0 flex-col bg-[var(--panel-bg)]">
      {#if diffError}
        <div class="border-b border-[var(--border-muted)] px-3 py-2 text-[12px] text-[#f87171]">
          {diffError}
        </div>
      {/if}
      <SidecarEditor
        bind:this={sidecarEditor}
        language={languageIdValue}
        onScroll={onTextScroll}
        onContentChange={clearCompareHighlights}
      />
    </div>
  {:else}
    <div class="h-full min-h-0 min-w-0 w-full">
      <GraphViewer
        bind:this={graphViewer}
        {enableRevealSync}
        {synchronizedRuntimeLoading}
        readonly={readonlyGraph}
        on:reveal={handleGraphReveal}
        on:runtime-state={handleGraphViewerRuntimeState}
      />
    </div>
  {/if}
</div>
