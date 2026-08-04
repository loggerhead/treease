<script lang="ts">
  import { Sparkles, Shrink, Wand2 } from 'lucide-svelte';
  import type { CommandId } from '../command-registry';
  import { languageId as languageIdStore } from '../store/document-session-store';
  import { supportedEditorLanguages } from '../monaco/language-support';
  import * as Select from './ui/select';
  import CommandPalette from './CommandPalette.svelte';
  import Tooltip from './Tooltip.svelte';
  import { trackEvent } from '../analytics/ga4';

  export let onShowAiInputPanel: () => void | Promise<void> = () => {};
  export let onFormat: () => void | Promise<void> = () => {};
  export let onMinify: () => void | Promise<void> = () => {};
  export let onCommandExecute: (id: CommandId) => void | Promise<void> = () => {};
  export let aiInputOpen = false;
  export let emptyDocument = false;

  const languageItems = supportedEditorLanguages.map((option) => ({ value: option.id, label: option.label }));

  function selectLanguage(value: string): void {
    if (value === $languageIdStore) return;
    trackEvent('language_selected', { from: $languageIdStore, to: value });
    languageIdStore.set(value as typeof $languageIdStore);
  }
</script>

<div class="function-bar" data-testid="function-bar" aria-label="Editor functions">
  <div class="function-bar__navigation">
    <Select.Root type="single" items={languageItems} value={$languageIdStore} onValueChange={selectLanguage}>
      <Select.Trigger size="sm" class="function-bar__language" aria-label="Language">
        <span data-slot="select-value">{supportedEditorLanguages.find((option) => option.id === $languageIdStore)?.label ?? $languageIdStore}</span>
      </Select.Trigger>
      <Select.Content side="bottom" align="start" sideOffset={0} class="min-w-[140px] rounded-[10px] border-[var(--border-strong)] bg-[var(--panel-bg)] shadow-[0_12px_28px_rgba(29,39,53,0.10)] data-[side=bottom]:translate-y-0">
        {#each supportedEditorLanguages as option}<Select.Item value={option.id} label={option.label} class="text-[12px]" />{/each}
      </Select.Content>
    </Select.Root>
    <div class="function-bar__command"><CommandPalette compact compactLabel="Command" onExecute={onCommandExecute} /></div>
  </div>
  <div class="function-bar__processing">
    <Tooltip content="Format" side="bottom"><button class="function-bar__button" aria-label="Format" disabled={emptyDocument} on:click={() => void onFormat()}><Wand2 size={13} /></button></Tooltip>
    <Tooltip content="Minify" side="bottom"><button class="function-bar__button" aria-label="Minify" disabled={emptyDocument} on:click={() => void onMinify()}><Shrink size={13} /></button></Tooltip>
    <button class:function-bar__ai--active={aiInputOpen} class="function-bar__ai" aria-label="Ask AI" aria-expanded={aiInputOpen} title="Ask AI" disabled={emptyDocument} on:click={() => void onShowAiInputPanel()}><Sparkles size={13} /><span>AI</span></button>
  </div>
</div>

<style>
  .function-bar {
    position: relative;
    z-index: 100;
    display: flex;
    min-width: 0;
    height: var(--topbar-height);
    flex: 0 0 var(--topbar-height);
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
    padding: 0 var(--space-4);
    border-bottom: 1px solid var(--border-strong);
    background: var(--panel-bg);
    color: var(--text-muted);
  }

  .function-bar__navigation,
  .function-bar__processing { display: inline-flex; min-width: 0; align-items: center; gap: var(--space-1); }
  .function-bar__navigation { justify-content: flex-start; }
  .function-bar__processing { justify-content: flex-end; }
  :global(.function-bar__language) {
    display: inline-flex;
    width: 108px;
    height: var(--control-height) !important;
    flex: 0 0 auto;
    align-items: center;
    border: 1px solid var(--border-muted) !important;
    border-radius: var(--control-radius) !important;
    padding: 0 7px !important;
    color: var(--text-primary) !important;
    background: var(--panel-bg-alt) !important;
    font-size: var(--font-size-control) !important;
    font-weight: 500;
    box-shadow: none !important;
  }
  :global(.function-bar__language [data-slot='select-value']) { display: block; overflow: visible; text-overflow: clip; white-space: nowrap; }
  .function-bar__button, .function-bar__ai { display: inline-flex; height: var(--control-height); align-items: center; justify-content: center; gap: 4px; border: 0; border-radius: var(--control-radius); padding: 0 5px; color: var(--text-muted); background: transparent; font-size: var(--font-size-control); transition: var(--control-transition); }
  .function-bar__button:hover:not(:disabled) { color: var(--text-primary); background: var(--panel-bg-alt); }
  .function-bar__button:disabled, .function-bar__ai:disabled { cursor: not-allowed; opacity: .42; }
  .function-bar__command { display: flex; align-items: center; }
  .function-bar__ai { color: #6d5229; background: #f8f1e4; }
  .function-bar__ai:hover:not(:disabled), .function-bar__ai--active { background: #f2e5cd; }
  .function-bar__button:focus-visible, .function-bar__ai:focus-visible { outline: none; box-shadow: var(--focus-ring); }

  @media (max-width: 620px) {
    .function-bar__ai span { display: none; }
    .function-bar__command :global(button) { padding: 0 6px; }
  }
</style>
