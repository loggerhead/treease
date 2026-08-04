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

  let graphSearchPanel: GraphSearchPanel | null = null
  let graphSearchOpen = false
  $: activeSurfaceMode = surfaceMode ?? (viewMode === 'graph' ? 'graph' : 'compare')
</script>

<header class:graph-topbar--global-only={!showTools} class:graph-topbar--tools-only={!showGlobal} class="graph-topbar" data-testid="graph-topbar">
  {#if showTools}
    <div class="graph-topbar__tools">
      {#if activeSurfaceMode === 'graph'}
      <div class="graph-topbar__search-wrap">
        <Tooltip content="Search graph" side="bottom" disabled={graphSearchOpen}><button class="graph-topbar__button" aria-label="Search graph" data-testid="graph-search-trigger" on:click={() => {
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
      <Tooltip content="Zoom in" side="bottom"><button class="graph-topbar__button" aria-label="Zoom in" data-testid="zoom-in-button" on:click={onZoomIn}>
        <ZoomIn size={13} />
      </button></Tooltip>
      <Tooltip content="Zoom out" side="bottom"><button class="graph-topbar__button" aria-label="Zoom out" data-testid="zoom-out-button" on:click={onZoomOut}>
        <ZoomOut size={13} />
      </button></Tooltip>
      <Tooltip content="Export image" side="bottom-left"><button class="graph-topbar__button" aria-label="Export image" on:click={onExportImage}>
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
    gap: 12px;
    padding: 0 8px 0 10px;
    border: 0;
    background: transparent;
  }

  .graph-topbar__tools, .graph-topbar__global { display: flex; align-items: center; gap: 3px; }
  .graph-topbar--global-only { justify-content: flex-end; }
  .graph-topbar--tools-only { justify-content: flex-end; }
  .graph-topbar__search-wrap { position: relative; display: flex; align-items: center; }
  .graph-topbar__search-wrap > :global(.relative) { position: static; }
  .graph-topbar__tools { padding: 3px; border: 1px solid var(--border-strong); border-radius: 7px; background: var(--panel-bg-alt); }
  .graph-topbar__button, .graph-topbar__share { display: inline-flex; height: 28px; align-items: center; justify-content: center; border: 0; border-radius: 5px; color: var(--text-primary); background: transparent; }
  .graph-topbar__button { width: 26px; transition: color 140ms ease, background-color 140ms ease, box-shadow 140ms ease; }
  .graph-topbar__share { gap: 5px; padding: 0 8px; font-size: 12px; font-weight: 650; letter-spacing: .01em; }
  .graph-topbar__button:hover { color: var(--text-primary); background: var(--panel-bg); box-shadow: 0 1px 3px rgb(29 39 53 / 9%); }
  .graph-topbar__share:hover { color: var(--text-primary); background: var(--panel-bg-alt); }

  @media (prefers-reduced-motion: no-preference) {
    .graph-topbar__button { transition-property: color, background-color, box-shadow, transform; }
    .graph-topbar__button:hover { transform: translateY(-1px); }
  }
  .graph-topbar__account { display: flex; align-items: center; }
  .graph-topbar__account :global([data-slot='button']) { width: 27px; height: 27px; border: 0; border-radius: 5px; }
  .graph-topbar__account :global([data-slot='button']:hover) { background: var(--panel-bg-alt); }
</style>
