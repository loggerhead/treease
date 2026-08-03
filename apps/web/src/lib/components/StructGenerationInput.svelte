<script lang="ts">
  import { LoaderCircle, X } from 'lucide-svelte';
  import { Button, IconButton } from './ui/button';
  import type { StructLanguage } from '../services/treease-server';

  export let targetLanguage: StructLanguage = 'typescript';
  export let rootName = 'Root';
  export let busy = false;
  export let error = '';
  export let onChangeTargetLanguage: (value: StructLanguage) => void = () => {};
  export let onChangeRootName: (value: string) => void = () => {};
  export let onSubmit: () => void | Promise<void> = () => {};
  export let onClose: () => void = () => {};

  const languageOptions: Array<{ id: StructLanguage; label: string }> = [
    { id: 'typescript', label: 'TypeScript' },
    { id: 'go', label: 'Go' },
    { id: 'rust', label: 'Rust' },
    { id: 'python', label: 'Python' },
    { id: 'java', label: 'Java' },
    { id: 'kotlin', label: 'Kotlin' },
    { id: 'csharp', label: 'C#' },
    { id: 'swift', label: 'Swift' },
    { id: 'dart', label: 'Dart' },
    { id: 'ruby', label: 'Ruby' },
    { id: 'php', label: 'PHP' },
  ];
</script>

<div class="border-t border-[var(--border-strong)] bg-[var(--panel-bg-alt)] px-2 py-1.5" data-testid="struct-generation-panel">
  {#if error}
    <p class="mb-1 text-[12px] text-[var(--danger-text,#dc2626)]" role="alert" aria-live="assertive">{error}</p>
  {/if}
  <div class="flex items-center gap-1.5">
    <label class="flex shrink-0 items-center gap-1 text-[12px] text-[var(--text-muted)]">
      <span>Target</span>
      <select
        value={targetLanguage}
        aria-label="Target language"
        class="h-[30px] rounded-[8px] border border-[var(--border-muted)] bg-[var(--panel-bg)] px-2 text-[12px] text-[var(--text-primary)] outline-none focus:border-[var(--accent)]"
        on:change={(event) => onChangeTargetLanguage((event.currentTarget as HTMLSelectElement).value as StructLanguage)}
      >
        {#each languageOptions as option}
          <option value={option.id}>{option.label}</option>
        {/each}
      </select>
    </label>
    <label class="flex min-w-0 flex-1 items-center gap-1 text-[12px] text-[var(--text-muted)]">
      <span class="shrink-0">Root</span>
      <input
        value={rootName}
        aria-label="Root name"
        class="h-[30px] min-w-0 flex-1 rounded-[8px] border border-[var(--border-muted)] bg-[var(--panel-bg)] px-2 text-[12px] text-[var(--text-primary)] outline-none focus:border-[var(--accent)]"
        on:input={(event) => onChangeRootName((event.currentTarget as HTMLInputElement).value)}
      />
    </label>
    <Button size="xs" on:click={() => void onSubmit()} disabled={busy}>
      {#if busy}<LoaderCircle size={13} class="mr-1 animate-spin" />{/if}
      {busy ? 'Generating' : 'Generate'}
    </Button>
    <IconButton aria-label="Close structure generation" title="Close" on:click={onClose} disabled={busy}><X size={12} /></IconButton>
  </div>
</div>
