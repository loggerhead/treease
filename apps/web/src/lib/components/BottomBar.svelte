<script lang="ts">
  import { Check, ChevronLeft, ChevronRight, Copy, Sparkles, Wand2, Shrink } from 'lucide-svelte';
  import { languageId as languageIdStore } from '../store/document-session-store';
  import { activeTempModel } from '../store/diagnostics-store';
  import { buildReadablePath, isPathSegIndex, pathSegKeyValue, type PathSeg } from '../store/tree-path';
  import { supportedEditorLanguages } from '../monaco/language-support';
  import type { CommandId } from '../command-registry';
  import * as Select from './ui/select';
  import * as ButtonGroup from './ui/button-group';
  import { IconButton } from './ui/button';
  import CommandSearchInput from './CommandSearchInput.svelte';
  import { settings, settingsStore } from '../settings/settings-store';
  import {
    buildWorkspacePathPrefixes,
    workspacePathKey,
  } from './graph-viewer/column-navigator/index';
  import type { ColumnNavigatorState } from './graph-viewer/column-navigator/types';
  import { trackEvent } from '../analytics/ga4';
  export let onFormat: () => void | Promise<void> = () => {};
  export let onMinify: () => void | Promise<void> = () => {};
  export let onCompact: () => void | Promise<void> = () => {};
  export let onSort: () => void | Promise<void> = () => {};
  export let onShowYqInput: () => void | Promise<void> = () => {};
  export let onShowAiInput: () => void | Promise<void> = () => {};
  export let onGenerateStruct: () => void | Promise<void> = () => {};
  export let onEscape: () => void | Promise<void> = () => {};
  export let onUnescape: () => void | Promise<void> = () => {};
  export let onNewDocument: () => void | Promise<void> = () => {};
  export let onOpenDocument: () => void | Promise<void> = () => {};
  export let onSaveDocument: () => void | Promise<void> = () => {};
  export let onSaveAsDocument: () => void | Promise<void> = () => {};
  export let onCloseDocument: () => void | Promise<void> = () => {};
  export let onTreePathSelect: (path: PathSeg[]) => void = () => {};
  /** Width of the editor pane; the path area starts at the graph pane boundary. */
  export let editorWidthPx = 0;
  export let graphVisible = true;
  export let columnNavigatorState: ColumnNavigatorState | null = null;
  export let onColumnNavigatorBack: () => void | Promise<void> = () => {};
  export let onColumnNavigatorForward: () => void | Promise<void> = () => {};
  export let onColumnNavigatorPathSelect: (path: PathSeg[]) => void | Promise<void> = () => {};

  const languageItems = supportedEditorLanguages.map((option) => ({ value: option.id, label: option.label }));
  const commandHandlers: Record<CommandId, () => void | Promise<void>> = {
    'workspace:new': () => onNewDocument(),
    'workspace:open': () => onOpenDocument(),
    'workspace:save': () => onSaveDocument(),
    'workspace:save-as': () => onSaveAsDocument(),
    'workspace:close-tab': () => onCloseDocument(),
    format: () => onFormat(),
    minify: () => onMinify(),
    compact: () => onCompact(),
    sort: () => onSort(),
    'show-yq-input': () => onShowYqInput(),
    'generate-struct': () => onGenerateStruct(),
    escape: () => onEscape(),
    unescape: () => onUnescape(),
    'toggle-nest': async () => {
      await settingsStore.save({ parser: { enableNest: !$settings.parser.enableNest } })
    },
    'toggle-auto-format': async () => {
      const currentFormatting = $settings.formatting
      await settingsStore.save({ formatting: { ...currentFormatting, smart: !currentFormatting.smart } })
    },
  };
  let commandQuery = '';
  let copiedColumnPath = false;
  let copyFeedbackFading = false;
  let columnPathCopyTimer: ReturnType<typeof setTimeout> | null = null;
  let columnPathFadeTimer: ReturnType<typeof setTimeout> | null = null;
  let previousLanguage = '';
  $: if (!previousLanguage) previousLanguage = $languageIdStore;
  $: if (previousLanguage && previousLanguage !== $languageIdStore) {
    trackEvent('language_selected', { from: previousLanguage, to: $languageIdStore });
    previousLanguage = $languageIdStore;
  }
  $: hasTreePath = ($activeTempModel?.treePath ?? []).length > 0;
  $: displayedPath = columnNavigatorState?.open
    ? columnNavigatorState.activePath
    : ($activeTempModel?.treePath ?? []);
  $: showDisplayedPathbar = graphVisible && (Boolean(columnNavigatorState?.open) || hasTreePath);
  $: if ($activeTempModel && $activeTempModel.commandQuery !== commandQuery)
    commandQuery = $activeTempModel.commandQuery;

  function columnPathLabel(path: PathSeg[]): string {
    if (!path.length) return '$';
    const segment = path[path.length - 1];
    return isPathSegIndex(segment) ? `[${segment.index}]` : pathSegKeyValue(segment);
  }

  function readableDisplayedPath(path: PathSeg[]): string {
    return buildReadablePath(path);
  }

  function selectDisplayedPath(path: PathSeg[]): void | Promise<void> {
    return columnNavigatorState?.open
      ? onColumnNavigatorPathSelect(path)
      : onTreePathSelect(path);
  }

  async function copyColumnNavigatorPath(): Promise<void> {
    if (!navigator.clipboard) return;
    await navigator.clipboard.writeText(readableDisplayedPath(displayedPath));
    copiedColumnPath = true;
    copyFeedbackFading = false;
    if (columnPathCopyTimer) clearTimeout(columnPathCopyTimer);
    if (columnPathFadeTimer) clearTimeout(columnPathFadeTimer);
    columnPathCopyTimer = setTimeout(() => {
      copyFeedbackFading = true;
      columnPathCopyTimer = null;
      columnPathFadeTimer = setTimeout(() => {
        copiedColumnPath = false;
        copyFeedbackFading = false;
        columnPathFadeTimer = null;
      }, 180);
    }, 1000);
  }

  /**
   * Sync the command query string into state.
   * @param value Input command string
   * @returns void
   */
  function updateCommandQuery(value: string) {
    commandQuery = value;
    activeTempModel.update((current) => ({ ...current, commandQuery: value }));
  }

  /**
   * Get the display name for a language dropdown.
   * @param value Language ID
   * @param options Language options
   * @returns Display name, or the ID when no name is available
   */
  function getLanguageLabel(value: string, options: ReadonlyArray<{ id: string; label: string }>) {
    return options.find((option) => option.id === value)?.label ?? value;
  }

</script>

<footer
  class="grid h-[var(--bottombar-height)] min-w-0 border-t border-[var(--border-strong)] bg-[var(--bottombar-bg)] text-[12px] text-[var(--text-muted)]"
  style:grid-template-columns={`${editorWidthPx}px minmax(0, 1fr)`}
>
  <div class="flex min-w-0 items-center gap-2 overflow-hidden px-4">
    <div class="inline-flex items-center gap-1">
      <Select.Root type="single" items={languageItems} bind:value={$languageIdStore}>
        <Select.Trigger
          size="sm"
          class="!h-[23px] rounded-[7px] border border-[rgba(15,23,42,0.10)] bg-[var(--panel-bg)] !px-2 !py-0 !text-[12px] !font-medium tracking-[-0.01em] text-[#111827] shadow-none transition-colors hover:border-[rgba(15,23,42,0.16)] focus-visible:ring-0 [&_svg]:size-[13px] [&_svg]:text-[#94a3b8]"
          aria-label="Language"
        >
          <span data-slot="select-value">{getLanguageLabel($languageIdStore, supportedEditorLanguages)}</span>
        </Select.Trigger>
        <Select.Content
          sideOffset={6}
          class="min-w-[140px] rounded-[10px] border-[rgba(15,23,42,0.10)] shadow-[0_12px_28px_rgba(15,23,42,0.10)] bg-[var(--panel-bg)]"
        >
          {#each supportedEditorLanguages as option}
            <Select.Item value={option.id} label={option.label} class="text-[12px]" />
          {/each}
        </Select.Content>
      </Select.Root>
    </div>
    <CommandSearchInput
      value={commandQuery}
      on:input={(event) => updateCommandQuery(event.detail)}
      onExecute={(id) => commandHandlers[id]()}
    />
    <ButtonGroup.Root
      variant="segmented-outline"
      class="rounded-[7px] border-[rgba(15,23,42,0.10)] bg-[var(--panel-bg)] [&>[data-slot=button]:not(:first-child)]:border-[rgba(15,23,42,0.10)]"
    >
      <IconButton
        aria-label="AI"
        title="Ask AI"
        class="h-[23px] min-w-[37px] gap-1 rounded-[6px] bg-[#f5f8ff] px-1.5 text-[#4779c9] hover:bg-[#edf3ff] hover:text-[#2f63b1]"
        on:click={onShowAiInput}
      >
        <Sparkles size={11.5} strokeWidth={1.9} />
        <span class="text-[10px] font-semibold tracking-[0.01em]">AI</span>
      </IconButton>
      <IconButton
        aria-label="Format"
        title="Format"
        class="h-[23px] w-[23px] rounded-[6px] text-[#475569] hover:bg-[#f8fafc] hover:text-[#111827]"
        on:click={onFormat}
      >
        <Wand2 size={11.5} strokeWidth={1.9} />
      </IconButton>
      <IconButton
        aria-label="Minify"
        title="Minify"
        class="h-[23px] w-[23px] rounded-[6px] text-[#475569] hover:bg-[#f8fafc] hover:text-[#111827]"
        on:click={onMinify}
      >
        <Shrink size={11.5} strokeWidth={1.9} />
      </IconButton>
    </ButtonGroup.Root>
  </div>
  <div class="flex h-full min-w-0 items-center gap-3 border-l border-[var(--border-strong)] px-4">
    {#if showDisplayedPathbar}
      <div class="bottom-column-navigator-pathbar group flex min-w-0 items-center gap-1.5" data-testid="bottom-column-navigator-pathbar">
        <div class="inline-flex shrink-0 items-center gap-0.5">
          <button
            type="button"
            class="inline-flex h-[24px] w-[24px] items-center justify-center rounded-[6px] text-[#64748b] hover:bg-[#e2e8f0] hover:text-[#1e293b] disabled:cursor-default disabled:text-[#cbd5e1] disabled:hover:bg-transparent"
            aria-label="Back in workspace history"
            title="Back in workspace history"
            data-testid="bottom-column-navigator-back"
            disabled={!columnNavigatorState?.open || !columnNavigatorState.canGoBack}
            on:click={() => void onColumnNavigatorBack()}
          ><ChevronLeft size={15} strokeWidth={2} /></button>
          <button
            type="button"
            class="inline-flex h-[24px] w-[24px] items-center justify-center rounded-[6px] text-[#64748b] hover:bg-[#e2e8f0] hover:text-[#1e293b] disabled:cursor-default disabled:text-[#cbd5e1] disabled:hover:bg-transparent"
            aria-label="Forward in workspace history"
            title="Forward in workspace history"
            data-testid="bottom-column-navigator-forward"
            disabled={!columnNavigatorState?.open || !columnNavigatorState.canGoForward}
            on:click={() => void onColumnNavigatorForward()}
          ><ChevronRight size={15} strokeWidth={2} /></button>
        </div>
        <div class="flex min-w-0 items-center overflow-x-auto whitespace-nowrap [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
          {#each buildWorkspacePathPrefixes(displayedPath) as prefix (workspacePathKey(prefix))}
            <button
              type="button"
              class="max-w-[180px] shrink-0 truncate rounded-[6px] border-0 bg-transparent px-1.5 py-1 font-mono text-[11px] leading-[1.3] text-[#64748b] hover:bg-[#dbeafe]/70 hover:text-[#1e3a5f]"
              class:active={workspacePathKey(prefix) === workspacePathKey(displayedPath)}
              title={readableDisplayedPath(prefix)}
              aria-current={workspacePathKey(prefix) === workspacePathKey(displayedPath) ? 'location' : undefined}
              on:click={() => void selectDisplayedPath(prefix)}
            >{columnPathLabel(prefix)}</button>
            {#if workspacePathKey(prefix) !== workspacePathKey(displayedPath)}
              <ChevronRight class="shrink-0 text-[#a8b3c2]" size={12} strokeWidth={1.8} aria-hidden="true" />
            {/if}
          {/each}
        </div>
        <button
          type="button"
          class="bottom-column-navigator-copy ml-0.5 inline-flex h-[22px] w-[22px] shrink-0 items-center justify-center rounded-[5px] text-[var(--text-muted)] opacity-0 hover:bg-[#e2e8f0] hover:text-[var(--text-primary)] group-hover:opacity-100 focus-visible:opacity-100"
          class:copied={copiedColumnPath}
          class:fading={copyFeedbackFading}
          title={copiedColumnPath ? 'Copied' : 'Copy tree path'}
          aria-label={copiedColumnPath ? 'Tree path copied' : 'Copy tree path'}
          data-testid="bottom-column-navigator-copy"
          on:click={() => void copyColumnNavigatorPath()}
        >
          {#if copiedColumnPath}
            <Check size={12} />
          {:else}
            <Copy size={12} />
          {/if}
        </button>
      </div>
    {/if}
    <div class="ml-auto inline-flex shrink-0 items-center gap-2">
      <span class="rounded-[6px] bg-[var(--panel-bg)] px-2 py-[1px] leading-[16px]">
        {$activeTempModel?.cursor ?? 'Ln 1, Col 1'}
        {#if ($activeTempModel?.selectionLength ?? 0) > 0}
          <span> ({$activeTempModel?.selectionLength} selected)</span>
        {/if}
      </span>
    </div>
  </div>
</footer>

<style>
  .bottom-column-navigator-pathbar button.active {
    color: #1e3a5f;
    background: rgba(219, 234, 254, 0.72);
  }

  .bottom-column-navigator-copy.copied {
    opacity: 1;
    color: #15803d;
    background: #dcfce7;
  }

  .bottom-column-navigator-copy.fading {
    opacity: 0 !important;
  }

  .bottom-column-navigator-copy {
    transition: opacity 180ms ease-out, background-color 180ms ease-out, color 180ms ease-out;
  }
</style>
