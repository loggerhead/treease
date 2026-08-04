<script lang="ts">
  import { onMount } from 'svelte'
  import Item from './Item.svelte'

  export let label = ''
  export let ariaLabel = label
  export let tooltip = label
  export let expanded = true
  export let open = false
  export let testId: string | undefined = undefined
  export let panelClass = ''
  export let placement: 'right-start' | 'right-end' = 'right-start'
  export let customTrigger = false
  export let triggerClass = ''

  let anchor: HTMLDivElement | null = null

  onMount(() => {
    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target as HTMLElement | null
      if (target?.closest('[data-slot="select-content"]')) return
      if (open && anchor && target && !anchor.contains(target)) open = false
    }

    document.addEventListener('pointerdown', handlePointerDown)
    return () => document.removeEventListener('pointerdown', handlePointerDown)
  })

  export function openPanel(): void {
    open = true
  }

  export function closePanel(): void {
    open = false
  }

  function togglePanel(): void {
    open = !open
  }
</script>

<div class="sidebar__item-wrap" bind:this={anchor}>
  {#if customTrigger}
    <button class={`sidebar__context-trigger ${triggerClass}`} type="button" aria-label={ariaLabel} data-testid={testId} on:click={togglePanel}>
      <slot name="trigger" />
    </button>
  {:else}
    <Item
      {label}
      {ariaLabel}
      {tooltip}
      {expanded}
      active={open}
      testId={testId}
      onClick={togglePanel}
    >
      <slot name="icon" slot="icon" />
    </Item>
  {/if}
  {#if open}
    <div class:sidebar__popover--right-end={placement === 'right-end'} class={`sidebar__popover ${panelClass}`}>
      <slot name="panel" />
    </div>
  {/if}
</div>

<style>
  .sidebar__item-wrap {
    position: relative;
    width: 100%;
  }

  :global(.sidebar__context-trigger) {
    display: flex;
    width: 32px;
    height: 32px;
    align-items: center;
    border: 0;
    border-radius: 6px;
    padding: 0;
    background: transparent;
    cursor: pointer;
  }

  :global(.sidebar__context-trigger:hover),
  :global(.sidebar__context-trigger[data-state='open']) {
    background: var(--panel-bg-alt);
  }

  :global(.sidebar__popover) {
    position: absolute;
    top: 0;
    left: 100%;
    z-index: 60;
    width: 300px;
    padding: var(--space-5);
    border: 1px solid var(--border-muted);
    border-radius: 0 var(--surface-radius) var(--surface-radius) var(--surface-radius);
    background: var(--panel-bg);
    box-shadow: 0 12px 28px rgb(24 59 86 / 12%);
  }

  :global(.sidebar__popover--right-end) {
    top: auto;
    bottom: 0;
  }
</style>
