<script lang="ts">
  import { ArrowRightLeft, FileInput, GitCompareArrows, ImageDown, Search, Share2, ZoomIn, ZoomOut } from 'lucide-svelte'
  import AccountMenu from './AccountMenu.svelte'
  import GraphSearchPanel from './GraphSearchPanel.svelte'
  import Tooltip from './Tooltip.svelte'

  export let viewMode: 'graph' | 'text' = 'graph'
  export let surfaceMode: 'graph' | 'compare' | undefined = undefined
  export let showTools = true
  export let showGlobal = true
  export let documentKey = ''
  export let language = ''
  export let text = ''
  /** Reads the mounted scene directly; this component never keeps its own readiness cache. */
  export let isGraphInteractive: () => boolean = () => false
  export let onSearchSelect: (event: CustomEvent<any>) => void = () => {}
  export let onSearchPreview: (result: any) => void = () => {}
  export let onSearchCancel: () => void = () => {}
  export let onOpenCompareFile: () => void = () => {}
  export let onSwapEditors: () => void = () => {}
  export let onCompare: () => void = () => {}
  export let onZoomIn: () => void = () => {}
  export let onZoomOut: () => void = () => {}
  export let onExportImage: () => void = () => {}
  export let onShare: () => void = () => {}
  export let onLogin: () => void = () => {}
  export let onLogout: () => Promise<void> = async () => {}
  export let onCheckForUpdates: () => Promise<void> = async () => {}
  export let onOpenSettings: () => void = () => {}
  export let controlsDisabled = false

  let graphSearchPanel: GraphSearchPanel | null = null
  let graphSearchOpen = false
  $: activeSurfaceMode = surfaceMode ?? (viewMode === 'graph' ? 'graph' : 'compare')
</script>

<header class:graph-topbar--global-only={!showTools} class:graph-topbar--tools-only={!showGlobal} class="graph-topbar" data-testid="graph-topbar">
  {#if showTools}
    <div class="graph-topbar__tools">
      {#if activeSurfaceMode === 'graph'}
      <div class="graph-topbar__search-wrap">
        <Tooltip content="Search graph" side="bottom" disabled={graphSearchOpen}><button class="graph-topbar__button" aria-label="Search graph" data-testid="graph-search-trigger" disabled={controlsDisabled} on:click={() => {
          if (isGraphInteractive()) void graphSearchPanel?.openPanel()
        }}>
          <Search size={13} />
        </button></Tooltip>
        <GraphSearchPanel
          bind:this={graphSearchPanel}
          {documentKey}
          {language}
          {text}
          panelClass="absolute right-0 top-[calc(100%+8px)] z-40 w-[320px]"
          onOpenChange={(open) => (graphSearchOpen = open)}
          previewResultCallback={onSearchPreview}
          cancelCallback={onSearchCancel}
          on:select={onSearchSelect}
        />
      </div>
      <Tooltip content="Zoom in" side="bottom"><button class="graph-topbar__button" aria-label="Zoom in" data-testid="zoom-in-button" disabled={controlsDisabled} on:click={onZoomIn}>
        <ZoomIn size={13} />
      </button></Tooltip>
      <Tooltip content="Zoom out" side="bottom"><button class="graph-topbar__button" aria-label="Zoom out" data-testid="zoom-out-button" disabled={controlsDisabled} on:click={onZoomOut}>
        <ZoomOut size={13} />
      </button></Tooltip>
      <Tooltip content="Export image" side="bottom-left"><button class="graph-topbar__button" aria-label="Export image" disabled={controlsDisabled} on:click={onExportImage}>
        <ImageDown size={13} />
      </button></Tooltip>
    {:else}
      <Tooltip content="Load compare file" side="bottom"><button class="graph-topbar__button" aria-label="Load compare file" on:click={onOpenCompareFile}>
        <FileInput size={13} />
      </button></Tooltip>
      <Tooltip content="Swap editors" side="bottom"><button class="graph-topbar__button" aria-label="Swap editors" on:click={onSwapEditors}>
        <ArrowRightLeft size={13} />
      </button></Tooltip>
      <Tooltip content="Run comparison" side="bottom-left"><button class="graph-topbar__button" aria-label="Run comparison" on:click={onCompare}>
        <GitCompareArrows size={13} />
      </button></Tooltip>
      {/if}
    </div>
  {/if}
  {#if showGlobal}
  <div class="graph-topbar__global">
    <button class="graph-topbar__share" aria-label="Share" title="Share" on:click={onShare}>
      <Share2 size={13} />
      <span>Share</span>
    </button>
    <div class="graph-topbar__account">
      <AccountMenu variant="editor" {onLogin} {onLogout} {onCheckForUpdates} {onOpenSettings} />
    </div>
  </div>
  {/if}
</header>

<style>
  .graph-topbar {
    position: relative;
    z-index: 20;
    display: flex;
    height: var(--topbar-height);
    min-width: 0;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    padding: 0 var(--space-4);
    border: 0;
    background: transparent;
  }

  .graph-topbar__tools, .graph-topbar__global { display: flex; align-items: center; gap: var(--space-1); }
  .graph-topbar--global-only { justify-content: flex-end; }
  .graph-topbar--tools-only { justify-content: flex-end; }
  .graph-topbar__search-wrap { position: relative; display: flex; align-items: center; }
  .graph-topbar__search-wrap > :global(.relative) { position: static; }
  .graph-topbar__tools { padding: var(--space-1); border: 1px solid var(--border-strong); border-radius: var(--control-radius); background: var(--panel-bg-alt); }
  .graph-topbar__button, .graph-topbar__share { display: inline-flex; height: var(--control-height); align-items: center; justify-content: center; border: 0; border-radius: 4px; color: var(--text-primary); background: transparent; }
  .graph-topbar__button { width: 24px; transition: var(--control-transition); }
  .graph-topbar__share { gap: 4px; padding: 0 7px; font-size: var(--font-size-control); font-weight: 650; letter-spacing: .01em; }
  .graph-topbar__button:hover:not(:disabled) { color: var(--text-primary); background: var(--panel-bg); box-shadow: 0 1px 3px rgb(29 39 53 / 9%); }
  .graph-topbar__button:disabled { cursor: not-allowed; opacity: .42; }
  .graph-topbar__share { transition: var(--control-transition); }
  .graph-topbar__share:hover { color: var(--text-primary); background: var(--panel-bg-alt); }
  .graph-topbar__button:focus-visible, .graph-topbar__share:focus-visible { outline: none; box-shadow: var(--focus-ring); }

  .graph-topbar__account { display: flex; align-items: center; }
  .graph-topbar__account :global([data-slot='button']) { width: var(--control-height); height: var(--control-height); border: 0; border-radius: 4px; }
  .graph-topbar__account :global([data-slot='button']:hover) { background: var(--panel-bg-alt); }
</style>
