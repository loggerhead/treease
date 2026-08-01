<script lang="ts">
  import { Check, ChevronRight, Copy, Sparkles, Wand2, Shrink } from 'lucide-svelte';
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
  let copiedTreePath = false;
  let copyFeedbackFading = false;
  let treePathCopyTimer: ReturnType<typeof setTimeout> | null = null;
  let treePathFadeTimer: ReturnType<typeof setTimeout> | null = null;
  $: hasTreePath = ($activeTempModel?.treePath ?? []).length > 0;
  $: treePath = $activeTempModel?.treePath ?? [];
  $: showTreePathbar = graphVisible && hasTreePath;
  $: if ($activeTempModel && $activeTempModel.commandQuery !== commandQuery)
    commandQuery = $activeTempModel.commandQuery;

  function treePathLabel(path: PathSeg[]): string {
    if (!path.length) return '$';
    const segment = path[path.length - 1];
    return isPathSegIndex(segment) ? `[${segment.index}]` : pathSegKeyValue(segment);
  }

  function readableTreePath(path: PathSeg[]): string {
    return buildReadablePath(path);
  }

  function buildTreePathPrefixes(path: PathSeg[]): PathSeg[][] {
    return Array.from({ length: path.length + 1 }, (_, index) => path.slice(0, index));
  }

  async function copyTreePath(): Promise<void> {
    if (!navigator.clipboard) return;
    await navigator.clipboard.writeText(readableTreePath(treePath));
    copiedTreePath = true;
    copyFeedbackFading = false;
    if (treePathCopyTimer) clearTimeout(treePathCopyTimer);
    if (treePathFadeTimer) clearTimeout(treePathFadeTimer);
    treePathCopyTimer = setTimeout(() => {
      copyFeedbackFading = true;
      treePathCopyTimer = null;
      treePathFadeTimer = setTimeout(() => {
        copiedTreePath = false;
        copyFeedbackFading = false;
        treePathFadeTimer = null;
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

  function selectLanguage(value: string): void {
    if (value === $languageIdStore) return;
    trackEvent('language_selected', { from: $languageIdStore, to: value });
    languageIdStore.set(value as typeof $languageIdStore);
  }

</script>

<footer
  class="grid h-[var(--bottombar-height)] min-w-0 border-t border-[var(--border-strong)] bg-[var(--bottombar-bg)] text-[12px] text-[var(--text-muted)]"
  style:grid-template-columns={`${editorWidthPx}px minmax(0, 1fr)`}
>
  <div class="flex min-w-0 items-center gap-2 overflow-visible px-4">
    <div class="inline-flex items-center gap-1">
      <Select.Root type="single" items={languageItems} value={$languageIdStore} onValueChange={selectLanguage}>
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
    {#if showTreePathbar}
      <div class="bottom-tree-pathbar group flex min-w-0 items-center gap-1.5" data-testid="bottom-tree-pathbar">
        <div class="flex min-w-0 items-center overflow-x-auto whitespace-nowrap [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
          {#each buildTreePathPrefixes(treePath) as prefix, index (index)}
            <button
              type="button"
              class="max-w-[180px] shrink-0 truncate rounded-[6px] border-0 bg-transparent px-1.5 py-1 font-mono text-[11px] leading-[1.3] text-[#64748b] hover:bg-[#dbeafe]/70 hover:text-[#1e3a5f]"
              class:active={index === treePath.length}
              title={readableTreePath(prefix)}
              aria-current={index === treePath.length ? 'location' : undefined}
              data-testid={`tree-path-crumb-${index}`}
              on:click={() => onTreePathSelect(prefix)}
            >{treePathLabel(prefix)}</button>
            {#if index < treePath.length}
              <ChevronRight class="shrink-0 text-[#a8b3c2]" size={12} strokeWidth={1.8} aria-hidden="true" />
            {/if}
          {/each}
        </div>
        <button
          type="button"
          class="bottom-tree-path-copy ml-0.5 inline-flex h-[22px] w-[22px] shrink-0 items-center justify-center rounded-[5px] text-[var(--text-muted)] opacity-0 hover:bg-[#e2e8f0] hover:text-[var(--text-primary)] group-hover:opacity-100 focus-visible:opacity-100"
          class:copied={copiedTreePath}
          class:fading={copyFeedbackFading}
          title={copiedTreePath ? 'Copied' : 'Copy tree path'}
          aria-label={copiedTreePath ? 'Tree path copied' : 'Copy tree path'}
          data-testid="bottom-tree-path-copy"
          on:click={() => void copyTreePath()}
        >
          {#if copiedTreePath}
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
  .bottom-tree-pathbar button.active {
    color: #1e3a5f;
    background: rgba(219, 234, 254, 0.72);
  }

  .bottom-tree-path-copy.copied {
    opacity: 1;
    color: #15803d;
    background: #dcfce7;
  }

  .bottom-tree-path-copy.fading {
    opacity: 0 !important;
  }

  .bottom-tree-path-copy {
    transition: opacity 180ms ease-out, background-color 180ms ease-out, color 180ms ease-out;
  }
</style>
