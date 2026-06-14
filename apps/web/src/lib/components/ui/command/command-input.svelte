<script lang="ts">
  import { createEventDispatcher } from 'svelte'
  export let value = ''
  export let placeholder = ''
  export let className = ''
  const dispatch = createEventDispatcher()
</script>

<div class={`flex items-center gap-2 border-b border-[var(--border-muted)] px-3 py-2 ${className}`}>
  <slot name="icon" />
  <input
    class="h-9 w-full border-none bg-transparent text-[13px] text-[var(--text-primary)] outline-none"
    bind:value
    {placeholder}
    on:input={(event) => dispatch('input', event)}
    on:keydown={(event) => dispatch('keydown', event)}
  />
  <slot name="trailing" />
</div>
