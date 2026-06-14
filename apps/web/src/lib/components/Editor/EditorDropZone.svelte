<script lang="ts">
  import EditorRuntimeLoading from './EditorRuntimeLoading.svelte';

  export let onDrop: (event: DragEvent) => void = () => {};
  export let onDragOver: (event: DragEvent) => void = () => {};
  export let loading = false;
  export let loadingPhase = 'Loading editor runtime...';

  let container: HTMLDivElement;

  export function getContainer(): HTMLDivElement {
    return container;
  }
</script>

<div
  class="relative flex h-full w-full flex-col"
  data-testid="source-editor-region"
  role="region"
  aria-label="Source editor"
  on:drop={onDrop}
  on:dragover={onDragOver}
>
  <div bind:this={container} class="w-full flex-1 overflow-hidden"></div>
  {#if loading}
    <div class="pointer-events-none absolute inset-0 z-1">
      <EditorRuntimeLoading phase={loadingPhase} />
    </div>
  {/if}
</div>
