<script lang="ts">
  import { ArrowLeftToLine, ArrowRightFromLine, Braces, Download, FileUp, Link2, MessageCircle, Settings, Share2, SquareStack } from 'lucide-svelte'
  import { languageId as languageIdStore } from '../store/document-session-store'
  import { settings, settingsStore } from '../settings/settings-store'
  import { assetUrl, r2Assets } from '$lib/assets'
  import * as Select from './ui/select'
  import AccountMenu from './AccountMenu.svelte'
  import ContextItem from './ContextItem.svelte'
  import FeedbackDialog from './FeedbackDialog.svelte'
  import ShareDialog from './ShareDialog.svelte'
  import SettingsDialog from './SettingsDialog.svelte'
  import type { ShareResource } from '../share/share-resource'
  import Item from './Item.svelte'
  import ToggleItem from './ToggleItem.svelte'
  import Tooltip from './Tooltip.svelte'

  type FormatOption = { id: string; label: string; extensions: string[] }

  export let formatOptions: FormatOption[] = []
  export let onRequestImportFile: (payload: { sourceFormat: string; targetFormat: string; accept: string[] }) => Promise<void> = async () => {}
  export let onImportFileStream: (payload: { file: File; sourceFormat: string; targetFormat: string; fileName: string }) => void = () => {}
  export let onExportPreview: (format: string) => void = () => {}
  export let onExportDownload: (format: string) => void = () => {}
  export let feedbackOpen = false
  export let shareOpen = false
  export let settingsOpen = false
  export let createShareResource: (() => Promise<ShareResource>) | null = null
  export let onLogin: () => void = () => {}
  export let onLogout: () => Promise<void> = async () => {}
  export let onCheckForUpdates: () => Promise<void> = async () => {}

  export let expanded = true
  export let onToggleSidebar: (expanded: boolean) => void = () => {}
  let importOpen = false
  let exportOpen = false
  let importDropActive = false
  let importFormat = 'json'
  let exportFormat = 'json'
  let importFileInput: HTMLInputElement | null = null
  let importItem: ContextItem | null = null
  let exportItem: ContextItem | null = null

  $: currentLanguage = $languageIdStore
  $: importLabel = formatOptions.find((item) => item.id === importFormat)?.label ?? importFormat
  $: exportLabel = formatOptions.find((item) => item.id === exportFormat)?.label ?? exportFormat

  export function openImportPanel(): void {
    importItem?.openPanel()
    exportItem?.closePanel()
  }

  export function openExportPanel(): void {
    exportItem?.openPanel()
    importItem?.closePanel()
  }

  function toggleSidebar(): void {
    onToggleSidebar(!expanded)
  }

  function handleImportFile(file: File | null | undefined): void {
    if (!file) return
    importDropActive = false
    onImportFileStream({ file, sourceFormat: importFormat, targetFormat: currentLanguage, fileName: file.name })
    importOpen = false
  }

  async function requestImportFile(): Promise<void> {
    const accept = formatOptions.find((item) => item.id === importFormat)?.extensions ?? []
    await onRequestImportFile({ sourceFormat: importFormat, targetFormat: currentLanguage, accept })
    importOpen = false
  }

  function handleExportPreview(): void {
    onExportPreview(exportFormat)
    exportOpen = false
  }

  function handleExportDownload(): void {
    onExportDownload(exportFormat)
    exportOpen = false
  }

  function toggleSetting(setting: 'formatting' | 'parser' | 'interaction'): void {
    if (setting === 'formatting') {
      void settingsStore.save({ formatting: { ...$settings.formatting, smart: !$settings.formatting.smart } })
    } else if (setting === 'parser') {
      void settingsStore.save({ parser: { enableNest: !$settings.parser.enableNest } })
    } else {
      void settingsStore.save({ interaction: { ...$settings.interaction, enableSyncScroll: !$settings.interaction.enableSyncScroll } })
    }
  }

</script>

<div class="sidebar-host" data-expanded={expanded}>
  <aside
    class:sidebar--expanded={expanded}
    class="sidebar"
  >
    <nav class="sidebar__nav" aria-label="Editor tools">
      <div class="sidebar__main">
        <Tooltip content="Treease home" side="right" disabled={expanded}>
          <a class="sidebar__logo" href="/" aria-label="Treease home">
            <img src={assetUrl(r2Assets.treeaseLogo)} alt="" />
            <span class="sidebar__label">Treease</span>
          </a>
        </Tooltip>

        <ContextItem bind:this={importItem} bind:open={importOpen} label="Import" ariaLabel="Import" testId="topbar-import-button" {expanded}>
          <FileUp size={16} slot="icon" />
          <div slot="panel" data-testid="import-panel">
            <div class="sidebar__popover-title">Import</div>
            <div class="sidebar__popover-row"><span>File type</span>
              <Select.Root type="single" items={formatOptions.map((option) => ({ value: option.id, label: option.label }))} bind:value={importFormat}>
                <Select.Trigger size="sm" class="sidebar__select"><span data-slot="select-value">{importLabel}</span></Select.Trigger>
                <Select.Content class="min-w-[150px]">{#each formatOptions as option}<Select.Item value={option.id} label={option.label} class="text-[12px]" />{/each}</Select.Content>
              </Select.Root>
            </div>
            <button
              class:sidebar__drop--active={importDropActive}
              class="sidebar__drop"
              aria-label="Choose import file"
              data-testid="import-drop-trigger"
              on:click={() => void requestImportFile()}
              on:dragenter|preventDefault={() => (importDropActive = true)}
              on:dragover|preventDefault={() => (importDropActive = true)}
              on:dragleave|preventDefault={() => (importDropActive = false)}
              on:drop|preventDefault={(event) => handleImportFile(event.dataTransfer?.files?.[0])}
            >Click to choose or drop a file</button>
            <input class="sr-only" type="file" bind:this={importFileInput} on:change={(event) => handleImportFile((event.currentTarget as HTMLInputElement).files?.[0])} />
          </div>
        </ContextItem>

        <ContextItem bind:this={exportItem} bind:open={exportOpen} label="Export" ariaLabel="Export" testId="topbar-export-button" {expanded}>
          <Download size={16} slot="icon" />
          <div slot="panel" data-testid="export-panel">
            <div class="sidebar__popover-title">Export</div>
            <div class="sidebar__popover-row"><span>Export to</span>
              <Select.Root type="single" items={formatOptions.map((option) => ({ value: option.id, label: option.label }))} bind:value={exportFormat}>
                <Select.Trigger size="sm" class="sidebar__select"><span data-slot="select-value">{exportLabel}</span></Select.Trigger>
                <Select.Content class="min-w-[150px]">{#each formatOptions as option}<Select.Item value={option.id} label={option.label} class="text-[12px]" />{/each}</Select.Content>
              </Select.Root>
            </div>
            <div class="sidebar__popover-actions">
              <button aria-label="Download export file" on:click={handleExportDownload}>Download</button>
              {#if currentLanguage !== exportFormat}<button aria-label="Preview export result" on:click={handleExportPreview}>Preview</button>{/if}
            </div>
          </div>
        </ContextItem>

        <div class="sidebar__rule"></div>
        <ToggleItem label="Auto format" ariaLabel="Auto format" tooltip="Automatically formats the document only when all content is replaced, such as pasting over the entire document or importing a file." {expanded} pressed={$settings.formatting.smart} onClick={() => toggleSetting('formatting')}>
          <Braces size={16} slot="icon" />
        </ToggleItem>
        <ToggleItem label="Parse nested JSON" ariaLabel="Parse nested JSON" tooltip="Parses nested JSON strings only when all content is replaced, such as pasting over the entire document or importing a file." {expanded} pressed={$settings.parser.enableNest} onClick={() => toggleSetting('parser')}>
          <SquareStack size={16} slot="icon" />
        </ToggleItem>
        <ToggleItem label="Navigation sync" ariaLabel="Navigation sync" tooltip="In Compare mode, links left and right editor scrolling. In Graph mode, links editor, graph, and navigation reveals." {expanded} pressed={$settings.interaction.enableSyncScroll} onClick={() => toggleSetting('interaction')}>
          <Link2 size={16} slot="icon" />
        </ToggleItem>
      </div>

      <div class="sidebar__footer">
        <ContextItem bind:open={feedbackOpen} label="Feedback" ariaLabel="Feedback" testId="feedback-trigger" placement="right-end" panelClass="sidebar__popover--feedback" {expanded}>
          <MessageCircle size={16} slot="icon" />
          <div slot="panel">
            <FeedbackDialog bind:open={feedbackOpen} />
          </div>
        </ContextItem>
        <ContextItem bind:open={shareOpen} label="Share" ariaLabel="Share" testId="share-trigger" placement="right-end" panelClass="sidebar__popover--share" {expanded}>
          <Share2 size={16} slot="icon" />
          <div slot="panel">
            <ShareDialog bind:open={shareOpen} createResource={createShareResource} />
          </div>
        </ContextItem>
        <ContextItem bind:open={settingsOpen} label="Settings" ariaLabel="Settings" testId="settings-trigger" placement="right-end" panelClass="sidebar__popover--settings" {expanded}>
          <Settings size={16} slot="icon" />
          <div slot="panel">
            <SettingsDialog bind:open={settingsOpen} />
          </div>
        </ContextItem>
        <Tooltip content="Account" side="right" disabled={expanded}>
          <div class="sidebar__account">
            <AccountMenu variant="editor" showTriggerTitle={false} showProfileTrigger={expanded} contextPanel {onLogin} {onLogout} {onCheckForUpdates} onOpenSettings={() => (settingsOpen = true)} />
          </div>
        </Tooltip>
        <Item label={expanded ? 'Collapse' : 'Expand'} ariaLabel={expanded ? 'Collapse sidebar' : 'Expand sidebar'} tooltip={expanded ? 'Collapse sidebar' : 'Expand sidebar'} {expanded} testId="sidebar-collapse-toggle" onClick={toggleSidebar}>
          {#if expanded}<ArrowLeftToLine size={16} slot="icon" />{:else}<ArrowRightFromLine size={16} slot="icon" />{/if}
        </Item>
      </div>
    </nav>
  </aside>
</div>

<style>
  .sidebar-host { position: relative; z-index: 50; width: var(--sidebar-rail-width); height: 100%; flex: 0 0 var(--sidebar-rail-width); transition: width 180ms cubic-bezier(.2,.8,.2,1), flex-basis 180ms cubic-bezier(.2,.8,.2,1); }
  .sidebar-host[data-expanded='true'] { width: 168px; flex-basis: 168px; }
  .sidebar { position: absolute; inset: 0 auto 0 0; width: var(--sidebar-rail-width); pointer-events: none; transition: width 180ms cubic-bezier(.2,.8,.2,1); }
  .sidebar--expanded { width: 168px; }
  .sidebar__nav { pointer-events: auto; display: flex; height: 100%; width: 100%; flex-direction: column; justify-content: space-between; gap: var(--space-3); overflow: visible; border-right: 1px solid var(--border-strong); background: var(--panel-bg); box-shadow: 3px 0 12px rgb(24 59 86 / 4%); transition: width 180ms cubic-bezier(.2,.8,.2,1); }
  .sidebar__main, .sidebar__footer { display: flex; width: 100%; box-sizing: border-box; flex-direction: column; align-items: stretch; gap: var(--space-2); padding: var(--space-2); }
  .sidebar__footer { gap: var(--space-2); padding-bottom: var(--space-3); }
  .sidebar__logo { display: flex; width: 32px; height: 32px; box-sizing: border-box; align-items: center; justify-content: flex-start; gap: var(--space-3); margin: var(--space-1) 0 var(--space-3); overflow: hidden; border-radius: var(--control-radius); padding: 0; color: var(--text-primary); text-decoration: none; }
  /* The brand mark matches the compact account avatar in both sidebar states. */
  .sidebar__logo img { width: 32px; height: 32px; flex: 0 0 32px; object-fit: contain; transform: none; }
  .sidebar--expanded .sidebar__logo { width: 100%; }
  .sidebar__logo .sidebar__label { overflow: hidden; opacity: 0; font-size: var(--font-size-title); font-weight: 600; letter-spacing: -.02em; line-height: 1; text-overflow: ellipsis; transition: opacity 120ms ease; }
  .sidebar--expanded .sidebar__logo .sidebar__label { opacity: 1; }
  .sidebar__rule { width: 100%; height: 1px; flex: 0 0 1px; margin: var(--space-2) 0; background: var(--border-muted); }
  .sidebar__account { display: flex; width: 32px; height: 46px; align-items: center; justify-content: center; margin-left: 0; border-radius: var(--control-radius); }
  .sidebar__account:hover { background: var(--panel-bg-alt); }
  .sidebar--expanded .sidebar__account { width: 100%; height: 46px; margin-left: 0; justify-content: flex-start; }
  .sidebar--expanded .sidebar__account :global(.editor-account-anchor--profile),
  .sidebar--expanded .sidebar__account :global(.account-profile-trigger) { width: 100%; height: 46px; }
  .sidebar__popover-title { margin-bottom: var(--space-3); color: var(--text-primary); font-size: 14px; font-weight: 650; letter-spacing: -.01em; }
  .sidebar__popover-row { display: flex; align-items: center; justify-content: space-between; gap: var(--space-4); color: var(--text-muted); font-size: var(--font-size-control); }
  .sidebar__drop { display: flex; width: 100%; height: 88px; align-items: center; justify-content: center; margin-top: var(--space-3); border: 1px dashed var(--border-muted); border-radius: var(--control-radius); color: var(--text-muted); background: var(--panel-bg-alt); font-size: var(--font-size-control); transition: var(--control-transition); }
  .sidebar__drop:hover, .sidebar__drop--active { border-color: var(--accent); color: var(--text-primary); background: var(--accent-soft); }
  .sidebar-host:not([data-expanded='true']) :global(.sidebar__item-wrap),
  .sidebar-host:not([data-expanded='true']) :global(.sidebar__item) { width: 32px; flex-basis: 32px; }
  .sidebar__popover-actions { display: flex; gap: var(--space-2); margin-top: var(--space-3); }
  .sidebar__popover-actions button { height: 28px; border: 1px solid var(--border-muted); border-radius: var(--control-radius); padding: 0 var(--space-3); color: var(--text-primary); background: var(--panel-bg); font-size: var(--font-size-control); transition: var(--control-transition); }
  .sidebar__popover-actions button:hover { border-color: var(--accent); background: var(--panel-bg-alt); }
  .sidebar__popover-actions button:focus-visible, .sidebar__drop:focus-visible { outline: none; box-shadow: var(--focus-ring); }
  :global(.sidebar__popover--feedback) { width: min(500px, calc(100vw - 52px)); max-height: calc(100vh - 160px); overflow-y: auto; padding: var(--space-6); }
  :global(.sidebar__popover--share) { width: min(368px, calc(100vw - 52px)); padding: var(--space-6); }
  :global(.sidebar__popover--settings) { width: min(620px, calc(100vw - 52px)); max-height: calc(100vh - 120px); overflow-y: auto; padding: var(--space-6); }
  @media (max-width: 760px) { .sidebar-host { width: 44px; flex-basis: 44px; } .sidebar-host[data-expanded='true'] { width: 158px; flex-basis: 158px; } .sidebar--expanded { width: 158px; } }
</style>
