<script lang="ts">
  import { tick, createEventDispatcher } from 'svelte'
  import fuzzysort from 'fuzzysort'
  import SearchPanel from './SearchPanel.svelte'
  import { commandItems, type CommandId } from '../command-registry'
  import { Wand2, Shrink, ArrowDownWideNarrow, Braces, WrapText, Code, Check } from 'lucide-svelte'
  import { settings, settingsStore } from '../settings/settings-store'
  import { languageId } from '../store/editor-store'

  export let value = ''
  export let onExecute: (id: CommandId) => void | Promise<void> = () => {}

  let open = false
  let query = ''
  let activeIndex = 0
  let searchPanel: SearchPanel | null = null
  let panelRef: HTMLDivElement | null = null
  let results: Array<{ id: CommandId; label: string; keywords: string[]; keywordText: string; type?: string }> = []
  
  const iconMap: Record<string, typeof Wand2> = {
    format: Wand2,
    minify: Shrink,
    sort: ArrowDownWideNarrow,
    'show-yq-input': Braces,
    escape: WrapText,
    unescape: Code,
  }

  $: commandSource = commandItems
    .filter((item) => item.langs.includes('*') || item.langs.includes($languageId))
    .map((item) => ({
      ...item,
      keywordText: item.keywords.join(' ')
    }))

  const dispatch = createEventDispatcher()

  function setQuery(next: string) {
    query = next
    dispatch('input', next)
  }

  function closePanel() {
    open = false
  }

  async function openPanel() {
    open = true
    activeIndex = 0
    await tick()
    searchPanel?.focusInput?.()
  }

  function handleGlobalKey(event: KeyboardEvent) {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
      event.preventDefault()
      if (!open) void openPanel()
      else closePanel()
    }
  }

  function handleDocumentPointerDown(event: PointerEvent) {
    const target = event.target as Node
    if (!panelRef || panelRef.contains(target)) return
    closePanel()
  }

  async function executeCommand(id: CommandId) {
    closePanel()
    if (id === 'toggle-nest') {
      await settingsStore.save({ parser: { enableNest: !$settings.parser.enableNest } })
      return
    }
    if (id === 'toggle-auto-format') {
      const current = $settings.formatting
      await settingsStore.save({ formatting: { ...current, smart: !current.smart } })
      return
    }
    await onExecute(id)
  }

  $: if (value !== query) query = value
  $: results = query
    ? (fuzzysort.go(query, commandSource as any, { keys: ['label', 'keywordText'] } as any) as any).map((result: any) => result.obj)
    : commandSource

  $: if (activeIndex >= results.length) activeIndex = Math.max(0, results.length - 1)
</script>

<svelte:window on:keydown={handleGlobalKey} on:pointerdown={handleDocumentPointerDown} />

<SearchPanel
  bind:this={searchPanel}
  bind:containerRef={panelRef}
  {open}
  {query}
  placeholder="Search Command"
  inputAriaLabel="Search command"
  inputTestId="command-search-input"
  shortcut="⌘ K"
  panelClass="absolute left-0 bottom-[calc(100%+8px)] z-40 w-[280px]"
  commandClassName="rounded-[10px] border-[var(--border-muted)] shadow-[0_8px_24px_rgba(15,23,42,0.08)] bg-[var(--panel-bg)]"
  inputClassName="h-7 rounded-[10px] border border-[var(--border-muted)] bg-[var(--panel-bg)] px-3"
  listClassName="command-search-list"
  emptyText="No results."
  showWhenClosed={true}
  inputInline={false}
  {activeIndex}
  {results}
  useVirtualList={false}
  onFocus={() => openPanel()}
  onClick={() => openPanel()}
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
      const item = results[activeIndex]
      if (item) void executeCommand(item.id)
    }
    if (keyEvent.key === 'Escape') {
      keyEvent.preventDefault()
      closePanel()
    }
  }}
  onItemHover={(index) => (activeIndex = index)}
  onItemSelect={(index) => {
    const item = results[index]
    if (item) void executeCommand(item.id)
  }}
  itemKey={(item) => item.id}
>
  <svelte:fragment slot="item" let:item let:index>
    <div class="flex items-center gap-2">
      {#if item.type === 'toggle'}
        {#if item.id === 'toggle-nest' && $settings.parser.enableNest || item.id === 'toggle-auto-format' && $settings.formatting.smart}
          <Check size={14} class="text-accent shrink-0" />
        {:else}
          <div class="w-[14px] shrink-0"></div>
        {/if}
      {:else if iconMap[item.id]}
        <svelte:component this={iconMap[item.id]} size={14} class="shrink-0" />
      {/if}
      <span>{item.label}</span>
    </div>
  </svelte:fragment>
</SearchPanel>
