<script lang="ts">
  import { tick } from 'svelte'

  export let content = ''
  export let side: 'top' | 'top-left' | 'right' | 'bottom' | 'left' | 'bottom-left' = 'right'
  export let className = ''
  export let disabled = false

  let trigger: HTMLSpanElement | null = null
  let tooltipContent: HTMLSpanElement | null = null
  let visible = false

  function portal(node: HTMLElement) {
    document.body.appendChild(node)
    return {
      destroy() {
        node.remove()
      }
    }
  }

  function position(): void {
    if (!visible || !trigger || !tooltipContent) return
    const triggerRect = trigger.getBoundingClientRect()
    const gap = 8
    let left = triggerRect.left
    let top = triggerRect.top
    let transform = 'translate(0, 0)'

    if (side === 'right') {
      left = triggerRect.right + gap
      top = triggerRect.top + triggerRect.height / 2
      transform = 'translate(0, -50%)'
    } else if (side === 'left') {
      left = triggerRect.left - gap
      top = triggerRect.top + triggerRect.height / 2
      transform = 'translate(-100%, -50%)'
    } else if (side === 'top') {
      left = triggerRect.left + triggerRect.width / 2
      top = triggerRect.top - gap
      transform = 'translate(-50%, -100%)'
    } else if (side === 'top-left') {
      left = triggerRect.right
      top = triggerRect.top - gap
      transform = 'translate(-100%, -100%)'
    } else if (side === 'bottom-left') {
      left = triggerRect.right - gap
      top = triggerRect.bottom + gap
      transform = 'translate(-100%, 0)'
    } else {
      left = triggerRect.left + triggerRect.width / 2
      top = triggerRect.bottom + gap
      transform = 'translate(-50%, 0)'
    }

    tooltipContent.style.left = `${left}px`
    tooltipContent.style.top = `${top}px`
    tooltipContent.style.transform = transform
  }

  async function show(): Promise<void> {
    if (disabled || !content) return
    visible = true
    await tick()
    position()
  }

  function hide(): void {
    visible = false
  }

  function handleFocusOut(): void {
    requestAnimationFrame(() => {
      if (!trigger?.contains(document.activeElement)) hide()
    })
  }

  $: if (disabled && visible) hide()
</script>

<svelte:window on:resize={position} on:scroll|capture={position} />

<span
  bind:this={trigger}
  class={`ui-tooltip ui-tooltip--${side} ${className}`}
  role="presentation"
  on:mouseenter={show}
  on:mouseleave={hide}
  on:focusin={show}
  on:focusout={handleFocusOut}
>
  <slot />
  {#if content && !disabled}
    <span
      bind:this={tooltipContent}
      use:portal
      class:ui-tooltip__content--visible={visible}
      class="ui-tooltip__content"
      role="tooltip"
      aria-hidden={!visible}
    >{content}</span>
  {/if}
</span>

<style>
  .ui-tooltip { position: relative; display: inline-flex; min-width: 0; }
  .ui-tooltip__content { position: fixed; z-index: 10001; width: max-content; max-width: 280px; padding: 6px 8px; border-radius: 5px; color: white; background: #183b56; box-shadow: 0 6px 16px rgba(24, 59, 86, .16); font-size: var(--font-size-control); line-height: 1.35; opacity: 0; pointer-events: none; transition: opacity 120ms ease, transform 120ms ease; }
  .ui-tooltip__content--visible { opacity: 1; }
</style>
