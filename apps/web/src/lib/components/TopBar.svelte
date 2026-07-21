<script lang="ts">
  import { onMount } from 'svelte'
  import { cubicOut } from 'svelte/easing'
  import { fly } from 'svelte/transition'
  import { Plus, X, FileInput, FileOutput, BookOpen, MessageCircle, Share2, ArrowRight } from 'lucide-svelte'
  import { trackEvent } from '../analytics/ga4'
  import { languageId as languageIdStore } from '../store/document-session-store'
  import AccountMenu from './AccountMenu.svelte'
  import * as Select from './ui/select'
  import * as ButtonGroup from './ui/button-group'
  import { Button, IconButton } from './ui/button'
  import type { SupportedEditorLanguageId } from '../monaco/language-support'
  import { assetUrl, r2Assets } from '$lib/assets'

  export let tabs: Array<{ id: string; name: string; languageId: SupportedEditorLanguageId; dirty?: boolean }> = []
  export let activeTabId = ''
  export let showTabDirty = false
  export let canAddTab = true
  export let showTabs = true
  export let showRightActions = true
  export let onAddTab: () => void = () => {}
  export let onCloseTab: (id: string) => void = () => {}
  export let onActivateTab: (id: string) => void = () => {}
  export let formatOptions: Array<{ id: string; label: string; extensions: string[] }> = []
  export let onRequestImportFile: (payload: { sourceFormat: string; targetFormat: string; accept: string[] }) => Promise<void> = async () => {}
  export let onImportFileStream: (payload: { file: File; sourceFormat: string; targetFormat: string; fileName: string }) => void = () => {}
  export let onExportPreview: (format: string) => void = () => {}
  export let onExportDownload: (format: string) => void = () => {}
  export let onShare: () => void = () => {}
  export let onFeedback: () => void = () => {}
  export let onOpenSettings: () => void = () => {}
  export let onLogin: () => void = () => {}
  export let onLogout: () => Promise<void> = async () => {}
  export let onCheckForUpdates: () => Promise<void> = async () => {}

  let importOpen = false
  let exportOpen = false
  let importFormat = 'json'
  let exportFormat = 'json'
  let importDropActive = false
  let importAnchor: HTMLDivElement | null = null
  let exportAnchor: HTMLDivElement | null = null

  const toggleImportPanel = () => {
    importOpen = !importOpen
    exportOpen = false
  }

  const toggleExportPanel = () => {
    exportOpen = !exportOpen
    importOpen = false
  }

  export function openImportPanel(): void {
    importOpen = true
    exportOpen = false
  }

  export function openExportPanel(): void {
    exportOpen = true
    importOpen = false
  }

  const handleImportFile = async (file: File | null | undefined) => {
    if (!file) return
    importDropActive = false
    onImportFileStream({ file, sourceFormat: importFormat, targetFormat: $languageIdStore, fileName: file.name })
    importOpen = false
  }

  const requestImportFile = async () => {
    const accept = formatOptions.find((item) => item.id === importFormat)?.extensions ?? []
    await onRequestImportFile({ sourceFormat: importFormat, targetFormat: $languageIdStore, accept })
    importOpen = false
  }

  $: importConversion = {
    srcLabel: getFormatLabel(importFormat, formatOptions),
    dstLabel: getFormatLabel($languageIdStore, formatOptions),
  }
  $: exportConversion = {
    srcLabel: getFormatLabel($languageIdStore, formatOptions),
    dstLabel: getFormatLabel(exportFormat, formatOptions),
  }

  /**
   * Get the display text for a format selector.
   * @param value Format ID
   * @param options Format options
   * @returns Display name, or the ID when no name is available
   */
  const getFormatLabel = (value: string, options: Array<{ id: string; label: string }>) => {
    return options.find((option) => option.id === value)?.label ?? value
  }

  onMount(() => {
    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target as HTMLElement | null
      const insideSelectOverlay = !!target?.closest('[data-slot="select-content"]')

      if (insideSelectOverlay) return

      if (importOpen && importAnchor && target && !importAnchor.contains(target)) {
        importOpen = false
      }
      if (exportOpen && exportAnchor && target && !exportAnchor.contains(target)) {
        exportOpen = false
      }
    }

    document.addEventListener('pointerdown', handlePointerDown)

    return () => {
      document.removeEventListener('pointerdown', handlePointerDown)
    }
  })
</script>

<header class="relative grid h-[var(--topbar-height)] grid-cols-[auto_auto_1fr_auto] items-center gap-3 border-b border-[var(--border-strong)] bg-[var(--topbar-bg)] px-3 text-[var(--text-primary)]">
  <a
    class="inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-[6px] outline-none transition-[background-color,box-shadow] duration-150 hover:bg-[var(--panel-bg-alt)] focus-visible:ring-2 focus-visible:ring-[var(--accent)]/25"
    aria-label="Treease home"
    title="Treease home"
    data-testid="topbar-home-link"
    href="/"
  >
    <img class="h-6 w-6 object-contain" src={assetUrl(r2Assets.treeaseLogo)} alt="Treease logo" />
  </a>
  <ButtonGroup.Root variant="segmented-outline" class="min-w-0">
    <IconButton aria-label="Import" title="Import" data-testid="topbar-import-button" on:click={toggleImportPanel}>
      <FileInput size={12} />
    </IconButton>
    <IconButton aria-label="Export" title="Export" data-testid="topbar-export-button" on:click={toggleExportPanel}>
      <FileOutput size={12} />
    </IconButton>
  </ButtonGroup.Root>
  <div class="min-w-0">
    {#if showTabs}
      <ButtonGroup.Root class="items-center gap-1.5 overflow-x-auto" data-testid="editor-tab-strip">
        {#each tabs as tab (tab.id)}
          <ButtonGroup.Root
            class={`inline-flex items-center gap-1 rounded-[6px] border border-transparent bg-transparent px-1.5 py-0.5 text-[var(--text-muted)] transition-[background-color,border-color,color,box-shadow] duration-150 ease-out ${
              tab.id === activeTabId ? 'border-[var(--border-strong)] bg-[var(--panel-bg)] text-[var(--text-primary)] shadow-[0_1px_2px_rgba(15,23,42,0.04)]' : 'hover:bg-[var(--panel-bg-alt)] hover:text-[var(--text-primary)]'
            }`}
            data-testid="editor-tab"
            data-tab-id={tab.id}
            data-active={tab.id === activeTabId}
          >
            <button
              class="text-[11px]"
              aria-label={`Open ${tab.name}`}
              title={`Open ${tab.name}`}
              data-testid={`tab-open-${tab.id}`}
              on:click={() => onActivateTab(tab.id)}
            >
              {showTabDirty && tab.dirty ? `${tab.name} •` : tab.name}
            </button>
            <button
              class="inline-flex items-center justify-center p-0.5"
              aria-label={`Close ${tab.name}`}
              title="Close tab"
              data-testid={`tab-close-${tab.id}`}
              on:click={() => onCloseTab(tab.id)}
            >
              <X size={10} />
            </button>
          </ButtonGroup.Root>
        {/each}
        {#if canAddTab}
          <Button
            variant="outline"
            size="xs"
            iconOnly={true}
            class="rounded-[6px]"
            aria-label="New tab"
            title="New tab"
            data-testid="new-tab-button"
            on:click={onAddTab}
          >
            <Plus size={12} />
          </Button>
        {/if}
      </ButtonGroup.Root>
    {/if}
  </div>
  <div class="flex items-center justify-end gap-2">
    {#if showRightActions}
      <ButtonGroup.Root variant="segmented-outline">
        <a
          class="inline-flex h-6 w-6 shrink-0 cursor-pointer items-center justify-center whitespace-nowrap rounded-none border-0 bg-transparent text-[var(--text-primary)] outline-none transition-[color,background-color,border-color,box-shadow] hover:bg-[var(--panel-bg-alt)] hover:text-[var(--accent)] focus-visible:ring-2 focus-visible:ring-[var(--accent)]/25"
          data-slot="button"
          data-variant="ghost"
          data-size="xs"
          data-icon-only="true"
          aria-label="Tutorial"
          title="Tutorial"
          data-testid="topbar-tutorial-link"
          href="/tutorial"
          target="_blank"
          rel="noopener noreferrer"
        >
          <BookOpen size={12} />
        </a>
        <button
          class="inline-flex h-6 w-6 shrink-0 cursor-pointer items-center justify-center whitespace-nowrap rounded-none border-0 bg-transparent text-[var(--text-primary)] outline-none transition-[color,background-color,border-color,box-shadow] hover:bg-[var(--panel-bg-alt)] hover:text-[var(--accent)] focus-visible:ring-2 focus-visible:ring-[var(--accent)]/25"
          data-slot="button"
          data-variant="ghost"
          data-size="xs"
          data-icon-only="true"
          aria-label="Feedback"
          title="Feedback"
          data-testid="topbar-feedback-button"
          type="button"
          on:click={onFeedback}
        >
          <MessageCircle size={12} />
        </button>
        <IconButton
          aria-label="Share"
          title="Share"
          on:click={() => {
            trackEvent('share_started', { surface: 'topbar' });
            onShare();
          }}
        >
          <Share2 size={12} />
        </IconButton>
        <div data-button-group-item>
          <AccountMenu variant="editor" {onLogin} {onLogout} {onCheckForUpdates} {onOpenSettings} />
        </div>
      </ButtonGroup.Root>
    {/if}
  </div>
  {#if importOpen}
        <div
          bind:this={importAnchor}
          class="absolute left-0 top-[var(--topbar-height)] z-30 w-[360px] rounded-b-[16px] border border-[var(--border-muted)] border-t-0 bg-white p-4 shadow-[0_12px_40px_rgba(15,23,42,0.12)]"
          data-testid="import-panel"
          style:transform-origin="top left"
          transition:fly={{ y: -6, duration: 150, opacity: 0.08, easing: cubicOut }}
        >
          <div class="text-[18px] font-semibold text-[var(--text-primary)]">Import</div>
          <div class="mt-3 flex items-center justify-between text-[13px] text-[var(--text-muted)]">
            <div class="flex items-center gap-2">
              <span>File type:</span>
              <div class="relative">
                <Select.Root
                  type="single"
                  items={formatOptions.map((option) => ({ value: option.id, label: option.label }))}
                  bind:value={importFormat}
                >
                  <Select.Trigger
                    size="sm"
                    class="rounded-[8px] border border-[var(--border-muted)] bg-white px-2 py-1 text-[13px] text-[var(--text-primary)] shadow-none focus-visible:border-[var(--accent)] focus-visible:shadow-[0_0_0_2px_rgba(56,189,248,0.25)]"
                  >
                    <span data-slot="select-value">{getFormatLabel(importFormat, formatOptions)}</span>
                  </Select.Trigger>
                  <Select.Content class="min-w-[180px]">
                    {#each formatOptions as option}
                      <Select.Item value={option.id} label={option.label} class="text-[13px]" />
                    {/each}
                  </Select.Content>
                </Select.Root>
              </div>
            </div>
            {#if importFormat !== $languageIdStore}
              <div class="flex items-center gap-1.5 text-[12px] text-[var(--text-muted)]">
                <span>{importConversion.srcLabel}</span>
                <ArrowRight size={10} />
                <span>{importConversion.dstLabel}</span>
              </div>
            {/if}
          </div>
          <button
            class={`mt-4 flex h-[160px] w-full flex-col items-center justify-center gap-2 rounded-[12px] border border-dashed text-[13px] text-[var(--text-muted)] transition-[border-color,background-color,box-shadow,color] duration-150 ease-out ${
              importDropActive
                ? 'border-[var(--accent)] bg-[#eff6ff] text-[var(--text-primary)] shadow-[0_0_0_1px_rgba(37,99,235,0.08)]'
                : 'border-[var(--border-muted)] bg-[var(--panel-bg)] hover:border-[var(--accent)]'
            }`}
            aria-label="Choose import file"
            data-testid="import-drop-trigger"
            on:click={() => void requestImportFile()}
            on:dragenter={(event) => {
              event.preventDefault()
              importDropActive = true
            }}
            on:dragover={(event) => {
              event.preventDefault()
              importDropActive = true
            }}
            on:dragleave={(event) => {
              event.preventDefault()
              const nextTarget = event.relatedTarget as Node | null
              if (!(event.currentTarget as HTMLElement).contains(nextTarget)) {
                importDropActive = false
              }
            }}
            on:drop={(event) => {
              event.preventDefault()
              importDropActive = false
              const file = event.dataTransfer?.files?.[0]
              void handleImportFile(file)
            }}
          >
            <span class="text-[12px]">Click here to select file or drop a file right here</span>
          </button>
        </div>
  {/if}
  {#if exportOpen}
        <div
          bind:this={exportAnchor}
          class="absolute left-0 top-[var(--topbar-height)] z-30 w-[360px] rounded-b-[16px] border border-[var(--border-muted)] border-t-0 bg-white p-4 shadow-[0_12px_40px_rgba(15,23,42,0.12)]"
          data-testid="export-panel"
          style:transform-origin="top left"
          transition:fly={{ y: -6, duration: 150, opacity: 0.08, easing: cubicOut }}
        >
          <div class="text-[18px] font-semibold text-[var(--text-primary)]">Export</div>
          <div class="mt-3 flex items-center justify-between text-[13px] text-[var(--text-muted)]">
            <div class="flex items-center gap-2">
              <span>Export to</span>
              <div class="relative">
                <Select.Root
                  type="single"
                  items={formatOptions.map((option) => ({ value: option.id, label: option.label }))}
                  bind:value={exportFormat}
                >
                  <Select.Trigger
                    size="sm"
                    class="rounded-[8px] border border-[var(--border-muted)] bg-white px-2 py-1 text-[13px] text-[var(--text-primary)] shadow-none focus-visible:border-[var(--accent)] focus-visible:shadow-[0_0_0_2px_rgba(56,189,248,0.25)]"
                    aria-label="Export format"
                    data-testid="export-format-trigger"
                  >
                    <span data-slot="select-value">{getFormatLabel(exportFormat, formatOptions)}</span>
                  </Select.Trigger>
                  <Select.Content class="min-w-[180px]">
                    {#each formatOptions as option}
                      <Select.Item value={option.id} label={option.label} class="text-[13px]" />
                    {/each}
                  </Select.Content>
                </Select.Root>
              </div>
            </div>
            {#if $languageIdStore !== exportFormat}
              <div class="flex items-center gap-1.5 text-[12px] text-[var(--text-muted)]">
                <span>{exportConversion.srcLabel}</span>
                <ArrowRight size={10} />
                <span>{exportConversion.dstLabel}</span>
              </div>
            {/if}
          </div>
          <div class="mt-3 flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              class="rounded-[10px] bg-white"
              aria-label="Download export file"
              on:click={() => {
                onExportDownload(exportFormat)
                exportOpen = false
              }}
            >
              Download
            </Button>
            {#if $languageIdStore !== exportFormat}
              <Button
                variant="outline"
                size="sm"
                class="rounded-[10px]"
                aria-label="Preview export result"
                on:click={() => onExportPreview(exportFormat)}
              >
                Preview
              </Button>
            {/if}
          </div>
        </div>
  {/if}
</header>
