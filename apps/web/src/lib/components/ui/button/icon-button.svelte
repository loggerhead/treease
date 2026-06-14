<script lang="ts">
  import { createEventDispatcher } from 'svelte'
  import Button from './button.svelte'
  import type { ComponentProps } from 'svelte'

  const dispatch = createEventDispatcher<{ click: MouseEvent }>()

  let {
    class: className,
    children,
    ...restProps
  }: Omit<ComponentProps<typeof Button>, 'variant' | 'size' | 'iconOnly'> = $props()

  function handleClick(event: CustomEvent<MouseEvent>) {
    dispatch('click', event.detail)
  }
</script>

<Button variant="ghost" size="xs" iconOnly={true} class={className} on:click={handleClick} {...restProps}>
  {@render children?.()}
</Button>
