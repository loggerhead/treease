<script lang="ts">
  import { tick } from 'svelte'
  import { cubicOut } from 'svelte/easing'
  import { fly } from 'svelte/transition'
  import { createVirtualizer } from '@tanstack/svelte-virtual'
  import { Command } from './ui/command'
  import SearchInput from './SearchInput.svelte'
  import SearchResultsList from './SearchResultsList.svelte'

  export let open = false
  export let query = ''
  export let placeholder = 'Search'
  export let shortcut = ''
  export let panelClass = 'absolute left-0 top-[calc(100%+8px)] z-40 w-[320px]'
  export let panelStyle = 'transform-origin: top left;'
  export let portalPanel = false
  export let commandClassName = 'rounded-[10px] border border-[rgba(15,23,42,0.10)] bg-[var(--panel-bg)] shadow-[0_12px_28px_rgba(15,23,42,0.10)]'
  export let shouldFilter = false
  export let loop = false
  export let listClassName = 'graph-search-list'
  export let emptyText = 'No results.'
  export let showWhenClosed = true
  export let inputInline = false
  export let inputAriaLabel = placeholder
  export let inputTestId = ''
  export let inputClassName = 'h-[28px] rounded-none border-0 border-b border-[rgba(15,23,42,0.10)] bg-transparent px-2 text-[#6b7280] transition-colors focus-within:border-b-[rgba(15,23,42,0.16)]'
  export let results: any[] = []
  export let activeIndex = -1
  export let useVirtualList = true
  export let estimateSize = 36
  export let overscan = 6
  export let onInput: (event: any) => void = () => {}
  export let onFocus: (event: any) => void = () => {}
  export let onClick: (event: any) => void = () => {}
  export let onItemSelect: (index: number, item: any) => void = () => {}
  export let onActiveIndexChange: (index: number) => void = () => {}
  export let onEscape: () => void = () => {}
  export let itemKey: (item: any, index: number) => string | number = (_item, index) => index
  export let itemValue: (item: any, index: number) => string = (item, index) => String(item?.id ?? item?.label ?? index)
  export let itemKeywords: (item: any, index: number) => string[] = () => []
  export let itemAriaLabel: (item: any, index: number) => string | undefined = () => undefined
  export let itemTestId: (item: any, index: number) => string | undefined = () => undefined
  export let containerRef: HTMLDivElement | null = null
  // Consumers with domain-specific navigation can own result markup while
  // sharing the panel, input, animation, and portal lifecycle.
  export let customResults = false

  let searchRef: SearchInput | null = null
  let listRef: HTMLDivElement | null = null
  let previousQuery = query

  $: visibleResults = shouldFilter && query.trim()
    ? results.filter((item, index) => {
        const haystack = `${itemValue(item, index)} ${itemKeywords(item, index).join(' ')}`.toLowerCase()
        return haystack.includes(query.trim().toLowerCase())
      })
    : results

  $: if (query !== previousQuery) {
    previousQuery = query
    activeIndex = visibleResults.length ? 0 : -1
    onActiveIndexChange(activeIndex)
  }

  $: if (!visibleResults.length) {
    activeIndex = -1
  } else if (activeIndex < 0 || activeIndex >= visibleResults.length) {
    activeIndex = 0
    onActiveIndexChange(activeIndex)
  }

  export function focusInput() {
    searchRef?.focus()
  }

  const virtualizer = createVirtualizer({
    count: 0,
    getScrollElement: () => listRef,
    estimateSize: () => estimateSize,
    overscan
  })

  $: if (open) {
    listRef = containerRef?.querySelector(`.${listClassName}`) as HTMLDivElement | null
  }
  $: if (open) {
    tick().then(() => {
      listRef = containerRef?.querySelector(`.${listClassName}`) as HTMLDivElement | null
    })
  }
  $: if ($virtualizer) {
    $virtualizer.setOptions({
      ...$virtualizer.options,
      count: visibleResults.length,
      getScrollElement: () => listRef,
      estimateSize: () => estimateSize,
      overscan
    })
  }
  function portal(node: HTMLElement, enabled: boolean) {
    if (enabled) document.body.appendChild(node)
    return {
      update(nextEnabled: boolean) {
        if (nextEnabled && node.parentNode !== document.body) document.body.appendChild(node)
      },
      destroy() {
        if (node.parentNode === document.body) node.remove()
      }
    }
  }

  function scrollActiveResultIntoView(index: number): void {
    void tick().then(() => {
      listRef?.querySelector<HTMLElement>(`[data-search-index="${index}"]`)
        ?.scrollIntoView({ block: 'nearest' })
    })
  }

  function moveActiveIndex(offset: number): void {
    if (!visibleResults.length) return
    activeIndex = (activeIndex + offset + visibleResults.length) % visibleResults.length
    onActiveIndexChange(activeIndex)
    scrollActiveResultIntoView(activeIndex)
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault()
      event.stopPropagation()
      moveActiveIndex(event.key === 'ArrowDown' ? 1 : -1)
      return
    }
    if (event.key === 'Enter') {
      const item = visibleResults[activeIndex]
      if (item) {
        event.preventDefault()
        event.stopPropagation()
        onItemSelect(activeIndex, item)
      }
      return
    }
    if (event.key === 'Escape') {
      event.preventDefault()
      event.stopPropagation()
      onEscape()
      return
    }
  }

  function handleInputKeydown(event: CustomEvent): void {
    handleKeydown(event.detail as KeyboardEvent)
  }
</script>

<div class="relative" bind:this={containerRef}>
  {#if !inputInline && (showWhenClosed || open)}
    <SearchInput
      bind:this={searchRef}
      value={query}
      {placeholder}
      {shortcut}
      {inputAriaLabel}
      {inputTestId}
      className={inputClassName}
      on:focus={onFocus}
      on:click={onClick}
      on:input={onInput}
      on:keydown={handleInputKeydown}
    />
  {/if}
  {#if open}
    <div
      use:portal={portalPanel}
      role="presentation"
      on:pointerdown|stopPropagation
      class={panelClass}
      style={panelStyle}
      transition:fly={{ y: 6, duration: 150, opacity: 0.08, easing: cubicOut }}
    >
      <Command
        class={commandClassName}
        {shouldFilter}
        {loop}
      >
        {#if inputInline}
        <SearchInput
            bind:this={searchRef}
            value={query}
            {placeholder}
            {shortcut}
            {inputAriaLabel}
            {inputTestId}
            className={inputClassName}
            on:focus={onFocus}
            on:click={onClick}
            on:input={onInput}
            on:keydown={handleInputKeydown}
          />
        {/if}
        <div class={inputInline ? 'mt-2' : ''}>
          {#if customResults}
            <slot name="results" />
          {:else}
            <SearchResultsList
              results={visibleResults}
              {activeIndex}
              {useVirtualList}
              virtualizer={virtualizer}
              {listClassName}
              {emptyText}
              {itemKey}
              {itemValue}
              {itemKeywords}
              {itemAriaLabel}
              {itemTestId}
              {onItemSelect}
            >
              <svelte:fragment slot="item" let:item let:index>
                <slot name="item" {item} {index} />
              </svelte:fragment>
            </SearchResultsList>
          {/if}
        </div>
      </Command>
    </div>
  {/if}
</div>
