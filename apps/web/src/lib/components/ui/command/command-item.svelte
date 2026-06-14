<script lang="ts">
  import { createEventDispatcher } from 'svelte'
  export let selected = false
  export let disabled = false
  export let className = ''
  export let ariaLabel: string | undefined = undefined
  export let testId: string | undefined = undefined
  const dispatch = createEventDispatcher()
</script>

<button
  class={`flex w-full items-center gap-2 rounded-[10px] px-3 py-2 text-left text-[14px] ${
    selected ? 'bg-[#eef2f7] text-[var(--text-primary)]' : 'text-[var(--text-primary)]'
  } ${disabled ? 'opacity-50' : 'hover:bg-[#f1f5f9]'} ${className}`}
  aria-label={ariaLabel}
  data-testid={testId}
  disabled={disabled}
  on:click={(event) => dispatch('click', event)}
  on:mouseenter={(event) => dispatch('mouseenter', event)}
>
  <slot />
</button>
