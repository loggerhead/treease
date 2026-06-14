<script lang="ts">
  export let heading = ''
  export let className = ''
</script>

<div class={`flex flex-col gap-1 ${className}`}>
  {#if heading}
    <div class="px-3 pb-1 text-[11px] uppercase tracking-[0.2em] text-[var(--text-muted)]">{heading}</div>
  {/if}
  <slot />
</div>
