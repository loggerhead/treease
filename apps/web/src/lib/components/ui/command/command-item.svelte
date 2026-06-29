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
  class={`flex w-full items-center gap-2 rounded-[8px] px-2.5 py-1.5 text-left text-[13px] ${
    selected ? 'bg-[#eef2f7] text-[var(--text-primary)]' : 'text-[var(--text-primary)]'
  } ${disabled ? 'opacity-50' : 'hover:bg-[#f8fafc]'} ${className}`}
  aria-label={ariaLabel}
  data-testid={testId}
  disabled={disabled}
  on:click={(event) => dispatch('click', event)}
  on:mouseenter={(event) => dispatch('mouseenter', event)}
>
  <slot />
</button>
