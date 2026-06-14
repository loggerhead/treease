<script lang="ts">
  export let className = ''
</script>

<div class={`px-3 py-4 text-[12px] text-[var(--text-muted)] ${className}`}>
  <slot>No results.</slot>
</div>
