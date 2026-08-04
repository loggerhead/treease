<script lang="ts">
  import Tooltip from './Tooltip.svelte'

  export let label = ''
  export let ariaLabel = label
  export let tooltip = label
  export let expanded = true
  export let showTooltipWhenExpanded = false
  export let active = false
  export let pressed: boolean | undefined = undefined
  export let testId: string | undefined = undefined
  export let onClick: () => void = () => {}
  export let className = ''
</script>

<Tooltip content={tooltip} side="right" disabled={expanded && !showTooltipWhenExpanded}>
  <button
    class={`sidebar__item ${className}`}
    class:sidebar__item--expanded={expanded}
    class:sidebar__item--active={active}
    class:sidebar__item--pressed={pressed}
    type="button"
    aria-label={ariaLabel}
    aria-pressed={pressed}
    data-testid={testId}
    on:click={onClick}
  >
    <slot name="icon" />
    <slot />
    <span class="sidebar__item-label">{label}</span>
  </button>
</Tooltip>

<style>
  :global(.sidebar__item) {
    display: flex;
    width: 36px;
    height: 36px;
    box-sizing: border-box;
    flex: 0 0 36px;
    align-items: center;
    gap: 9px;
    overflow: hidden;
    border: 0;
    border-radius: 6px;
    padding: 0 10px;
    color: var(--text-muted);
    background: transparent;
    text-align: left;
    white-space: nowrap;
    cursor: pointer;
    transition: color 120ms ease, background-color 120ms ease;
  }

  :global(.sidebar__item.sidebar__item--expanded) {
    width: 160px;
    flex-basis: 160px;
    padding-inline: 4px;
  }

  :global(.sidebar__item:hover),
  :global(.sidebar__item--active) {
    color: var(--text-primary);
    background: var(--panel-bg-alt);
  }

  :global(.sidebar__item--active) {
    color: var(--accent);
    box-shadow: inset 2px 0 0 var(--accent);
  }

  :global(.sidebar__item--pressed) {
    color: var(--accent);
    background: var(--accent-soft);
  }

  :global(.sidebar__item svg) {
    display: block;
    width: 16px;
    height: 16px;
    flex: 0 0 16px;
  }

  :global(.sidebar__item-label) {
    overflow: hidden;
    opacity: 0;
    font-size: 11px;
    text-overflow: ellipsis;
    transition: opacity 120ms ease;
  }

  :global(.sidebar__item--expanded .sidebar__item-label) {
    opacity: 1;
  }

  @media (max-width: 760px) {
    :global(.sidebar__item.sidebar__item--expanded) {
      width: 134px;
      flex-basis: 134px;
    }
  }
</style>
