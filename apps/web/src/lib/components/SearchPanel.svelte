<script lang="ts">
  import { tick } from 'svelte'
  import { createVirtualizer } from '@tanstack/svelte-virtual'
  import SearchInput from './SearchInput.svelte'
  import SearchResultsList from './SearchResultsList.svelte'

  export let open = false
  export let query = ''
  export let placeholder = 'Search'
  export let shortcut = ''
  export let panelClass = 'absolute left-0 top-[calc(100%+8px)] z-40 w-[320px]'
  export let panelFrameClass = ''
  export let commandClassName = ''
  export let listClassName = 'graph-search-list'
  export let emptyText = 'No results.'
  export let showWhenClosed = true
  export let inputInline = false
  export let inputAriaLabel = placeholder
  export let inputTestId = ''
  export let inputClassName = ''
  export let activeIndex = 0
  export let results: any[] = []
  export let useVirtualList = true
  export let estimateSize = 36
  export let overscan = 6
  export let onInput: (event: any) => void = () => {}
  export let onKeydown: (event: any) => void = () => {}
  export let onFocus: (event: any) => void = () => {}
  export let onClick: (event: any) => void = () => {}
  export let onItemHover: (index: number) => void = () => {}
  export let onItemSelect: (index: number) => void = () => {}
  export let itemKey: (item: any, index: number) => string | number = (_item, index) => index
  export let itemAriaLabel: (item: any, index: number) => string | undefined = () => undefined
  export let itemTestId: (item: any, index: number) => string | undefined = () => undefined
  export let containerRef: HTMLDivElement | null = null

  let searchRef: SearchInput | null = null
  let listRef: HTMLDivElement | null = null

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
      count: results.length,
      getScrollElement: () => listRef,
      estimateSize: () => estimateSize,
      overscan
    })
  }
  $: if (open && results.length > 0) $virtualizer.scrollToIndex(activeIndex)
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
      on:keydown={onKeydown}
    />
  {/if}
  {#if open}
    <div class={panelClass}>
      {#if panelFrameClass}
        <div class={panelFrameClass}>
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
              on:keydown={onKeydown}
            />
          {/if}
          <div class={inputInline ? 'mt-2' : ''}>
            <SearchResultsList
              {results}
              {activeIndex}
              {useVirtualList}
              virtualizer={virtualizer}
              {listClassName}
              {emptyText}
              {itemKey}
              {itemAriaLabel}
              {itemTestId}
              {onItemHover}
              {onItemSelect}
              {commandClassName}
            >
              <svelte:fragment slot="item" let:item let:index>
                <slot name="item" {item} {index} />
              </svelte:fragment>
            </SearchResultsList>
          </div>
        </div>
      {:else}
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
            on:keydown={onKeydown}
          />
        {/if}
        <div class={inputInline ? 'mt-2' : ''}>
          <SearchResultsList
            {results}
            {activeIndex}
            {useVirtualList}
            virtualizer={virtualizer}
            {listClassName}
            {emptyText}
            {itemKey}
            {itemAriaLabel}
            {itemTestId}
            {onItemHover}
            {onItemSelect}
            {commandClassName}
          >
            <svelte:fragment slot="item" let:item let:index>
              <slot name="item" {item} {index} />
            </svelte:fragment>
          </SearchResultsList>
        </div>
      {/if}
    </div>
  {/if}
</div>
