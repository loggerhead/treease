<script lang="ts">
  import { CommandList, CommandItem, CommandEmpty, CommandGroup } from './ui/command';

  export let results: any[] = [];
  export let activeIndex = -1;
  export let useVirtualList = true;
  export let virtualizer: any;
  export let listClassName = '';
  export let emptyText = 'No results.';
  export let itemKey: (item: any, index: number) => string | number = (_item, index) => index;
  export let itemValue: (item: any, index: number) => string = (item, index) => String(item?.id ?? item?.label ?? index);
  export let itemKeywords: (item: any, index: number) => string[] = () => [];
  export let itemAriaLabel: (item: any, index: number) => string | undefined = () => undefined;
  export let itemTestId: (item: any, index: number) => string | undefined = () => undefined;
  export let onItemSelect: (index: number, item: any) => void = () => {};
</script>

<CommandList class={listClassName}>
  <CommandEmpty>{emptyText}</CommandEmpty>
  <CommandGroup>
    {#if useVirtualList}
      <div style={`height: ${$virtualizer.getTotalSize()}px; position: relative;`}>
        {#each $virtualizer.getVirtualItems() as row (row.key)}
          <div
            style={`position: absolute; top: 0; left: 0; width: 100%; transform: translateY(${row.start}px);`}
          >
            <CommandItem
              value={itemValue(results[row.index], row.index)}
              keywords={itemKeywords(results[row.index], row.index)}
              aria-label={itemAriaLabel(results[row.index], row.index)}
              data-testid={itemTestId(results[row.index], row.index)}
              data-search-index={row.index}
              data-search-active={activeIndex === row.index ? 'true' : 'false'}
              aria-selected={activeIndex === row.index}
              onSelect={() => onItemSelect(row.index, results[row.index])}
            >
              <slot name="item" item={results[row.index]} index={row.index} />
            </CommandItem>
          </div>
        {/each}
      </div>
    {:else}
      {#each results as item, index (itemKey(item, index))}
        <CommandItem
          value={itemValue(item, index)}
          keywords={itemKeywords(item, index)}
          aria-label={itemAriaLabel(item, index)}
          data-testid={itemTestId(item, index)}
          data-search-index={index}
          data-search-active={activeIndex === index ? 'true' : 'false'}
          aria-selected={activeIndex === index}
          onSelect={() => onItemSelect(index, item)}
        >
          <slot name="item" {item} {index} />
        </CommandItem>
      {/each}
    {/if}
  </CommandGroup>
</CommandList>

<style>
  :global([data-search-active='true']) {
    background-color: #eef2f7 !important;
    color: var(--text-primary) !important;
  }

  :global([data-search-active='false']) {
    background-color: transparent !important;
  }
</style>
