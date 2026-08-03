<script lang="ts">
  import { MoreHorizontal, Plus, X } from 'lucide-svelte'
  import type { CommandId } from '../command-registry'
  import { activeTempModel } from '../store/graph-selection-store'
  import CommandSearchInput from './CommandSearchInput.svelte'

  export let tabs: Array<{ id: string; name: string; languageId: string; dirty?: boolean }> = []
  export let activeTabId = ''
  export let showTabDirty = false
  export let canAddTab = true
  export let showTabs = true
  export let placement: 'top' | 'sidebar' = 'top'
  export let onAddTab: () => void = () => {}
  export let onCloseTab: (id: string) => void = () => {}
  export let onActivateTab: (id: string) => void = () => {}
  export let onRenameTab: (id: string, name: string) => void = () => {}
  export let onCommandExecute: (id: CommandId) => void | Promise<void> = () => {}
  export let showCommandSearch = true

  let renamingTabId: string | null = null
  let renamedTabName = ''
  let commandQuery = ''
  let tabMenuOpen = false

  $: if ($activeTempModel && $activeTempModel.commandQuery !== commandQuery) commandQuery = $activeTempModel.commandQuery
  $: directTabs = tabs

  function activateTabFromMenu(id: string) {
    onActivateTab(id)
    tabMenuOpen = false
  }

  function closeTabFromMenu(id: string) {
    onCloseTab(id)
    tabMenuOpen = false
  }

  function closeTabMenu() {
    tabMenuOpen = false
  }

  function handleWindowKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') closeTabMenu()
  }

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
</script>

<svelte:window on:keydown={handleWindowKeydown} on:click={closeTabMenu} />

<header class:editor-topbar--sidebar={placement === 'sidebar'} class="editor-topbar" data-testid="editor-topbar">
  {#if showCommandSearch}
    <div class="editor-topbar__command">
      <CommandSearchInput compact value={commandQuery} on:input={(event) => updateCommandQuery(event.detail)} onExecute={onCommandExecute} />
    </div>
  {/if}
  {#if showTabs}
    <div class="editor-topbar__tabs" data-testid="editor-tab-strip">
      <div class="editor-topbar__tab-list">
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
            >{showTabDirty && tab.dirty ? `${tab.name} •` : tab.name}</button>
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

      <div class="editor-tab-menu" on:click|stopPropagation>
          <button
            class="editor-tab__more"
            aria-label="Show open tabs"
            aria-expanded={tabMenuOpen}
            aria-haspopup="menu"
            title="Show open tabs"
            data-testid="tab-menu-button"
            on:click={() => (tabMenuOpen = !tabMenuOpen)}
          >
            <MoreHorizontal size={15} />
          </button>
          {#if tabMenuOpen}
            <div class="editor-tab-menu__popover" role="menu" aria-label="Open tabs" data-testid="tab-menu">
              <div class="editor-tab-menu__label">OPEN TABS</div>
              {#each tabs as tab (tab.id)}
                <div class="editor-tab-menu__item" data-tab-id={tab.id}>
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
                      class="editor-tab-menu__open"
                      role="menuitem"
                      aria-label={`Open ${tab.name}`}
                      title={`Open ${tab.name}`}
                      data-testid={`tab-menu-open-${tab.id}`}
                      on:click={() => activateTabFromMenu(tab.id)}
                      on:dblclick|stopPropagation={() => beginTabRename(tab)}
                    >{showTabDirty && tab.dirty ? `${tab.name} •` : tab.name}</button>
                  {/if}
                  <button
                    class="editor-tab__close"
                    aria-label={`Close ${tab.name}`}
                    title="Close tab"
                    data-testid={`tab-close-${tab.id}`}
                    on:click={() => closeTabFromMenu(tab.id)}
                  ><X size={11} /></button>
                </div>
              {/each}
            </div>
          {/if}
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
  .editor-topbar {
    position: relative;
    z-index: 20;
    display: flex;
    min-width: 0;
    height: var(--topbar-height);
    align-items: center;
    gap: 8px;
    padding: 0 10px;
    border-bottom: 1px solid var(--border-strong);
    background: var(--topbar-bg);
    box-shadow: 0 1px 0 rgb(29 39 53 / 2%);
  }

  .editor-topbar--sidebar {
    height: auto;
    min-height: 0;
    flex: 0 0 auto;
    align-items: stretch;
    padding: 0;
    border: 0;
    background: transparent;
    box-shadow: none;
  }

  .editor-topbar--sidebar .editor-topbar__tabs {
    flex-direction: column;
    align-items: stretch;
    gap: 3px;
  }

  .editor-topbar--sidebar .editor-topbar__tab-list {
    width: 100%;
    flex: 0 0 auto;
    flex-direction: column;
    align-items: stretch;
    gap: 3px;
    overflow: visible;
  }

  .editor-topbar--sidebar .editor-tab,
  .editor-topbar--sidebar .editor-tab__new,
  .editor-topbar--sidebar .editor-tab__more {
    width: 100%;
  }

  .editor-topbar--sidebar .editor-tab-menu { width: 100%; }
  .editor-topbar--sidebar .editor-tab-menu__popover { top: 0; left: calc(100% + 7px); right: auto; }
  .editor-topbar--sidebar .editor-tab-menu__popover::before { top: 14px; left: -5px; right: auto; transform: rotate(-45deg); }

  .editor-topbar__command {
    position: relative;
    display: flex;
    flex: 0 0 auto;
    align-items: center;
  }

  .editor-topbar__tabs {
    display: flex;
    min-width: 0;
    margin-left: 0;
    flex: 1 1 auto;
    align-items: center;
    gap: 4px;
    overflow: visible;
  }

  .editor-topbar__tab-list {
    display: flex;
    min-width: 0;
    flex: 1 1 auto;
    align-items: center;
    gap: 4px;
    overflow-x: auto;
    scrollbar-width: none;
  }

  .editor-topbar__tab-list::-webkit-scrollbar { display: none; }

  .editor-tab {
    display: inline-flex;
    min-width: 0;
    height: 32px;
    flex: 0 0 auto;
    align-items: center;
    gap: 4px;
    border: 1px solid transparent;
    border-radius: 6px;
    padding: 0 6px 0 9px;
    color: var(--text-muted);
    transition: background-color 150ms ease, border-color 150ms ease, color 150ms ease;
  }

  .editor-tab:hover { background: var(--panel-bg-alt); color: var(--text-primary); }
  .editor-tab--active { border-color: color-mix(in srgb, var(--accent) 48%, var(--border-strong)); background: var(--accent-soft); color: var(--text-primary); box-shadow: 0 1px 2px rgb(29 39 53 / 5%); }
  .editor-tab--renaming { border-color: var(--accent); box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 16%, transparent); }

  .editor-tab__open, .editor-tab__close, .editor-tab__new {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 0;
    background: transparent;
    color: inherit;
  }

  .editor-tab__open { display: block; min-width: 0; max-width: 150px; flex: 0 1 auto; overflow: hidden; padding: 0; text-overflow: ellipsis; white-space: nowrap; font-size: 11px; }
  .editor-tab__close { width: 19px; height: 19px; border-radius: 4px; }
  .editor-tab__close:hover { background: color-mix(in srgb, var(--accent) 11%, transparent); color: var(--accent); }
  .editor-tab__new { width: 32px; height: 32px; flex: 0 0 auto; order: 2; border: 1px solid var(--border-muted); border-radius: 6px; color: var(--text-muted); }
  .editor-tab__new:hover { background: var(--panel-bg-alt); color: var(--text-primary); }
  .editor-tab-menu { position: relative; z-index: 1; order: 3; flex: 0 0 auto; }
  .editor-tab__more { display: inline-flex; width: 32px; height: 32px; flex: 0 0 auto; align-items: center; justify-content: center; border: 1px solid var(--border-muted); border-radius: 6px; color: var(--text-muted); background: transparent; }
  .editor-tab__more:hover, .editor-tab__more[aria-expanded='true'] { border-color: color-mix(in srgb, var(--accent) 40%, var(--border-muted)); color: var(--text-primary); background: var(--panel-bg-alt); }
  .editor-tab-menu__popover { position: absolute; z-index: 40; top: calc(100% + 7px); right: 0; width: 238px; padding: 6px; border: 1px solid var(--border-strong); border-radius: 8px; background: var(--topbar-bg); box-shadow: 0 12px 30px rgb(29 39 53 / 15%); }
  .editor-tab-menu__popover::before { position: absolute; top: -5px; right: 14px; width: 8px; height: 8px; border-top: 1px solid var(--border-strong); border-left: 1px solid var(--border-strong); background: var(--topbar-bg); content: ''; transform: rotate(45deg); }
  .editor-tab-menu__label { padding: 5px 8px 6px; color: var(--text-muted); font-size: 9px; font-weight: 700; letter-spacing: .08em; }
  .editor-tab-menu__item { display: flex; min-width: 0; align-items: center; gap: 4px; border-radius: 5px; padding: 2px 3px 2px 8px; color: var(--text-primary); }
  .editor-tab-menu__item:hover { background: var(--panel-bg-alt); }
  .editor-tab-menu__open { display: block; min-width: 0; flex: 1; overflow: hidden; border: 0; padding: 5px 0; color: inherit; background: transparent; text-align: left; text-overflow: ellipsis; white-space: nowrap; font-size: 11px; }
  .editor-tab-menu__item .editor-tab__close { flex: 0 0 auto; }
  .editor-tab__rename-wrap { position: relative; min-width: 0; }
  .editor-tab-menu__item .editor-tab__rename-wrap { flex: 1; }
  .editor-tab__measure { visibility: hidden; white-space: pre; font-size: 11px; }
  .editor-tab__rename { position: absolute; inset: 0; width: 100%; min-width: 70px; border: 0; outline: 0; background: transparent; color: inherit; font-size: 11px; }

  @media (max-width: 620px) {
    .editor-topbar__tabs { gap: 3px; }
    .editor-tab__open { max-width: 112px; }
  }
</style>
