<script lang="ts">
  import { tick, createEventDispatcher } from 'svelte'
  import SearchPanel from './SearchPanel.svelte'
  import { callSharedWasmWorker } from '../wasm/wasm-worker-singleton'
  import { settings } from '../settings/settings-store'
  import type { PathSeg } from '../store/tree-path'
  import { editorRevision, graphAppliedRevision } from '../store/document-session-store'
  import { getWorkspaceSnapshotId } from '../store/workspace-store'

  type GraphSearchResult = {
    nodeId?: number
    target: 'node' | 'key' | 'value'
    label: string
    path: PathSeg[]
    pathText: string
  }
  type GraphSearchReadResult =
    | { status: 'ready'; data: GraphSearchResult[] }
    | { status: 'snapshotNotReady' }

  export let documentKey = ''
  export let language = ''
  export let text = ''
  export let shortcut = '⌘ F'
  export let panelClass = 'absolute right-0 top-[calc(100%+8px)]'
  export let onOpenChange: (open: boolean) => void = () => {}
  export let previewResultCallback: (result: GraphSearchResult) => void = () => {}
  export let cancelCallback: () => void = () => {}

  let open = false
  let query = ''
  let activeIndex = -1
  let results: GraphSearchResult[] = []
  let searchPanel: SearchPanel | null = null
  let panelRef: HTMLDivElement | null = null
  let resultsList: HTMLDivElement | null = null
  let debounceHandle: ReturnType<typeof setTimeout> | null = null
  let searchToken = 0
  let lastSearchDependencySignature = ''
  let resolvedQuery = ''
  let lastPreviewedResultKey = ''
  let hasPreviewedResults = false

  const dispatch = createEventDispatcher()

  /**
   * Open the search panel and focus the input.
   * @returns Promise<void>
   */
  export async function openPanel(): Promise<void> {
    open = true
    lastPreviewedResultKey = ''
    hasPreviewedResults = false
    onOpenChange(true)
    activeIndex = results.length ? 0 : -1
    previewResult(activeIndex)
    await tick()
  searchPanel?.focusInput()
  }

  /**
   * Close the search panel.
   * @returns void
   */
  export function closePanel(restorePreview = true): void {
    if (restorePreview && open && hasPreviewedResults) {
      cancelCallback()
      dispatch('cancel')
    }
    open = false
    onOpenChange(false)
  }

  /**
   * Update the query string and trigger a search.
   * @param next Latest query value
   * @returns void
   */
  function setQuery(next: string): void {
    // Invalidate an in-flight request immediately, not only when the next
    // debounced request starts. A stale result must never replace this list.
    searchToken += 1
    query = next
    lastPreviewedResultKey = ''
    const trimmed = next.trim()
    resolvedQuery = trimmed ? '' : resolvedQuery
    activeIndex = -1
    results = []
    scheduleSearch(next)
  }

  /**
   * Trigger a delayed search.
   * @param next Search keyword
   * @returns void
   */
  function scheduleSearch(next: string): void {
    if (debounceHandle) clearTimeout(debounceHandle)
    const trimmed = next.trim()
    if (!trimmed) {
      results = []
      return
    }
    debounceHandle = setTimeout(() => {
      void runSearch(trimmed)
    }, 120)
  }

  /**
   * Execute a search request.
   * @param keyword Search keyword
   * @returns Promise<void>
   */
  async function runSearch(keyword: string): Promise<void> {
    if (!documentKey || !language) {
      results = []
      return
    }
    const token = (searchToken += 1)
    try {
      const result = await callSharedWasmWorker<GraphSearchReadResult>('graphSearch', {
        documentKey,
        snapshotId: getWorkspaceSnapshotId(documentKey),
        language,
        query: keyword,
        nest: $settings.parser.enableNest
      })
      if (token !== searchToken) return
      results = result.status === 'ready' ? result.data : []
      resolvedQuery = keyword
      activeIndex = results.length ? 0 : -1
      previewResult(activeIndex)
    } catch {
      if (token !== searchToken) return
      results = []
      resolvedQuery = keyword
    }
  }

  /**
   * Handle a selected search result.
   * @param item Selected result
   * @returns void
   */
  function resultKey(item: GraphSearchResult): string {
    return `${item.pathText}|${item.target}|${item.nodeId ?? 'unresolved'}|${item.label}`
  }

  function previewResult(index: number): void {
    const item = results[index]
    if (!item) return
    const key = resultKey(item)
    if (key === lastPreviewedResultKey) return
    lastPreviewedResultKey = key
    hasPreviewedResults = true
    previewResultCallback(item)
    dispatch('preview', item)
  }

  /**
   * The single state transition for mouse and keyboard navigation. Both paths
   * preview through the same graph-navigation callback; only keyboard asks the
   * result list to follow the active option.
   */
  function activateResult(index: number, scrollIntoView = false): void {
    if (!results[index]) return
    activeIndex = index
    previewResult(index)
    if (scrollIntoView) void scrollActiveResultIntoView(index)
  }

  async function scrollActiveResultIntoView(index: number): Promise<void> {
    await tick()
    resultsList
      ?.querySelector<HTMLElement>(`[data-graph-search-index="${index}"]`)
      ?.scrollIntoView({ block: 'nearest' })
  }

  function moveActiveResult(offset: number): void {
    if (!results.length) return
    const nextIndex = (activeIndex + offset + results.length) % results.length
    activateResult(nextIndex, true)
  }

  function handleSearchKeydown(event: KeyboardEvent): void {
    if (event.key === 'ArrowDown') {
      event.preventDefault()
      moveActiveResult(1)
      return
    }
    if (event.key === 'ArrowUp') {
      event.preventDefault()
      moveActiveResult(-1)
      return
    }
    if (event.key === 'Enter') {
      const item = results[activeIndex]
      if (item && query.trim() === resolvedQuery) {
        event.preventDefault()
        selectResult(item)
      }
      return
    }
    if (event.key === 'Escape') {
      event.preventDefault()
      closePanel()
    }
  }

  function selectResult(item: GraphSearchResult): void {
    dispatch('select', item)
    closePanel(false)
  }

  /**
   * Respond to global keyboard shortcuts.
   * @param event Keyboard event
   * @returns void
   */
  function handleGlobalKey(event: KeyboardEvent): void {
    if (open && event.key === 'Escape') {
      event.preventDefault()
      closePanel()
      return
    }
    if (!((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'f')) return
    const target = event.target as HTMLElement | null
    if (target?.closest('.monaco-editor')) return
    event.preventDefault()
    if (!open) void openPanel()
  }

  /**
   * Close the panel when the user clicks outside it.
   * @param event Pointer event
   * @returns void
   */
  function handleDocumentPointerDown(event: PointerEvent): void {
    if (!open || !panelRef) return
    const path = typeof event.composedPath === 'function' ? event.composedPath() : []
    if (path.includes(panelRef)) return
    closePanel()
  }

  $: if (!query.trim()) {
    lastSearchDependencySignature = ''
  }

  $: {
    const dependencySignature = documentKey && text && language && $graphAppliedRevision >= $editorRevision
      ? `${documentKey}:${language}:${text.length}:${$settings.parser.enableNest}:${$graphAppliedRevision}`
      : ''
    if (query.trim() && dependencySignature && dependencySignature !== lastSearchDependencySignature) {
      lastSearchDependencySignature = dependencySignature
      void runSearch(query.trim())
    }
  }

</script>

<svelte:window on:keydown|capture={handleGlobalKey} on:pointerdown={handleDocumentPointerDown} />

<SearchPanel
  bind:this={searchPanel}
  bind:containerRef={panelRef}
  {open}
  {query}
  placeholder="Search graph"
  inputAriaLabel="Search graph"
  inputTestId="graph-search-input"
  {shortcut}
  panelClass={`${panelClass} z-40 w-[320px]`}
  panelStyle="transform-origin: top right;"
  listClassName="graph-search-list"
  showWhenClosed={false}
  inputInline={true}
  customResults={true}
  onInput={(event: any) => {
    const detail = event.detail as InputEvent
    setQuery((detail.target as HTMLInputElement).value)
  }}
  onKeydown={(event: any) => handleSearchKeydown(event.detail as KeyboardEvent)}
>
  <svelte:fragment slot="results">
    <div
      bind:this={resultsList}
      id="graph-search-results"
      class="graph-search-list max-h-[300px] overflow-y-auto p-1"
      role="listbox"
      aria-label="Graph search results"
    >
      {#if results.length}
        {#each results as item, index (resultKey(item))}
          <div
            id={`graph-search-result-${index}`}
            role="option"
            tabindex="-1"
            aria-label={`Graph search result ${item.pathText}`}
            aria-selected={activeIndex === index}
            data-graph-search-index={index}
            data-testid={`graph-search-result-${item.nodeId ?? 'unresolved'}-${item.pathText}`}
            class:graph-search-result--active={activeIndex === index}
            class="graph-search-result flex h-[40px] w-full cursor-default select-none items-center gap-2 rounded-[8px] px-2.5 py-1.5 text-left text-[13px] outline-none"
            on:mouseenter={() => activateResult(index)}
            on:click={() => {
              activateResult(index)
              selectResult(item)
            }}
            on:keydown={(event) => {
              if (event.key === 'Enter' || event.key === ' ') {
                event.preventDefault()
                activateResult(index)
                selectResult(item)
              }
            }}
          >
            <div class="flex min-w-0 flex-1 flex-col gap-0">
              <span class="truncate text-[13px] leading-[16px]">{item.label}</span>
              <div class="left-truncate text-[11px] leading-[12px] text-[var(--text-muted)]">
                <span class="left-truncate-content">{item.pathText}</span>
              </div>
            </div>
          </div>
        {/each}
      {:else}
        <div class="px-3 py-6 text-center text-[13px] text-[var(--text-muted)]">No results.</div>
      {/if}
    </div>
  </svelte:fragment>
</SearchPanel>

<style>
  .graph-search-result--active {
    background-color: #eef2f7;
    color: var(--text-primary);
  }

  .graph-search-result:not(.graph-search-result--active):hover {
    background-color: #f7f9fc;
  }
</style>
