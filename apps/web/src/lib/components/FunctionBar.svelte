<script lang="ts">
  import { Sparkles, Shrink, Wand2 } from 'lucide-svelte';
  import type { CommandId } from '../command-registry';
  import { languageId as languageIdStore } from '../store/document-session-store';
  import { supportedEditorLanguages } from '../monaco/language-support';
  import * as Select from './ui/select';
  import CommandSearchInput from './CommandSearchInput.svelte';
  import Tooltip from './Tooltip.svelte';
  import { trackEvent } from '../analytics/ga4';

  export let onShowAiInput: () => void | Promise<void> = () => {};
  export let onFormat: () => void | Promise<void> = () => {};
  export let onMinify: () => void | Promise<void> = () => {};
  export let onCommandExecute: (id: CommandId) => void | Promise<void> = () => {};
  export let aiInputOpen = false;

  const languageItems = supportedEditorLanguages.map((option) => ({ value: option.id, label: option.label }));

  function selectLanguage(value: string): void {
    if (value === $languageIdStore) return;
    trackEvent('language_selected', { from: $languageIdStore, to: value });
    languageIdStore.set(value as typeof $languageIdStore);
  }
</script>

<div class="editor-functionbar" data-testid="editor-functionbar" aria-label="Editor functions">
  <div class="editor-functionbar__navigation">
    <Select.Root type="single" items={languageItems} value={$languageIdStore} onValueChange={selectLanguage}>
      <Select.Trigger size="sm" class="editor-functionbar__language" aria-label="Language">
        <span data-slot="select-value">{supportedEditorLanguages.find((option) => option.id === $languageIdStore)?.label ?? $languageIdStore}</span>
      </Select.Trigger>
      <Select.Content side="bottom" align="start" sideOffset={0} class="min-w-[140px] rounded-[10px] border-[var(--border-strong)] bg-[var(--panel-bg)] shadow-[0_12px_28px_rgba(29,39,53,0.10)] data-[side=bottom]:translate-y-0">
        {#each supportedEditorLanguages as option}<Select.Item value={option.id} label={option.label} class="text-[12px]" />{/each}
      </Select.Content>
    </Select.Root>
    <div class="editor-functionbar__command"><CommandSearchInput compact compactLabel="Command" onExecute={onCommandExecute} /></div>
  </div>
  <div class="editor-functionbar__processing">
    <Tooltip content="Format" side="bottom"><button class="editor-functionbar__button" aria-label="Format" on:click={() => void onFormat()}><Wand2 size={13} /></button></Tooltip>
    <Tooltip content="Minify" side="bottom"><button class="editor-functionbar__button" aria-label="Minify" on:click={() => void onMinify()}><Shrink size={13} /></button></Tooltip>
    <button class:editor-functionbar__ai--active={aiInputOpen} class="editor-functionbar__ai" aria-label="Ask AI" aria-expanded={aiInputOpen} title="Ask AI" on:click={() => void onShowAiInput()}><Sparkles size={13} /><span>AI</span></button>
  </div>
</div>

<style>
  .editor-functionbar {
    position: relative;
    z-index: 100;
    display: flex;
    min-width: 0;
    height: var(--topbar-height);
    flex: 0 0 var(--topbar-height);
    align-items: center;
    justify-content: space-between;
    gap: 4px;
    padding: 0 10px;
    border-bottom: 1px solid var(--border-strong);
    background: var(--panel-bg);
    color: var(--text-muted);
  }

  .editor-functionbar__navigation,
  .editor-functionbar__processing { display: inline-flex; min-width: 0; align-items: center; gap: 3px; }
  .editor-functionbar__navigation { justify-content: flex-start; }
  .editor-functionbar__processing { justify-content: flex-end; }
  :global(.editor-functionbar__language) {
    display: inline-flex;
    width: 72px;
    height: 28px !important;
    flex: 0 0 auto;
    align-items: center;
    border: 1px solid var(--border-muted) !important;
    border-radius: 6px !important;
    padding: 0 8px !important;
    color: var(--text-primary) !important;
    background: var(--panel-bg-alt) !important;
    font-size: 11px !important;
    font-weight: 500;
    box-shadow: none !important;
  }
  :global(.editor-functionbar__language [data-slot='select-value']) { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .editor-functionbar__button, .editor-functionbar__ai { display: inline-flex; height: 28px; align-items: center; justify-content: center; gap: 4px; border: 0; border-radius: 6px; padding: 0 5px; color: var(--text-muted); background: transparent; font-size: 11px; }
  .editor-functionbar__button:hover { color: var(--text-primary); background: var(--panel-bg-alt); }
  .editor-functionbar__command { display: flex; align-items: center; }
  .editor-functionbar__command :global(button) { width: auto; min-width: 28px; height: 28px; gap: 4px; padding: 0 5px; font-size: 11px; }
  .editor-functionbar__ai { color: #7b5424; background: #f8efdf; }
  .editor-functionbar__ai:hover, .editor-functionbar__ai--active { background: #f3e4c9; }

  @media (max-width: 620px) {
    .editor-functionbar__ai span { display: none; }
    .editor-functionbar__command :global(button) { padding: 0 6px; }
  }
</style>
