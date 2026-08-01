<script lang="ts">
  import EditorRuntimeLoading from './EditorRuntimeLoading.svelte';

  export let onDrop: (event: DragEvent) => void = () => {};
  export let onDragOver: (event: DragEvent) => void = () => {};
  export let onPointerDownCapture: (event: PointerEvent) => void = () => {};
  export let loading = false;
  export let loadingPhase = 'Loading editor runtime...';
  export let error = false;
  export let onRetry: () => void = () => {};

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
  on:pointerdown|capture={onPointerDownCapture}
  on:drop={onDrop}
  on:dragover={onDragOver}
>
  <div bind:this={container} class="w-full flex-1 overflow-hidden"></div>
  {#if loading}
    <div class="pointer-events-none absolute inset-0 z-1">
      <EditorRuntimeLoading phase={loadingPhase} />
    </div>
  {:else if error}
    <div
      class="absolute inset-0 z-1 flex items-center justify-center bg-[var(--panel-bg)]/95 p-6"
      data-testid="editor-runtime-error"
      role="alert"
    >
      <div class="max-w-sm text-center">
        <p class="text-sm font-semibold text-[var(--text-primary)]">{loadingPhase}</p>
        <p class="mt-2 text-xs text-[var(--text-muted)]">The editor runtime failed, but the rest of the workspace is still available.</p>
        <button
          type="button"
          class="mt-4 rounded border border-[var(--border-muted)] px-3 py-1.5 text-xs font-medium text-[var(--text-primary)]"
          on:click={onRetry}
        >
          Retry editor
        </button>
      </div>
    </div>
  {/if}
</div>
