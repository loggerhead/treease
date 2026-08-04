<script lang="ts">
import { Cloud, CloudOff, CloudUpload, CircleAlert, LoaderCircle, Plus, X } from 'lucide-svelte'
  import type { CommandId } from '../command-registry'
  import { activeTempModel } from '../store/graph-selection-store'
  import CommandPalette from './CommandPalette.svelte'

  type CloudSyncStatus = 'synced' | 'syncing' | 'pending' | 'error' | 'offline'
  export let tabs: Array<{ id: string; name: string; languageId: string; dirty?: boolean; syncStatus?: CloudSyncStatus }> = []
  export let activeTabId = ''
  export let showTabDirty = false
  export let canAddTab = true
  export let showTabs = true
  export let placement: 'top' | 'bottom' | 'sidebar' = 'top'
  export let onAddTab: () => void = () => {}
  export let onCloseTab: (id: string) => void = () => {}
  export let onActivateTab: (id: string) => void = () => {}
  export let onRenameTab: (id: string, name: string) => void = () => {}
  export let onCommandExecute: (id: CommandId) => void | Promise<void> = () => {}
  export let showCommandSearch = true

  let renamingTabId: string | null = null
  let renamedTabName = ''
  let commandQuery = ''

  $: if ($activeTempModel && $activeTempModel.commandQuery !== commandQuery) commandQuery = $activeTempModel.commandQuery
  $: directTabs = tabs

  function updateCommandQuery(value: string) {
    commandQuery = value
    activeTempModel.update((current) => ({ ...current, commandQuery: value }))
  }

  function beginTabRename(tab: { id: string; name: string }) {
    onActivateTab(tab.id)
    renamingTabId = tab.id
    renamedTabName = tab.name
  }

  function commitTabRename(tab: { id: string; name: string }) {
    if (renamingTabId !== tab.id) return
    const name = renamedTabName.trim()
    if (name && name !== tab.name) onRenameTab(tab.id, name)
    renamingTabId = null
    renamedTabName = ''
  }

  function cancelTabRename() {
    renamingTabId = null
    renamedTabName = ''
  }

  function focusRenameInput(node: HTMLInputElement) {
    queueMicrotask(() => node.focus())
  }

  function syncStatusLabel(status: CloudSyncStatus): string {
    return { synced: 'Synced to cloud', syncing: 'Syncing to cloud', pending: 'Local changes pending cloud sync', error: 'Cloud sync failed', offline: 'Cloud sync is offline' }[status]
  }
</script>

<header class:tab-switcher--sidebar={placement === 'sidebar'} class:tab-switcher--bottom={placement === 'bottom'} class="tab-switcher" data-testid="tab-switcher">
  {#if showCommandSearch}
    <div class="tab-switcher__command">
      <CommandPalette compact value={commandQuery} on:input={(event) => updateCommandQuery(event.detail)} onExecute={onCommandExecute} />
    </div>
  {/if}
  {#if showTabs}
    <div class="tab-switcher__tabs" data-testid="editor-tab-strip">
      <div class="tab-switcher__tab-list">
      {#each directTabs as tab (tab.id)}
        <div
          class={`editor-tab ${tab.id === activeTabId ? 'editor-tab--active' : ''} ${renamingTabId === tab.id ? 'editor-tab--renaming' : ''}`}
          data-testid="editor-tab"
          data-tab-id={tab.id}
          data-active={tab.id === activeTabId}
          data-renaming={renamingTabId === tab.id}
        >
          {#if renamingTabId === tab.id}
            <div class="editor-tab__rename-wrap">
              <span class="editor-tab__measure">{showTabDirty && tab.dirty ? `${tab.name} •` : tab.name}</span>
              <input
                class="editor-tab__rename"
                aria-label={`Rename ${tab.name}`}
                data-testid={`tab-rename-${tab.id}`}
                bind:value={renamedTabName}
                use:focusRenameInput
                on:blur={() => commitTabRename(tab)}
                on:keydown={(event) => {
                  if (event.key === 'Enter') commitTabRename(tab)
                  if (event.key === 'Escape') cancelTabRename()
                }}
              />
            </div>
          {:else}
            <button
              class="editor-tab__open"
              aria-label={`Open ${tab.name}`}
              title={`Open ${tab.name}`}
              data-testid={`tab-open-${tab.id}`}
              on:click={() => onActivateTab(tab.id)}
              on:dblclick|stopPropagation={() => beginTabRename(tab)}
            >{#if tab.syncStatus}<span class={`editor-tab__sync editor-tab__sync--${tab.syncStatus}`} title={syncStatusLabel(tab.syncStatus)} aria-label={syncStatusLabel(tab.syncStatus)}>{#if tab.syncStatus === 'synced'}<Cloud size={11} />{:else if tab.syncStatus === 'syncing'}<LoaderCircle size={11} />{:else if tab.syncStatus === 'pending'}<CloudUpload size={11} />{:else if tab.syncStatus === 'error'}<CircleAlert size={11} />{:else}<CloudOff size={11} />{/if}</span>{/if}{showTabDirty && tab.dirty ? `${tab.name} •` : tab.name}</button>
          {/if}
          <button
            class="editor-tab__close"
            aria-label={`Close ${tab.name}`}
            title="Close tab"
            data-testid={`tab-close-${tab.id}`}
            on:click={() => onCloseTab(tab.id)}
          ><X size={11} /></button>
        </div>
      {/each}
      </div>

      {#if canAddTab}
        <button class="editor-tab__new" aria-label="New tab" title="New tab" data-testid="new-tab-button" on:click={onAddTab}>
          <Plus size={14} />
        </button>
      {/if}
    </div>
  {/if}
</header>

<style>
  .tab-switcher {
    position: relative;
    z-index: 20;
    display: flex;
    min-width: 0;
    height: var(--topbar-height);
    align-items: center;
    gap: var(--space-3);
    padding: 0 var(--space-4);
    border-bottom: 1px solid var(--border-strong);
    background: var(--topbar-bg);
    box-shadow: 0 1px 0 rgb(29 39 53 / 2%);
  }

  .tab-switcher--sidebar {
    height: auto;
    min-height: 0;
    flex: 0 0 auto;
    align-items: stretch;
    padding: 0;
    border: 0;
    background: transparent;
    box-shadow: none;
  }

  .tab-switcher--bottom {
    height: var(--topbar-height);
    flex: 0 0 var(--topbar-height);
    border-top: 1px solid var(--border-strong);
    border-bottom: 0;
  }

  .tab-switcher--sidebar .tab-switcher__tabs {
    flex-direction: column;
    align-items: stretch;
    gap: var(--space-2);
  }

  .tab-switcher--sidebar .tab-switcher__tab-list {
    width: 100%;
    flex: 0 0 auto;
    flex-direction: column;
    align-items: stretch;
    gap: var(--space-2);
    overflow: visible;
  }

  .tab-switcher--sidebar .editor-tab,
  .tab-switcher--sidebar .editor-tab__new,
  .tab-switcher--sidebar .editor-tab__more {
    width: 100%;
  }


  .tab-switcher__command {
    position: relative;
    display: flex;
    flex: 0 0 auto;
    align-items: center;
  }

  .tab-switcher__tabs {
    display: flex;
    min-width: 0;
    margin-left: 0;
    flex: 1 1 auto;
    align-items: center;
    gap: var(--space-1);
    overflow: visible;
  }

  .tab-switcher__tab-list {
    display: flex;
    min-width: 0;
    flex: 1 1 auto;
    align-items: center;
    gap: var(--space-1);
    overflow-x: auto;
    scrollbar-width: none;
  }

  .tab-switcher__tab-list::-webkit-scrollbar { display: none; }

  .editor-tab {
    display: inline-flex;
    min-width: 0;
    height: 28px;
    flex: 0 0 auto;
    align-items: center;
    gap: var(--space-2);
    border: 1px solid transparent;
    border-radius: var(--control-radius);
    padding: 0 var(--space-3) 0 var(--space-4);
    color: var(--text-muted);
    transition: var(--control-transition);
  }

  .editor-tab:hover { background: var(--panel-bg-alt); color: var(--text-primary); }
  .editor-tab--active { border-color: color-mix(in srgb, var(--accent) 44%, var(--border-strong)); background: var(--accent-soft); color: var(--text-primary); box-shadow: 0 1px 2px rgb(24 59 86 / 5%); }
  .editor-tab--renaming { border-color: var(--accent); box-shadow: var(--focus-ring); }

  .editor-tab__open, .editor-tab__close, .editor-tab__new {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 0;
    background: transparent;
    color: inherit;
  }

  .editor-tab__open { display: block; min-width: 0; max-width: 150px; flex: 0 1 auto; overflow: hidden; padding: 0; text-overflow: ellipsis; white-space: nowrap; font-size: var(--font-size-control); }
  .editor-tab__sync { display: inline-flex; flex: 0 0 auto; align-items: center; color: var(--text-muted); }
  .editor-tab__sync--synced { color: #277558; }
  .editor-tab__sync--syncing { color: var(--accent); }
  .editor-tab__sync--pending { color: #9a6a1c; }
  .editor-tab__sync--error { color: var(--danger); }
  .editor-tab__sync--offline { color: var(--text-muted); }
  .editor-tab__close { width: 18px; height: 18px; border-radius: 4px; }
  .editor-tab__close:hover { background: color-mix(in srgb, var(--accent) 11%, transparent); color: var(--accent); }
  .editor-tab__new { width: 24px; height: 24px; flex: 0 0 auto; order: 2; margin-left: var(--space-1); border-radius: 4px; color: var(--text-muted); transition: var(--control-transition); }
  .editor-tab__new:hover { color: var(--accent); background: var(--accent-soft); }
  .editor-tab__new:focus-visible { outline: none; box-shadow: var(--focus-ring); }
  .editor-tab__rename-wrap { position: relative; min-width: 0; }
  .editor-tab__measure { visibility: hidden; white-space: pre; font-size: var(--font-size-control); }
  .editor-tab__rename { position: absolute; inset: 0; width: 100%; min-width: 70px; border: 0; outline: 0; background: transparent; color: inherit; font-size: var(--font-size-control); }
  .editor-tab:focus-within { box-shadow: var(--focus-ring); }

  @media (max-width: 620px) {
    .tab-switcher__tabs { gap: 3px; }
    .editor-tab__open { max-width: 112px; }
  }
</style>
