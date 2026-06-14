<script lang="ts">
  import RuntimeLoadingBar from '../runtime-loading/RuntimeLoadingBar.svelte';

  export let phase = 'Loading editor runtime...';

  const gutterRows = Array.from({ length: 11 }, (_, index) => index + 1);
  const lineWidths = [72, 44, 58, 82, 36, 64, 52, 76, 48, 68, 40];
</script>

<div class="editor-runtime-loading" role="status" aria-live="polite" aria-label="Editor loading status">
  <div class="editor-runtime-loading__topbar">
    <span class="editor-runtime-loading__dot"></span>
    <span class="editor-runtime-loading__phase">{phase}</span>
  </div>

  <div class="editor-runtime-loading__body" aria-hidden="true">
    <div class="editor-runtime-loading__gutter">
      {#each gutterRows as row}
        <span>{row}</span>
      {/each}
    </div>
    <div class="editor-runtime-loading__lines">
      {#each lineWidths as width}
        <RuntimeLoadingBar width={`${width}%`} />
      {/each}
    </div>
  </div>

  <p class="editor-runtime-loading__hint">Preparing syntax highlighting, diagnostics, and the sample document.</p>
</div>

<style>
  .editor-runtime-loading {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background:
      linear-gradient(90deg, rgba(37, 99, 235, 0.04) 1px, transparent 1px),
      linear-gradient(180deg, rgba(37, 99, 235, 0.035) 1px, transparent 1px),
      var(--panel-bg);
    background-size: 36px 36px;
    color: var(--text-primary);
  }

  .editor-runtime-loading__topbar {
    display: flex;
    min-height: 32px;
    align-items: center;
    gap: 8px;
    border-bottom: 1px solid var(--border-muted);
    background: rgba(248, 250, 252, 0.9);
    padding: 0 12px;
  }

  .editor-runtime-loading__dot {
    width: 8px;
    height: 8px;
    border-radius: 999px;
    background: var(--accent);
    box-shadow: 0 0 0 5px rgba(37, 99, 235, 0.12);
  }

  .editor-runtime-loading__phase {
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 600;
  }

  .editor-runtime-loading__body {
    display: grid;
    min-height: 0;
    flex: 1;
    grid-template-columns: 48px minmax(0, 1fr);
  }

  .editor-runtime-loading__gutter {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 9px;
    border-right: 1px solid var(--border-muted);
    background: rgba(248, 250, 252, 0.72);
    color: #94a3b8;
    font-family: "SFMono-Regular", Consolas, "Liberation Mono", monospace;
    font-size: 11px;
    padding: 16px 10px 0 0;
  }

  .editor-runtime-loading__lines {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 17px 18px;
  }

  .editor-runtime-loading__hint {
    margin: 0;
    border-top: 1px solid var(--border-muted);
    background: rgba(248, 250, 252, 0.86);
    color: var(--text-muted);
    font-size: 12px;
    padding: 9px 12px;
  }

</style>
