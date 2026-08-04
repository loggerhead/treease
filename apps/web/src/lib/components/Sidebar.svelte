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

  let expanded = true
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
    expanded = !expanded
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

<div class="editor-sidebar-host" data-expanded={expanded}>
  <aside
    class:editor-sidebar--expanded={expanded}
    class="editor-sidebar"
  >
    <nav class="editor-sidebar__nav" aria-label="Editor tools">
      <div class="editor-sidebar__main">
        <Tooltip content="Treease home" side="right" disabled={expanded}>
          <a class="editor-sidebar__logo" href="/" aria-label="Treease home">
            <img src={assetUrl(r2Assets.treeaseLogo)} alt="Treease" />
            <span class="editor-sidebar__label">Treease</span>
          </a>
        </Tooltip>

        <ContextItem bind:this={importItem} bind:open={importOpen} label="Import" ariaLabel="Import" testId="topbar-import-button" {expanded}>
          <FileUp size={16} slot="icon" />
          <div slot="panel">
            <div class="editor-sidebar__popover-title">Import</div>
            <div class="editor-sidebar__popover-row"><span>File type</span>
              <Select.Root type="single" items={formatOptions.map((option) => ({ value: option.id, label: option.label }))} bind:value={importFormat}>
                <Select.Trigger size="sm" class="editor-sidebar__select"><span data-slot="select-value">{importLabel}</span></Select.Trigger>
                <Select.Content class="min-w-[150px]">{#each formatOptions as option}<Select.Item value={option.id} label={option.label} class="text-[12px]" />{/each}</Select.Content>
              </Select.Root>
            </div>
            <button
              class:editor-sidebar__drop--active={importDropActive}
              class="editor-sidebar__drop"
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
          <div slot="panel">
            <div class="editor-sidebar__popover-title">Export</div>
            <div class="editor-sidebar__popover-row"><span>Export to</span>
              <Select.Root type="single" items={formatOptions.map((option) => ({ value: option.id, label: option.label }))} bind:value={exportFormat}>
                <Select.Trigger size="sm" class="editor-sidebar__select"><span data-slot="select-value">{exportLabel}</span></Select.Trigger>
                <Select.Content class="min-w-[150px]">{#each formatOptions as option}<Select.Item value={option.id} label={option.label} class="text-[12px]" />{/each}</Select.Content>
              </Select.Root>
            </div>
            <div class="editor-sidebar__popover-actions">
              <button aria-label="Download export file" on:click={handleExportDownload}>Download</button>
              {#if currentLanguage !== exportFormat}<button aria-label="Preview export result" on:click={handleExportPreview}>Preview</button>{/if}
            </div>
          </div>
        </ContextItem>

        <div class="editor-sidebar__rule"></div>
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

      <div class="editor-sidebar__footer">
        <ContextItem bind:open={feedbackOpen} label="Feedback" ariaLabel="Feedback" testId="feedback-trigger" placement="right-end" panelClass="editor-sidebar__popover--feedback" {expanded}>
          <MessageCircle size={16} slot="icon" />
          <div slot="panel">
            <FeedbackDialog bind:open={feedbackOpen} />
          </div>
        </ContextItem>
        <ContextItem bind:open={shareOpen} label="Share" ariaLabel="Share" testId="share-trigger" placement="right-end" panelClass="editor-sidebar__popover--share" {expanded}>
          <Share2 size={16} slot="icon" />
          <div slot="panel">
            <ShareDialog bind:open={shareOpen} createResource={createShareResource} />
          </div>
        </ContextItem>
        <ContextItem bind:open={settingsOpen} label="Settings" ariaLabel="Settings" testId="settings-trigger" placement="right-end" panelClass="editor-sidebar__popover--settings" {expanded}>
          <Settings size={16} slot="icon" />
          <div slot="panel">
            <SettingsDialog bind:open={settingsOpen} />
          </div>
        </ContextItem>
        <Tooltip content="Account" side="right" disabled={expanded}>
          <div class="editor-sidebar__account">
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
  .editor-sidebar-host { position: relative; z-index: 50; width: var(--sidebar-rail-width); height: 100%; flex: 0 0 var(--sidebar-rail-width); transition: width 200ms ease, flex-basis 200ms ease; }
  .editor-sidebar-host[data-expanded='true'] { width: 184px; flex-basis: 184px; }
  .editor-sidebar { position: absolute; inset: 0 auto 0 0; width: var(--sidebar-rail-width); pointer-events: none; transition: width 200ms ease; }
  .editor-sidebar--expanded { width: 184px; }
  .editor-sidebar__nav { pointer-events: auto; display: flex; height: 100%; width: 100%; flex-direction: column; justify-content: space-between; gap: 8px; overflow: visible; border-right: 1px solid var(--border-strong); background: var(--panel-bg); box-shadow: 4px 0 16px rgb(29 39 53 / 6%); transition: width 180ms ease; }
  .editor-sidebar__main, .editor-sidebar__footer { display: flex; flex-direction: column; align-items: center; gap: 3px; padding: 6px 4px; }
  .editor-sidebar-host[data-expanded='true'] .editor-sidebar__main,
  .editor-sidebar-host[data-expanded='true'] .editor-sidebar__footer { align-items: stretch; padding-inline: 12px; }
  .editor-sidebar__footer { gap: 5px; padding-bottom: 8px; }
  .editor-sidebar__logo { display: flex; width: 36px; height: 36px; box-sizing: border-box; align-items: center; justify-content: center; gap: 0; margin: 0 0 5px; overflow: hidden; border-radius: 6px; padding: 0; color: var(--text-primary); text-decoration: none; }
  .editor-sidebar__logo img { width: 30px; height: 30px; flex: 0 0 30px; object-fit: contain; transform: translateX(2px); }
  .editor-sidebar--expanded .editor-sidebar__logo { width: 100%; height: 30px; justify-content: flex-start; padding: 0 4px; }
  .editor-sidebar--expanded .editor-sidebar__logo img { width: 25px; height: 25px; flex-basis: 25px; transform: none; }
  .editor-sidebar__logo .editor-sidebar__label { overflow: hidden; opacity: 0; text-overflow: ellipsis; transition: opacity 120ms ease; }
  .editor-sidebar--expanded .editor-sidebar__logo .editor-sidebar__label { opacity: 1; }
  .editor-sidebar__rule { height: 1px; margin: 5px 3px; background: var(--border-muted); }
  .editor-sidebar__account { display: flex; width: 100%; height: 36px; align-items: center; justify-content: center; border-radius: 6px; }
  .editor-sidebar__account:hover { background: var(--panel-bg-alt); }
  .editor-sidebar--expanded .editor-sidebar__account { width: 100%; height: 58px; justify-content: flex-start; }
  .editor-sidebar--expanded .editor-sidebar__account :global(.editor-account-anchor--profile),
  .editor-sidebar--expanded .editor-sidebar__account :global(.account-profile-trigger) { width: 100%; height: 58px; }
  .editor-sidebar__popover-title { margin-bottom: 10px; color: var(--text-primary); font-size: 15px; font-weight: 650; }
  .editor-sidebar__popover-row { display: flex; align-items: center; justify-content: space-between; gap: 8px; color: var(--text-muted); font-size: 11px; }
  .editor-sidebar__select { height: 25px; min-width: 120px; border: 1px solid var(--border-muted); background: var(--panel-bg); font-size: 11px; }
  .editor-sidebar__drop { display: flex; width: 100%; height: 100px; align-items: center; justify-content: center; margin-top: 12px; border: 1px dashed var(--border-muted); border-radius: 8px; color: var(--text-muted); background: var(--panel-bg-alt); font-size: 11px; }
  .editor-sidebar__drop:hover, .editor-sidebar__drop--active { border-color: var(--accent); color: var(--text-primary); background: var(--accent-soft); }
  .editor-sidebar__popover-actions { display: flex; gap: 7px; margin-top: 12px; }
  .editor-sidebar__popover-actions button { height: 27px; border: 1px solid var(--border-muted); border-radius: 6px; padding: 0 9px; color: var(--text-primary); background: var(--panel-bg); font-size: 11px; }
  .editor-sidebar__popover-actions button:hover { background: var(--panel-bg-alt); }
  :global(.editor-sidebar__popover--feedback) { width: min(520px, calc(100vw - 52px)); max-height: calc(100vh - 200px); overflow-y: auto; padding: 18px; }
  :global(.editor-sidebar__popover--share) { width: min(384px, calc(100vw - 52px)); padding: 18px; }
  :global(.editor-sidebar__popover--settings) { width: min(640px, calc(100vw - 52px)); max-height: calc(100vh - 132px); overflow-y: auto; padding: 18px; }
  @media (max-width: 760px) { .editor-sidebar-host { width: 44px; flex-basis: 44px; } .editor-sidebar-host[data-expanded='true'] { width: 158px; flex-basis: 158px; } .editor-sidebar--expanded { width: 158px; } }
</style>
