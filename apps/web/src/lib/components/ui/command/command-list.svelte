<script lang="ts">
  export let className = ''
</script>

<div class={`max-h-[280px] overflow-auto p-2 ${className}`}>
  <slot />
</div>
