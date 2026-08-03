<script lang="ts">
  import { Check, ChevronDown, ChevronLeft, ChevronRight, ChevronUp, CircleAlert, Cloud, CloudOff, CloudUpload, Copy, LoaderCircle, MoreHorizontal, Pencil, Plus, X } from 'lucide-svelte'
  import { onMount } from 'svelte'
  import { activeTempModel } from '../store/diagnostics-store'
  import { buildReadablePath, isPathSegIndex, pathSegKeyValue, type PathSeg } from '../store/tree-path'
  import type { ColumnNavigatorState } from './graph-viewer/column-navigator/types'
  import Tooltip from './Tooltip.svelte'

  export let pane: 'editor' | 'graph' | 'both' = 'both'
  export let fileName = 'Untitled'
  type CloudSyncStatus = 'synced' | 'syncing' | 'pending' | 'error' | 'offline'

  export let tabs: Array<{ id: string; name: string; languageId: string; dirty?: boolean; syncStatus?: CloudSyncStatus }> = []
  export let activeTabId = ''
  export let onActivateTab: (id: string) => void = () => {}
  export let onRenameTab: (id: string, name: string) => void = () => {}
  export let onCloseTab: (id: string) => void = () => {}
  export let canAddTab = true
  export let onAddTab: () => void = () => {}
  export let onShowAiInput: () => void | Promise<void> = () => {}
  export let onTreePathSelect: (path: PathSeg[]) => void = () => {}
  export let onRevealError: (line: number, column: number) => void = () => {}
  export let onColumnNavigatorBack: () => void | Promise<void> = () => {}
  export let onColumnNavigatorForward: () => void | Promise<void> = () => {}
  export let onCollapseColumnNavigator: () => void = () => {}
  export let onPinColumnNavigatorCollapsed: () => void = () => {}
  export let onExpandColumnNavigator: () => void = () => {}
  export let columnNavigatorState: ColumnNavigatorState | null = null
  export let graphVisible = true
  export let surfaceMode: 'graph' | 'compare' = 'graph'
  export let embedded = false
  // Kept for URL-command and host compatibility while actions now live in the sidebar.
  export let editorWidthPx = 0
  export let onFormat: () => void | Promise<void> = () => {}
  export let onMinify: () => void | Promise<void> = () => {}
  export let onCompact: () => void | Promise<void> = () => {}
  export let onSort: () => void | Promise<void> = () => {}
  export let onShowYqInput: () => void | Promise<void> = () => {}
  export let onGenerateStruct: () => void | Promise<void> = () => {}
  export let onEscape: () => void | Promise<void> = () => {}
  export let onUnescape: () => void | Promise<void> = () => {}
  export let onNewDocument: () => void | Promise<void> = () => {}
  export let onOpenDocument: () => void | Promise<void> = () => {}
  export let onSaveDocument: () => void | Promise<void> = () => {}
  export let onSaveAsDocument: () => void | Promise<void> = () => {}
  export let onCloseDocument: () => void | Promise<void> = () => {}

  let copiedTreePath = false
  let copyFeedbackFading = false
  let tabsOpen = false
  let tabSwitcher: HTMLDivElement | null = null
  let renamingTabId: string | null = null
  let tabActionMenuId: string | null = null
  let tabActionMenuPosition = { left: 0, top: 0 }
  let renamedTabName = ''
  let treePathCopyTimer: ReturnType<typeof setTimeout> | null = null
  let treePathFadeTimer: ReturnType<typeof setTimeout> | null = null
  $: treePath = $activeTempModel?.treePath ?? []
  $: displayedPath = treePath
  $: documentInvalid = Boolean($activeTempModel?.error) || ($activeTempModel?.diagnostics?.length ?? 0) > 0
  $: showTreePathbar = pane === 'graph' && surfaceMode === 'graph' && graphVisible

  onMount(() => {
    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target as Node | null
      if (tabsOpen && tabSwitcher && target && !tabSwitcher.contains(target)) {
        tabsOpen = false
        tabActionMenuId = null
      }
    }
    document.addEventListener('pointerdown', handlePointerDown)
    return () => document.removeEventListener('pointerdown', handlePointerDown)
  })

  function beginTabRename(tab: { id: string; name: string }): void {
    renamingTabId = tab.id
    tabActionMenuId = null
    renamedTabName = tab.name
  }

  function closeTab(tabId: string): void {
    onCloseTab(tabId)
    tabActionMenuId = null
    tabsOpen = false
  }

  function toggleTabActionMenu(tabId: string, trigger: HTMLButtonElement): void {
    if (tabActionMenuId === tabId) {
      tabActionMenuId = null
      return
    }
    const rect = trigger.getBoundingClientRect()
    tabActionMenuPosition = {
      left: Math.max(8, rect.right - 116),
      top: Math.max(8, rect.top - 64),
    }
    tabActionMenuId = tabId
  }

  function syncStatusLabel(status: CloudSyncStatus): string {
    return {
      synced: 'Synced to cloud',
      syncing: 'Syncing to cloud',
      pending: 'Local changes pending cloud sync',
      error: 'Cloud sync failed',
      offline: 'Cloud sync is offline',
    }[status]
  }

  function commitTabRename(tab: { id: string; name: string }): void {
    if (renamingTabId !== tab.id) return
    const name = renamedTabName.trim()
    if (name && name !== tab.name) onRenameTab(tab.id, name)
    renamingTabId = null
    renamedTabName = ''
  }

  function cancelTabRename(): void {
    renamingTabId = null
    renamedTabName = ''
  }

  function focusRenameInput(node: HTMLInputElement): void {
    queueMicrotask(() => node.focus())
  }

  function treePathLabel(path: PathSeg[]): string {
    if (!path.length) return '$'
    const segment = path[path.length - 1]
    return isPathSegIndex(segment) ? `[${segment.index}]` : pathSegKeyValue(segment)
  }

  function buildTreePathPrefixes(path: PathSeg[]): PathSeg[][] {
    return Array.from({ length: path.length + 1 }, (_, index) => path.slice(0, index))
  }

  function revealFirstDiagnostic(): void {
    const diagnostic = $activeTempModel?.diagnostics?.[0]
    if (!diagnostic) return
    onRevealError(diagnostic.startLineNumber, diagnostic.startColumn)
  }

  async function copyTreePath(): Promise<void> {
    if (!navigator.clipboard) return
    await navigator.clipboard.writeText(buildReadablePath(displayedPath))
    copiedTreePath = true
    copyFeedbackFading = false
    if (treePathCopyTimer) clearTimeout(treePathCopyTimer)
    if (treePathFadeTimer) clearTimeout(treePathFadeTimer)
    treePathCopyTimer = setTimeout(() => {
      copyFeedbackFading = true
      treePathCopyTimer = null
      treePathFadeTimer = setTimeout(() => {
        copiedTreePath = false
        copyFeedbackFading = false
        treePathFadeTimer = null
      }, 180)
    }, 1000)
  }

</script>

{#if pane === 'editor'}
  <footer class="editor-bottombar" data-testid="editor-bottombar">
    <div class="editor-bottombar__tabs" bind:this={tabSwitcher}>
      <button
        type="button"
        class="editor-bottombar__file"
        title="Switch open tab"
        aria-label={`Switch open tab: ${fileName}`}
        aria-haspopup="listbox"
        aria-expanded={tabsOpen}
        data-testid="editor-tab-switcher"
        on:click={() => (tabsOpen = !tabsOpen)}
      >
        <span>{fileName}</span><ChevronDown size={12} />
      </button>
      {#if tabsOpen}
        <div class="editor-bottombar__tab-menu" role="listbox" aria-label="Open tabs" data-testid="editor-tab-menu">
          {#each tabs as tab (tab.id)}
            <div
              role="option"
              aria-selected={tab.id === activeTabId}
              class:active={tab.id === activeTabId}
              class="editor-bottombar__tab-option"
              data-testid={`editor-tab-option-${tab.id}`}
            >
              {#if renamingTabId === tab.id}
                <input
                  class="editor-bottombar__tab-rename"
                  aria-label={`Rename ${tab.name}`}
                  data-testid={`editor-tab-rename-${tab.id}`}
                  bind:value={renamedTabName}
                  use:focusRenameInput
                  on:click|stopPropagation
                  on:blur={() => commitTabRename(tab)}
                  on:keydown={(event) => {
                    if (event.key === 'Enter') commitTabRename(tab)
                    if (event.key === 'Escape') cancelTabRename()
                  }}
                />
              {:else}
                <button type="button" class="editor-bottombar__tab-option-open" title={`Open ${tab.name}`} on:click={() => { onActivateTab(tab.id); tabsOpen = false }} on:dblclick|stopPropagation={() => beginTabRename(tab)}>
                  {#if tab.syncStatus}
                    <span class={`editor-bottombar__tab-sync editor-bottombar__tab-sync--${tab.syncStatus}`} title={syncStatusLabel(tab.syncStatus)} aria-label={syncStatusLabel(tab.syncStatus)}>
                      {#if tab.syncStatus === 'synced'}<Cloud size={12} />
                      {:else if tab.syncStatus === 'syncing'}<LoaderCircle size={12} />
                      {:else if tab.syncStatus === 'pending'}<CloudUpload size={12} />
                      {:else if tab.syncStatus === 'error'}<CircleAlert size={12} />
                      {:else}<CloudOff size={12} />{/if}
                    </span>
                  {/if}
                  <span class="editor-bottombar__tab-name">{tab.name}</span>{#if tab.dirty}<span class="editor-bottombar__tab-dirty">•</span>{/if}
                </button>
                <button
                  type="button"
                  class="editor-bottombar__tab-actions-button"
                  aria-label={`Actions for ${tab.name}`}
                  aria-expanded={tabActionMenuId === tab.id}
                  aria-haspopup="menu"
                  data-testid={`editor-tab-actions-${tab.id}`}
                  on:click|stopPropagation={(event) => toggleTabActionMenu(tab.id, event.currentTarget as HTMLButtonElement)}
                ><MoreHorizontal size={14} /></button>
                {#if tabActionMenuId === tab.id}
                  <div class="editor-bottombar__tab-actions-menu" role="menu" aria-label={`Actions for ${tab.name}`} data-testid={`editor-tab-actions-menu-${tab.id}`} style:left={`${tabActionMenuPosition.left}px`} style:top={`${tabActionMenuPosition.top}px`}>
                    <button type="button" role="menuitem" data-testid={`editor-tab-action-rename-${tab.id}`} on:click|stopPropagation={() => beginTabRename(tab)}><Pencil size={12} />Rename</button>
                    <button type="button" role="menuitem" class="editor-bottombar__tab-actions-menu-close" data-testid={`editor-tab-action-close-${tab.id}`} on:click|stopPropagation={() => closeTab(tab.id)}><X size={12} />Close</button>
                  </div>
                {/if}
              {/if}
            </div>
          {/each}
          {#if canAddTab}
            <button type="button" class="editor-bottombar__new-tab" data-testid="editor-tab-new" on:click={() => { onAddTab(); tabsOpen = false }}><Plus size={12} />New tab</button>
          {/if}
        </div>
      {/if}
    </div>
    <span class="editor-bottombar__spacer"></span>
    <span class="editor-bottombar__cursor">{$activeTempModel?.cursor ?? 'Ln 1, Col 1'}{#if ($activeTempModel?.selectionLength ?? 0) > 0}{' '}<span>({$activeTempModel?.selectionLength} selected)</span>{/if}</span>
    <Tooltip content={documentInvalid ? 'Document has errors' : 'Document is valid'} side="top"><button type="button" class:editor-bottombar__invalid={documentInvalid} class="editor-bottombar__valid" disabled={!documentInvalid} on:click={revealFirstDiagnostic}>{#if documentInvalid}<CircleAlert size={13} />Invalid{:else}<Check size={13} />Valid{/if}</button></Tooltip>
  </footer>
{:else if pane === 'graph'}
  <footer class="graph-bottombar" class:graph-bottombar--embedded={embedded} data-testid="graph-bottombar">
    {#if showTreePathbar}
      <div class="graph-tree-pathbar group" data-testid="bottom-tree-pathbar">
        {#if columnNavigatorState}
          <div class="graph-tree-pathbar__history">
            <Tooltip content="Back in workspace history" side="top-left"><button
              type="button"
              class="graph-tree-pathbar__history-button"
              aria-label="Back in workspace history"
              data-testid="bottom-column-navigator-back"
              disabled={!columnNavigatorState.canGoBack}
              on:click={() => void onColumnNavigatorBack()}
            ><ChevronLeft size={13} strokeWidth={2} /></button></Tooltip>
            <Tooltip content="Forward in workspace history" side="top-left"><button
              type="button"
              class="graph-tree-pathbar__history-button"
              aria-label="Forward in workspace history"
              data-testid="bottom-column-navigator-forward"
              disabled={!columnNavigatorState.canGoForward}
              on:click={() => void onColumnNavigatorForward()}
            ><ChevronRight size={13} strokeWidth={2} /></button></Tooltip>
          </div>
        {/if}
        <div class="graph-tree-pathbar__scroll">
          {#each buildTreePathPrefixes(displayedPath) as prefix, index (index)}
            <Tooltip content={buildReadablePath(prefix)} side="bottom"><button
              type="button"
              class:active={index === displayedPath.length}
              aria-current={index === displayedPath.length ? 'location' : undefined}
              data-testid={`tree-path-crumb-${index}`}
              on:click={() => onTreePathSelect(prefix)}
            >{treePathLabel(prefix)}</button></Tooltip>
            {#if index < displayedPath.length}<ChevronRight size={12} strokeWidth={1.8} aria-hidden="true" />{/if}
          {/each}
        </div>
        <Tooltip content={copiedTreePath ? 'Copied' : 'Copy tree path'} side="top-left"><button
          type="button"
          class="graph-tree-path-copy"
          class:copied={copiedTreePath}
          class:fading={copyFeedbackFading}
          aria-label={copiedTreePath ? 'Tree path copied' : 'Copy tree path'}
          data-testid="bottom-tree-path-copy"
          on:click={() => void copyTreePath()}
        >
          {#if copiedTreePath}<Check size={13} />{:else}<Copy size={13} />{/if}
        </button></Tooltip>
        {#if columnNavigatorState}
          <Tooltip
            className="graph-tree-path-expand-tooltip"
            content={columnNavigatorState.collapsed ? 'Expand column navigator' : 'Collapse column navigator'}
            side="top-left"
          ><button
            type="button"
            class="graph-tree-path-expand"
            aria-label={columnNavigatorState.collapsed ? 'Expand column navigator' : 'Collapse column navigator'}
            data-testid={columnNavigatorState.collapsed ? 'column-navigator-expand' : 'column-navigator-collapse'}
            on:click={columnNavigatorState.collapsed ? onExpandColumnNavigator : onCollapseColumnNavigator}
          >{#if columnNavigatorState.collapsed}<ChevronUp size={14} strokeWidth={2} />{:else}<ChevronDown size={14} strokeWidth={2} />{/if}</button></Tooltip>
          <Tooltip
            className="graph-tree-path-close-tooltip"
            content={columnNavigatorState.collapsed ? 'Navigator already collapsed' : 'Keep navigator collapsed'}
            side="top-left"
          ><button
            type="button"
            class="graph-tree-path-close"
            aria-label="Keep navigator collapsed"
            data-testid="column-navigator-pin-collapsed"
            disabled={columnNavigatorState.collapsed}
            on:click={onPinColumnNavigatorCollapsed}
          ><X size={14} strokeWidth={2} /></button></Tooltip>
        {/if}
      </div>
    {/if}
  </footer>
{/if}

<style>
  .editor-bottombar, .graph-bottombar { display: flex; min-width: 0; height: var(--bottombar-height); align-items: center; border-top: 1px solid var(--border-strong); background: var(--bottombar-bg); color: var(--text-muted); font-size: 12px; }
  .editor-bottombar { gap: 8px; padding: 0 10px; }
  .editor-bottombar__tabs { position: relative; min-width: 0; }
  .editor-bottombar__file { display: inline-flex; max-width: min(260px, 34vw); min-width: 0; align-items: center; gap: 4px; overflow: hidden; border: 0; border-radius: 5px; padding: 2px 5px; color: var(--text-muted); background: transparent; font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .editor-bottombar__file span { overflow: hidden; text-overflow: ellipsis; }
  .editor-bottombar__file:hover, .editor-bottombar__file[aria-expanded='true'] { color: var(--text-primary); background: var(--panel-bg-alt); }
  .editor-bottombar__tab-menu { position: absolute; z-index: 60; bottom: calc(100% + 7px); left: 0; display: flex; width: 220px; max-height: min(320px, 48vh); flex-direction: column; gap: 2px; overflow-y: auto; padding: 5px; border: 1px solid var(--border-strong); border-radius: 8px; background: var(--panel-bg); box-shadow: 0 12px 28px rgb(29 39 53 / 16%); }
  .editor-bottombar__tab-option { position: relative; display: flex; width: 100%; min-height: 28px; align-items: center; gap: 4px; overflow: visible; border-radius: 5px; padding: 0 3px 0 8px; color: var(--text-muted); background: transparent; font-size: 11px; text-align: left; }
  .editor-bottombar__tab-option:hover, .editor-bottombar__tab-option.active { color: var(--text-primary); background: var(--accent-soft); }
  .editor-bottombar__tab-option-open { display: flex; min-width: 0; flex: 1; align-items: center; gap: 4px; overflow: hidden; border: 0; padding: 0; color: inherit; background: transparent; font-size: inherit; text-align: left; }
  .editor-bottombar__tab-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .editor-bottombar__tab-actions-button { display: inline-flex; width: 24px; height: 24px; flex: 0 0 24px; align-items: center; justify-content: center; border: 0; border-radius: 4px; color: var(--text-muted); background: transparent; opacity: .7; }
  .editor-bottombar__tab-option:hover .editor-bottombar__tab-actions-button, .editor-bottombar__tab-actions-button:focus-visible, .editor-bottombar__tab-actions-button[aria-expanded='true'] { opacity: 1; }
  .editor-bottombar__tab-actions-button:hover, .editor-bottombar__tab-actions-button[aria-expanded='true'] { color: var(--text-primary); background: var(--panel-bg); }
  .editor-bottombar__tab-actions-menu { position: fixed; z-index: 10000; display: grid; min-width: 116px; padding: 4px; border: 1px solid var(--border-strong); border-radius: 6px; background: var(--panel-bg); box-shadow: 0 8px 18px rgb(29 39 53 / 16%); }
  .editor-bottombar__tab-actions-menu button { display: flex; min-height: 26px; align-items: center; gap: 5px; border: 0; border-radius: 4px; padding: 0 7px; color: var(--text-primary); background: transparent; font: inherit; text-align: left; }
  .editor-bottombar__tab-actions-menu button:hover, .editor-bottombar__tab-actions-menu button:focus-visible { background: var(--panel-bg-alt); }
  .editor-bottombar__tab-actions-menu-close { color: var(--danger) !important; }
  .editor-bottombar__tab-sync { display: inline-flex; flex: 0 0 auto; align-items: center; color: var(--text-muted); }
  .editor-bottombar__tab-sync--synced { color: #277558; }
  .editor-bottombar__tab-sync--syncing { color: var(--accent); }
  .editor-bottombar__tab-sync--syncing :global(svg) { animation: editor-bottombar-sync-spin 1s linear infinite; }
  .editor-bottombar__tab-sync--pending { color: #9a6a1c; }
  .editor-bottombar__tab-sync--error { color: var(--danger); }
  .editor-bottombar__tab-sync--offline { color: var(--text-muted); }
  .editor-bottombar__tab-rename { width: 100%; min-width: 0; border: 1px solid var(--accent); border-radius: 4px; outline: none; padding: 3px 5px; color: var(--text-primary); background: var(--panel-bg); font: inherit; }
  @keyframes editor-bottombar-sync-spin { to { transform: rotate(360deg); } }
  .editor-bottombar__tab-dirty { color: var(--accent); }
  .editor-bottombar__new-tab { display: inline-flex; min-height: 28px; align-items: center; gap: 5px; margin-top: 3px; border-top: 1px solid var(--border-muted); padding: 5px 8px 2px; color: var(--text-muted); background: transparent; font-size: 11px; text-align: left; }
  .editor-bottombar__new-tab:hover { color: var(--text-primary); }
  .editor-bottombar__spacer { flex: 1; min-width: 0; }
  .editor-bottombar__cursor { flex: 0 0 auto; color: var(--text-muted); font-size: 11px; text-align: right; white-space: nowrap; }
  .editor-bottombar__cursor span { color: inherit; }
  .editor-bottombar__valid { display: inline-flex; flex: 0 0 auto; align-items: center; gap: 4px; color: #277558; font-size: 11px; }
  .editor-bottombar__valid.editor-bottombar__invalid { color: var(--danger); }
  .bottombar-actions { display: inline-flex; align-items: center; gap: 3px; }
  .bottombar-ai, .bottombar-action { display: inline-flex; height: 23px; align-items: center; justify-content: center; border: 0; border-radius: 6px; }
  .bottombar-ai { gap: 4px; padding: 0 7px; color: #7b5424; background: #f8efdf; font-size: 10px; }
  .bottombar-ai:hover { background: #f3e4c9; }
  .bottombar-action { width: 25px; color: var(--text-muted); background: transparent; }
  .bottombar-action:hover { color: var(--text-primary); background: var(--panel-bg-alt); }
  .graph-bottombar { padding: 0 10px; background: var(--panel-bg); box-shadow: inset 0 1px 0 rgb(255 255 255 / 62%); }
  .graph-bottombar--embedded { border: 0; box-shadow: none; }
  .graph-tree-pathbar { display: flex; width: 100%; min-width: 0; align-items: center; gap: 5px; }
  :global(.graph-tree-path-expand-tooltip) { display: inline-flex; width: 24px; height: 24px; flex: 0 0 24px; order: 4; }
  .graph-tree-path-expand { display: inline-flex; width: 100%; height: 100%; align-items: center; justify-content: center; border: 0; border-radius: 5px; color: var(--text-muted); background: transparent; }
  .graph-tree-path-expand:hover, .graph-tree-path-expand:focus-visible { color: var(--text-primary); background: var(--panel-bg-alt); }
  :global(.graph-tree-path-close-tooltip) { display: inline-flex; width: 24px; height: 24px; flex: 0 0 24px; order: 5; }
  .graph-tree-path-close { display: inline-flex; width: 100%; height: 100%; align-items: center; justify-content: center; border: 0; border-radius: 5px; color: var(--text-muted); background: transparent; }
  .graph-tree-path-close:hover, .graph-tree-path-close:focus-visible { color: var(--text-primary); background: var(--panel-bg-alt); }
  .graph-tree-path-close:disabled { cursor: default; opacity: .35; }
  .graph-tree-pathbar__history { display: inline-flex; order: 3; flex: 0 0 auto; align-items: center; gap: 3px; margin-left: auto; padding-left: 4px; }
  /* Match the graph toolbar target: 26 × 28 px, including the border. */
  .graph-tree-pathbar__history-button, .graph-tree-path-copy { box-sizing: border-box; width: 26px; height: 28px; padding: 0; }
  .graph-tree-pathbar__history-button { display: inline-flex; align-items: center; justify-content: center; border: 1px solid transparent; border-radius: 5px; color: var(--text-muted); background: transparent; }
  .graph-tree-pathbar__history-button:hover { border-color: var(--border-strong); color: var(--text-primary); background: var(--panel-bg-alt); }
  .graph-tree-pathbar__history-button:disabled { cursor: default; color: var(--border-strong); }
  .graph-tree-pathbar__history-button:disabled:hover { border-color: transparent; background: transparent; }
  .graph-tree-pathbar__scroll { display: flex; min-width: 0; max-width: calc(100% - 78px); flex: 0 1 auto; align-items: center; gap: 3px; overflow-x: auto; white-space: nowrap; scrollbar-width: none; }
  .graph-tree-pathbar__scroll::-webkit-scrollbar { display: none; }
  .graph-tree-pathbar__scroll button { max-width: 180px; overflow: hidden; border: 1px solid transparent; border-radius: 5px; padding: 3px 6px; color: var(--text-muted); background: transparent; font: 11px ui-monospace, monospace; text-overflow: ellipsis; transition: color 120ms ease, background-color 120ms ease, border-color 120ms ease; }
  .graph-tree-pathbar__scroll button:hover, .graph-tree-pathbar__scroll button.active { border-color: color-mix(in srgb, var(--accent) 24%, var(--border-strong)); color: var(--accent); background: var(--accent-soft); }
  .graph-tree-path-copy { display: inline-flex; order: 2; flex: 0 0 auto; align-items: center; justify-content: center; margin-left: 2px; border: 0; border-radius: 5px; color: var(--text-muted); background: transparent; opacity: 0; transition: opacity 180ms ease-out, background-color 180ms ease-out, color 180ms ease-out; }
  .graph-tree-pathbar:hover .graph-tree-path-copy, .graph-tree-path-copy:focus-visible, .graph-tree-path-copy.copied { opacity: 1; }
  .graph-tree-path-copy:hover { color: var(--text-primary); background: var(--panel-bg-alt); }
  .graph-tree-path-copy.copied { color: #13775c; background: #e0f0e8; }
  .graph-tree-path-copy.fading { opacity: 0 !important; }
</style>
