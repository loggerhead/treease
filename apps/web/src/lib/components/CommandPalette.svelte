<script lang="ts">
  import { tick, createEventDispatcher } from 'svelte'
  import SearchPanel from './SearchPanel.svelte'
  import Tooltip from './Tooltip.svelte'
  import { commandItems, type CommandId } from '../command-registry'
  import { Search, Wand2, Shrink, Minimize2, ArrowDownWideNarrow, ListFilter, WrapText, Code, Info } from 'lucide-svelte'
  import { languageId } from '../store/document-session-store'

  export let value = ''
  export let compact = false
  export let compactLabel = ''
  export let onExecute: (id: CommandId) => void | Promise<void> = () => {}

  let open = false
  let query = ''
  let compactTrigger: HTMLButtonElement | null = null
  let compactPanelStyle = ''
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

  function setQuery(next: string) {
    query = next
    dispatch('input', next)
  }

  function closePanel() {
    open = false
  }

  function positionCompactPanel(): void {
    if (!compactTrigger || !compact) return
    const rect = compactTrigger.getBoundingClientRect()
    const panelWidth = 280
    const left = Math.max(8, Math.min(rect.left, window.innerWidth - panelWidth - 8))
    compactPanelStyle = `left: ${left}px; top: ${rect.bottom + 8}px;`
  }

  export async function openPanel() {
    open = true
    positionCompactPanel()
    await tick()
    positionCompactPanel()
    searchPanel?.focusInput?.()
  }

  function handleDocumentPointerDown(event: PointerEvent) {
    const target = event.target as Node
    if (!panelRef || panelRef.contains(target)) return
    closePanel()
  }

  async function executeCommand(id: CommandId) {
    closePanel()
    await onExecute(id)
  }

  $: if (value !== query) query = value
  // Filtering is owned by the shadcn command primitive. Keeping the full
  // source here is important: the primitive can then show its empty state
  // and handle keyboard navigation consistently.
  $: results = commandSource

</script>

<svelte:window
  on:pointerdown={handleDocumentPointerDown}
  on:resize={positionCompactPanel}
  on:scroll={positionCompactPanel}
/>

{#if compact}
  <button
    bind:this={compactTrigger}
    type="button"
    class:command-palette__trigger--labeled={Boolean(compactLabel)}
    class="command-palette__trigger"
    aria-label="Search commands"
    title="Search commands (⌘K)"
    data-testid="command-search-button"
    on:click={() => void openPanel()}
  >
    <Search size={14} />
    {#if compactLabel}<span>{compactLabel}</span>{/if}
  </button>
{/if}

<SearchPanel
  bind:this={searchPanel}
  bind:containerRef={panelRef}
  {open}
  {query}
  placeholder="Search Command"
  inputAriaLabel="Search command"
  inputTestId="command-search-input"
  panelClass={compact
    ? 'fixed z-[10000] max-h-[calc(100dvh-40px)] w-[280px]'
    : 'absolute left-0 bottom-[calc(100%+8px)] z-40 max-h-[calc(100dvh-42px-8px)] w-[280px]'}
  panelStyle={compact ? compactPanelStyle : ''}
  portalPanel={compact}
  listClassName="command-search-list"
  emptyText="No results."
  showWhenClosed={!compact}
  inputInline={compact}
  {results}
  useVirtualList={false}
  shouldFilter={true}
  itemValue={(item) => item.label}
  itemKeywords={(item) => item.keywords}
  onFocus={() => openPanel()}
  onClick={() => openPanel()}
  onInput={(event: any) => {
    const detail = event.detail as InputEvent
    setQuery((detail.target as HTMLInputElement).value)
  }}
  onEscape={closePanel}
  onItemSelect={(_index, item) => {
    if (item) void executeCommand(item.id)
  }}
  itemKey={(item) => item.id}
>
  <svelte:fragment slot="item" let:item let:index>
    <div class="flex items-center gap-2">
      {#if iconMap[item.id]}
        <svelte:component this={iconMap[item.id]} size={13} strokeWidth={2} class="shrink-0 text-[#334155]" />
      {/if}
      <span class="text-[#111827]">{item.label}</span>
      {#if item.description}
        <Tooltip content={item.description} side="right" className="ml-0.5 shrink-0 cursor-help text-[#94a3b8] hover:text-[#475569]">
          <span aria-hidden="true"><Info size={12} strokeWidth={2.1} /></span>
        </Tooltip>
      {/if}
    </div>
  </svelte:fragment>
</SearchPanel>

<style>
  .command-palette__trigger {
    display: inline-flex;
    width: var(--control-height);
    height: var(--control-height);
    align-items: center;
    justify-content: center;
    gap: var(--space-1);
    border: 1px solid var(--border-muted);
    border-radius: var(--control-radius);
    padding: 0;
    color: var(--text-primary);
    background: var(--panel-bg);
    font-size: var(--font-size-control);
    line-height: 1;
    transition: var(--control-transition);
  }

  .command-palette__trigger--labeled { width: auto; padding: 0 var(--space-2) 0 var(--space-3); }
  .command-palette__trigger:hover { border-color: var(--border-strong); background: var(--panel-bg-alt); }
  .command-palette__trigger:focus-visible { outline: none; box-shadow: var(--focus-ring); }
</style>
