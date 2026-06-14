<script lang="ts">
  import { Search } from 'lucide-svelte'
  import { createEventDispatcher } from 'svelte'

  export let value = ''
  export let placeholder = 'Search Command'
  export let shortcut = '⌘ K'
  export let className = ''
  export let inputAriaLabel = placeholder
  export let inputTestId = ''
  let inputEl: HTMLInputElement | null = null

  export function focus() {
    inputEl?.focus()
  }

  const dispatch = createEventDispatcher()
</script>

<div
  class={`flex min-w-[200px] items-center gap-2 rounded-[10px] bg-[var(--panel-bg)] px-2 py-1 text-[12px] text-[var(--text-muted)] ${className}`}
  role="button"
  tabindex="0"
  on:click={(event) => {
    inputEl?.focus()
    dispatch('click', event)
  }}
  on:keydown={(event) => {
    if (event.target !== event.currentTarget) return
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault()
      inputEl?.focus()
      dispatch('click', event)
    }
  }}
>
  <Search size={14} />
  <input
    bind:this={inputEl}
    class="min-w-0 flex-1 border-none bg-transparent text-[12px] text-[var(--text-primary)] outline-none"
    bind:value
    {placeholder}
    aria-label={inputAriaLabel}
    data-testid={inputTestId || undefined}
    on:focus={(event) => dispatch('focus', event)}
    on:input={(event) => dispatch('input', event)}
    on:keydown|capture={(event) => {
      if (event.key === 'Escape') {
        event.preventDefault()
        event.stopPropagation()
      }
      dispatch('keydown', event)
    }}
  />
  <span class="rounded-[6px] px-[6px] py-[1px] text-[10px] text-[var(--text-muted)]">{shortcut}</span>
</div>
