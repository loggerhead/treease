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
  <div class="flex min-w-0 items-center gap-2">
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
  <div class="flex h-full min-w-0 flex-1 items-center gap-3">
    <div class="ml-auto flex h-full min-w-0 items-center gap-2.5">
      {#if hasTreePath}
        <div class="inline-flex h-full min-w-0 items-center text-[12px] leading-4 text-[var(--text-primary)]">
          <TreePathBreadcrumb
            value={$activeTempModel?.treePath ?? []}
            on:select={(event) => onTreePathSelect(event.detail)}
          />
        </div>
      {/if}
      <div class="inline-flex items-center gap-2">
        <span class="rounded-[6px] bg-[var(--panel-bg)] px-2 py-[1px] leading-[16px]">
          {$activeTempModel?.cursor ?? 'Ln 1, Col 1'}
          {#if ($activeTempModel?.selectionLength ?? 0) > 0}
            <span> ({$activeTempModel?.selectionLength} selected)</span>
          {/if}
        </span>
      </div>
    </div>
  </div>
</footer>
