<script lang="ts">
  export let className = ''
</script>

<div class={`max-h-[280px] overflow-auto p-1.5 ${className}`}>
  <slot />
</div>
