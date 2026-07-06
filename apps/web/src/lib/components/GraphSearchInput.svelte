<script lang="ts">
  import { tick, createEventDispatcher } from 'svelte'
  import SearchPanel from './SearchPanel.svelte'
  import { callSharedWasmWorker } from '../wasm/wasm-worker-singleton'
  import { settings } from '../settings/settings-store'
  import type { PathSeg } from '../store/tree-path'
  import { editorRevision, graphAppliedRevision } from '../store/document-session-store'
  import { getWorkspaceSnapshotId } from '../store/workspace-snapshot-bindings'

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
  export let panelClass = 'absolute left-0 top-[calc(100%+8px)]'

  let open = false
  let query = ''
  let activeIndex = 0
  let results: GraphSearchResult[] = []
  let searchPanel: SearchPanel | null = null
  let panelRef: HTMLDivElement | null = null
  let debounceHandle: ReturnType<typeof setTimeout> | null = null
  let searchToken = 0
  let lastSearchDependencySignature = ''
  let resolvedQuery = ''

  const dispatch = createEventDispatcher()

  /**
   * 打开搜索面板并聚焦输入框。
   * @returns Promise<void>
   */
  export async function openPanel(): Promise<void> {
    open = true
    activeIndex = 0
    await tick()
    searchPanel?.focusInput?.()
  }

  /**
   * 关闭搜索面板。
   * @returns void
   */
  export function closePanel(): void {
    open = false
  }

  /**
   * 更新查询字符串并触发搜索。
   * @param next 最新查询值
   * @returns void
   */
  function setQuery(next: string): void {
    query = next
    const trimmed = next.trim()
    resolvedQuery = trimmed ? '' : resolvedQuery
    activeIndex = 0
    results = []
    scheduleSearch(next)
  }

  /**
   * 延迟触发搜索。
   * @param next 搜索关键字
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
   * 执行搜索请求。
   * @param keyword 搜索关键字
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
      activeIndex = 0
    } catch {
      if (token !== searchToken) return
      results = []
      resolvedQuery = keyword
    }
  }

  /**
   * 处理选中搜索结果。
   * @param item 选中的结果
   * @returns void
   */
  function selectResult(item: GraphSearchResult): void {
    dispatch('select', item)
    closePanel()
  }

  /**
   * 响应全局快捷键。
   * @param event 键盘事件
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
   * 监听点击面板外部以关闭。
   * @param event 指针事件
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

  $: if (activeIndex >= results.length) activeIndex = Math.max(0, results.length - 1)
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
  panelFrameClass="overflow-hidden rounded-[14px] border border-[var(--border-muted)] bg-white/95 shadow-[0_12px_32px_rgba(15,23,42,0.12)] backdrop-blur"
  commandClassName="border-none bg-transparent px-2 pb-2 pt-1 shadow-none"
  listClassName="graph-search-list"
  emptyText="No results."
  showWhenClosed={false}
  inputInline={true}
  inputClassName="h-9 rounded-none border-0 border-b border-[var(--border-muted)] bg-transparent px-3 text-[13px] shadow-none"
  {activeIndex}
  {results}
  useVirtualList={false}
  estimateSize={56}
  itemAriaLabel={(item: GraphSearchResult) => `Graph search result ${item.pathText}`}
  itemTestId={(item: GraphSearchResult) => `graph-search-result-${item.nodeId ?? 'unresolved'}-${item.pathText}`}
  onInput={(event: any) => {
    const detail = event.detail as InputEvent
    setQuery((detail.target as HTMLInputElement).value)
  }}
  onKeydown={(event: any) => {
    const keyEvent = event.detail as KeyboardEvent
    if (!open && keyEvent.key === 'Enter') {
      void openPanel()
      return
    }
    if (!open) return
    if (keyEvent.key === 'ArrowDown') {
      keyEvent.preventDefault()
      activeIndex = Math.min(results.length - 1, activeIndex + 1)
    }
    if (keyEvent.key === 'ArrowUp') {
      keyEvent.preventDefault()
      activeIndex = Math.max(0, activeIndex - 1)
    }
    if (keyEvent.key === 'Enter') {
      keyEvent.preventDefault()
      if (query.trim() !== resolvedQuery) return
      const item = results[activeIndex]
      if (item) selectResult(item)
    }
    if (keyEvent.key === 'Escape') {
      keyEvent.preventDefault()
      closePanel()
    }
  }}
  onItemHover={(index) => (activeIndex = index)}
  onItemSelect={(index) => {
    const item = results[index]
    if (item) selectResult(item)
  }}
>
  <svelte:fragment slot="item" let:item let:index>
    <div class="flex min-w-0 flex-1 flex-col gap-1">
      <span class="truncate text-[13px]">{item?.label}</span>
      <div class="left-truncate text-[11px] text-[var(--text-muted)]">
        <span class="left-truncate-content">{item?.pathText}</span>
      </div>
    </div>
  </svelte:fragment>
</SearchPanel>
