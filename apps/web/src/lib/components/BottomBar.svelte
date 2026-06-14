<script lang="ts">
  import { Wand2, Shrink } from 'lucide-svelte';
  import { languageId as languageIdStore, activeTempModel } from '../store/editor-store';
  import type { PathSeg } from '../store/tree-path';
  import { supportedEditorLanguages } from '../monaco/language-support';
  import type { CommandId } from '../command-registry';
  import * as Select from './ui/select';
  import * as ButtonGroup from './ui/button-group';
  import { IconButton } from './ui/button';
  import CommandSearchInput from './CommandSearchInput.svelte';
  import { settings, settingsStore } from '../settings/settings-store';
  import TreePathBreadcrumb from './TreePathBreadcrumb.svelte';
  export let onFormat: () => void | Promise<void> = () => {};
  export let onMinify: () => void | Promise<void> = () => {};
  export let onSort: () => void | Promise<void> = () => {};
  export let onShowYqInput: () => void | Promise<void> = () => {};
  export let onEscape: () => void | Promise<void> = () => {};
  export let onUnescape: () => void | Promise<void> = () => {};
  export let onTreePathSelect: (path: PathSeg[]) => void = () => {};

  const languageItems = supportedEditorLanguages.map((option) => ({ value: option.id, label: option.label }));
  const commandHandlers: Record<CommandId, () => void | Promise<void>> = {
    format: () => onFormat(),
    minify: () => onMinify(),
    sort: () => onSort(),
    'show-yq-input': () => onShowYqInput(),
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
  $: hasTreePath = ($activeTempModel?.treePath ?? []).length > 0;
  $: if ($activeTempModel && $activeTempModel.commandQuery !== commandQuery)
    commandQuery = $activeTempModel.commandQuery;

  /**
   * 同步命令查询字符串到状态
   * @param value 输入的命令字符串
   * @returns void
   */
  function updateCommandQuery(value: string) {
    commandQuery = value;
    activeTempModel.update((current) => ({ ...current, commandQuery: value }));
  }

  /**
   * 获取语言下拉显示名称
   * @param value 语言 id
   * @param options 语言选项列表
   * @returns 对应的显示名称，找不到时返回 id
   */
  function getLanguageLabel(value: string, options: ReadonlyArray<{ id: string; label: string }>) {
    return options.find((option) => option.id === value)?.label ?? value;
  }

</script>

<footer
  class="flex h-[var(--bottombar-height)] items-center justify-between gap-4 border-t border-[var(--border-strong)] bg-[var(--bottombar-bg)] px-4 text-[12px] text-[var(--text-muted)]"
>
  <div class="flex min-w-0 items-center gap-3">
    <div class="inline-flex items-center gap-1.5">
      <Select.Root type="single" items={languageItems} bind:value={$languageIdStore}>
        <Select.Trigger
          size="sm"
          class="h-6 rounded-[10px] border border-[var(--border-muted)] bg-[var(--panel-bg)] px-3 py-0.5 text-[12px] text-[var(--text-primary)] shadow-none focus-visible:ring-0"
          aria-label="Language"
        >
          <span data-slot="select-value">{getLanguageLabel($languageIdStore, supportedEditorLanguages)}</span>
        </Select.Trigger>
        <Select.Content
          sideOffset={6}
          class="min-w-[140px] rounded-[10px] border-[var(--border-muted)] shadow-[0_8px_24px_rgba(15,23,42,0.08)] bg-[var(--panel-bg)]"
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
    <ButtonGroup.Root variant="segmented-outline">
      <IconButton aria-label="Format" title="Format" on:click={onFormat}>
        <Wand2 size={12} />
      </IconButton>
      <IconButton aria-label="Minify" title="Minify" on:click={onMinify}>
        <Shrink size={12} />
      </IconButton>
    </ButtonGroup.Root>
  </div>
  <div class="flex min-w-0 flex-1 items-center gap-3 h-full">
    <div class="ml-auto flex min-w-0 items-center gap-3 h-full">
      {#if hasTreePath}
        <div class="inline-flex min-w-0 items-center text-[12px] leading-4 text-[var(--text-primary)] h-full">
          <TreePathBreadcrumb
            value={$activeTempModel?.treePath ?? []}
            on:select={(event) => onTreePathSelect(event.detail)}
          />
        </div>
      {/if}
      <div class="inline-flex items-center gap-3">
        <span class="rounded-[6px] bg-[var(--panel-bg)] px-2 py-[2px]">
          {$activeTempModel?.cursor ?? 'Ln 1, Col 1'}
          {#if ($activeTempModel?.selectionLength ?? 0) > 0}
            <span> ({$activeTempModel?.selectionLength} selected)</span>
          {/if}
        </span>
      </div>
    </div>
  </div>
</footer>
