<script lang="ts">
  import { PanelLeftClose, PanelRightClose } from 'lucide-svelte';
  import type { SplitLayoutCollapseSide } from './split-layout-controller';

  export let side: SplitLayoutCollapseSide;
</script>

<div class={`app-split-collapse-hint app-split-collapse-hint--${side}`} aria-hidden="true">
  {#if side === 'left'}
    <PanelLeftClose size={16} />
  {:else}
    <PanelRightClose size={16} />
  {/if}
</div>
