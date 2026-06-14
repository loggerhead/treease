<script lang="ts">
  import { Command, CommandList, CommandItem, CommandEmpty, CommandGroup } from './ui/command';

  export let results: any[] = [];
  export let activeIndex = 0;
  export let useVirtualList = true;
  export let virtualizer: any;
  export let listClassName = '';
  export let emptyText = 'No results.';
  export let itemKey: (item: any, index: number) => string | number = (_item, index) => index;
  export let itemAriaLabel: (item: any, index: number) => string | undefined = () => undefined;
  export let itemTestId: (item: any, index: number) => string | undefined = () => undefined;
  export let onItemHover: (index: number) => void = () => {};
  export let onItemSelect: (index: number) => void = () => {};
  export let commandClassName = '';
</script>

<Command className={commandClassName}>
  <CommandList className={listClassName}>
    {#if results.length === 0}
      <CommandEmpty>{emptyText}</CommandEmpty>
    {:else}
      <CommandGroup>
        {#if useVirtualList}
          <div style={`height: ${$virtualizer.getTotalSize()}px; position: relative;`}>
            {#each $virtualizer.getVirtualItems() as row (row.key)}
              <div
                style={`position: absolute; top: 0; left: 0; width: 100%; transform: translateY(${row.start}px);`}
              >
                <CommandItem
                  ariaLabel={itemAriaLabel(results[row.index], row.index)}
                  selected={row.index === activeIndex}
                  testId={itemTestId(results[row.index], row.index)}
                  on:mouseenter={() => onItemHover(row.index)}
                  on:click={() => onItemSelect(row.index)}
                >
                  <slot name="item" item={results[row.index]} index={row.index} />
                </CommandItem>
              </div>
            {/each}
          </div>
        {:else}
          {#each results as item, index (itemKey(item, index))}
            <CommandItem
              ariaLabel={itemAriaLabel(item, index)}
              selected={index === activeIndex}
              testId={itemTestId(item, index)}
              on:mouseenter={() => onItemHover(index)}
              on:click={() => onItemSelect(index)}
            >
              <slot name="item" {item} {index} />
            </CommandItem>
          {/each}
        {/if}
      </CommandGroup>
    {/if}
  </CommandList>
</Command>
