<script lang="ts">
  import Item from './Item.svelte'

  export let label = ''
  export let ariaLabel = label
  export let tooltip = label
  export let expanded = true
  export let pressed = false
  export let onClick: () => void = () => {}
</script>

<Item
  {label}
  {ariaLabel}
  {tooltip}
  {expanded}
  showTooltipWhenExpanded
  active={pressed}
  {pressed}
  {onClick}
>
  <slot name="icon" slot="icon" />
</Item>
