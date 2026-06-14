<script lang="ts">
  export let className = ''
</script>

<div class={`flex h-full w-full flex-col overflow-hidden rounded-[14px] border border-[var(--border-muted)] bg-white text-[var(--text-primary)] shadow-[0_12px_40px_rgba(15,23,42,0.12)] ${className}`}>
  <slot />
</div>
