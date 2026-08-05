<script lang="ts">
  import { Search } from 'lucide-svelte'
  import { createEventDispatcher } from 'svelte'
  import { CommandInput } from './ui/command'

  export let value = ''
  export let placeholder = 'Search Command'
  export let shortcut = '⌘ K'
  export let className = ''
  export let inputAriaLabel = placeholder
  export let inputTestId = ''
  let inputEl: { focus: () => void } | null = null

  export function focus() {
    inputEl?.focus()
  }

  const dispatch = createEventDispatcher<{
    click: MouseEvent;
    focus: FocusEvent;
    input: { event: Event; value: string };
    keydown: { event: KeyboardEvent; value: string };
  }>()
</script>

<div
  class={`flex min-w-[200px] items-center gap-1 rounded-[7px] bg-[var(--panel-bg)] px-2 py-0 text-[13px] text-[#6b7280] ${className}`}

>
  <Search size={12} strokeWidth={2} class="text-[#64748b]" />
  <CommandInput
    bind:this={inputEl}
    class="min-w-0 flex-1 border-none bg-transparent text-[13px] leading-[1] font-normal tracking-[-0.01em] text-[#111827] outline-none placeholder:text-[#9ca3af]"
    {value}
    {placeholder}
    aria-label={inputAriaLabel}
    data-testid={inputTestId || undefined}
    onclick={(event) => {
      inputEl?.focus()
      dispatch('click', event)
    }}
    onfocus={(event) => dispatch('focus', event)}
    oninput={(event) => dispatch('input', {
      event,
      value: (event.currentTarget as HTMLInputElement).value,
    })}
    onkeydown={(event) => dispatch('keydown', {
      event,
      value: (event.currentTarget as HTMLInputElement).value,
    })}
  />
  <span class="ml-1 rounded-[4px] border-l border-[rgba(15,23,42,0.06)] pl-1.5 pr-[2px] text-[10px] font-medium leading-[16px] tracking-[-0.01em] text-[#94a3b8]">{shortcut}</span>
</div>
