<script lang="ts">
  import { tick, createEventDispatcher } from 'svelte'
  import fuzzysort from 'fuzzysort'
  import SearchPanel from './SearchPanel.svelte'
  import { commandItems, type CommandId } from '../command-registry'
  import { Wand2, Shrink, Minimize2, ArrowDownWideNarrow, ListFilter, WrapText, Code, Check, Info } from 'lucide-svelte'
  import { settings, settingsStore } from '../settings/settings-store'
  import { languageId } from '../store/document-session-store'

  export let value = ''
  export let onExecute: (id: CommandId) => void | Promise<void> = () => {}

  let open = false
  let query = ''
  let activeIndex = 0
  let searchPanel: SearchPanel | null = null
  let panelRef: HTMLDivElement | null = null
  let results: Array<{ id: CommandId; label: string; keywords: string[]; keywordText: string; description?: string; type?: string }> = []
  
  const iconMap: Record<string, typeof Wand2> = {
    format: Wand2,
    minify: Shrink,
    compact: Minimize2,
    sort: ArrowDownWideNarrow,
    'show-yq-input': ListFilter,
    'generate-struct': Code,
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

  function isToggleEnabled(id: CommandId) {
    if (id === 'toggle-nest') return $settings.parser.enableNest
    if (id === 'toggle-auto-format') return $settings.formatting.smart
    return false
  }

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
  commandClassName="rounded-[10px] border-[rgba(15,23,42,0.10)] shadow-[0_12px_28px_rgba(15,23,42,0.10)] bg-[var(--panel-bg)]"
  inputClassName="h-[23px] rounded-[7px] border border-[rgba(15,23,42,0.10)] bg-[var(--panel-bg)] px-2 text-[#6b7280] transition-colors hover:border-[rgba(15,23,42,0.16)] focus-within:border-[rgba(15,23,42,0.16)]"
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
        <span
          class={`flex h-[14px] w-[14px] shrink-0 items-center justify-center rounded-[4px] border ${
            isToggleEnabled(item.id)
              ? 'border-[#2563eb] bg-[#eff6ff] text-[#2563eb]'
              : 'border-[rgba(15,23,42,0.16)] bg-white text-transparent'
          }`}
        >
          <Check size={10} strokeWidth={2.4} />
        </span>
      {:else if iconMap[item.id]}
        <svelte:component this={iconMap[item.id]} size={13} strokeWidth={2} class="shrink-0 text-[#334155]" />
      {/if}
      <span class={item.type === 'toggle' ? 'font-medium text-[#1f2937]' : 'text-[#111827]'}>{item.label}</span>
      {#if item.description}
        <span
          class="ml-0.5 inline-flex shrink-0 cursor-help items-center text-[#94a3b8] transition-colors hover:text-[#475569]"
          role="img"
          aria-label={item.description}
          title={item.description}
        >
          <Info size={12} strokeWidth={2.1} />
        </span>
      {/if}
    </div>
  </svelte:fragment>
</SearchPanel>
