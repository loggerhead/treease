<script lang="ts">
  import { createEventDispatcher } from 'svelte'
  import { cn, type WithElementRef } from '$lib/utils'
  import type { HTMLButtonAttributes } from 'svelte/elements'

  const dispatch = createEventDispatcher<{ click: MouseEvent }>()

  let {
    ref = $bindable(null),
    class: className,
    children,
    variant = 'outline',
    size = 'default',
    iconOnly = false,
    type = 'button',
    onclick,
    ...restProps
  }: WithElementRef<HTMLButtonAttributes> & {
    variant?: 'outline' | 'ghost'
    size?: 'xs' | 'sm' | 'default'
    iconOnly?: boolean
  } = $props()

  function handleClick(event: MouseEvent & { currentTarget: EventTarget & HTMLButtonElement }) {
    onclick?.(event)
    dispatch('click', event)
  }

  const baseClass =
    'inline-flex shrink-0 items-center justify-center whitespace-nowrap rounded-[8px] text-[var(--text-primary)] outline-none transition-[color,background-color,border-color,box-shadow] disabled:pointer-events-none disabled:opacity-50 focus-visible:ring-2 focus-visible:ring-[var(--accent)]/25'

  const variantClass = {
    outline: 'border border-[var(--border-muted)] bg-[var(--panel-bg)] hover:border-[var(--accent)]',
    ghost: 'border border-transparent bg-transparent hover:bg-[var(--panel-bg-alt)] hover:text-[var(--accent)]'
  } as const

  function getSizeClass(currentSize: 'xs' | 'sm' | 'default', currentIconOnly: boolean) {
    if (currentSize === 'xs') return currentIconOnly ? 'h-6 w-6' : 'h-6 px-2.5 text-[12px] font-medium'
    if (currentSize === 'sm') return currentIconOnly ? 'h-7 w-7' : 'h-7 px-3 text-[13px] font-medium'
    return currentIconOnly ? 'h-9 w-9' : 'h-9 px-4 text-[14px] font-medium'
  }
</script>

<button
  bind:this={ref}
  data-slot="button"
  data-variant={variant}
  data-size={size}
  data-icon-only={iconOnly ? 'true' : 'false'}
  {type}
  class={cn(baseClass, variantClass[variant], getSizeClass(size, iconOnly), className)}
  onclick={handleClick}
  {...restProps}
>
  {@render children?.()}
</button>
