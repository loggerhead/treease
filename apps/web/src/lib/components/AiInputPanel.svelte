<script lang="ts">
  import { onMount } from 'svelte'
  import { ArrowUp, LoaderCircle, Sparkles, X } from 'lucide-svelte'
  import { IconButton } from './ui/button'

  export let value = ''
  export let busy = false
  export let error = ''
  export let success = ''
  export let quotaExhausted = false
  export let upgradeBusy = false
  export let onChange: (value: string) => void = () => {}
  export let onSubmit: (value: string) => void | Promise<void> = () => {}
  export let onUpgrade: () => void | Promise<void> = () => {}
  export let onClose: () => void = () => {}

  let input: HTMLTextAreaElement | null = null
  const maxInputHeight = 160

  function resizeInput() {
    if (!input) return
    input.style.height = 'auto'
    const nextHeight = Math.min(input.scrollHeight, maxInputHeight)
    input.style.height = `${nextHeight}px`
    input.style.overflowY = input.scrollHeight > maxInputHeight ? 'auto' : 'hidden'
  }

  function handleSubmit() {
    const instruction = value.trim()
    if (!instruction || busy) return
    onSubmit(instruction)
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key !== 'Enter' || event.shiftKey) return
    event.preventDefault()
    handleSubmit()
  }

  function handleInput(event: Event) {
    onChange((event.currentTarget as HTMLTextAreaElement).value)
    resizeInput()
  }

  onMount(() => {
    resizeInput()
    input?.focus()
  })
</script>

<div class="editor-input-panel editor-input-panel--ai px-2 py-1.5" data-testid="ai-input-panel">
  {#if error}
    <div class="mb-1 flex items-center justify-between gap-2" role="alert" aria-live="assertive" aria-atomic="true">
      <p class="text-[12px] text-[var(--danger-text,#dc2626)]" data-testid="ai-input-error">{error}</p>
      {#if quotaExhausted}
        <button
          type="button"
          class="shrink-0 rounded-[6px] bg-[var(--accent)] px-2 py-1 text-[11px] font-semibold text-white transition-colors hover:brightness-95 disabled:cursor-wait disabled:opacity-60"
          data-testid="ai-quota-upgrade-button"
          disabled={upgradeBusy}
          on:click={onUpgrade}
        >{upgradeBusy ? 'Opening checkout…' : 'Upgrade for more AI'}</button>
      {/if}
    </div>
  {/if}
  {#if busy}
    <p class="mb-1 flex items-center gap-1.5 pl-1 text-[11px] text-[var(--text-muted)]" role="status" aria-live="polite">
      <LoaderCircle size={11} strokeWidth={1.9} class="animate-spin text-[#4779c9]" />
      Processing your request…
    </p>
  {:else if success}
    <p class="mb-1 flex min-w-0 items-center gap-1.5 pl-1 text-[11px] text-[var(--text-muted)]" role="status" aria-live="polite">
      <Sparkles size={11} strokeWidth={1.9} class="shrink-0 text-[var(--accent)]" />
      <span class="shrink-0">Executed</span>
      <code class="truncate rounded bg-[var(--accent-soft)] px-1 py-px font-mono text-[var(--accent)]">{success}</code>
    </p>
  {/if}
  <form class="flex items-end gap-1.5" on:submit|preventDefault={handleSubmit}>
    <div class="flex min-h-[30px] min-w-0 flex-1 items-end gap-2 overflow-hidden rounded-[var(--control-radius)] border border-[var(--border-muted)] bg-[var(--panel-bg)] px-2.5 shadow-[0_1px_2px_rgb(24_59_86_/_5%)] focus-within:border-[var(--accent)] focus-within:shadow-[var(--focus-ring)]">
      <Sparkles size={13} strokeWidth={1.9} class="mb-1.5 shrink-0 text-[var(--accent)]" />
      <textarea
        bind:this={input}
        value={value}
        rows="1"
        class="min-h-[28px] min-w-0 flex-1 resize-none bg-transparent py-1 text-[13px] leading-5 text-[var(--text-primary)] outline-none placeholder:text-[#8a9ab2]"
        placeholder="Ask AI to transform the current document…"
        aria-label="AI instruction"
        disabled={busy}
        on:input={handleInput}
        on:keydown={handleKeydown}
      ></textarea>
    </div>
    <button
      type="submit"
      aria-label={busy ? 'Processing' : 'Send'}
      title={busy ? 'Processing…' : 'Send'}
      class="inline-flex h-[30px] w-[30px] shrink-0 items-center justify-center rounded-full bg-[var(--accent)] text-white shadow-[0_1px_2px_rgb(24_59_86_/_20%)] transition-[background-color,transform] hover:brightness-95 active:scale-95 disabled:cursor-not-allowed disabled:opacity-45"
      disabled={busy || !value.trim()}
    >
      {#if busy}<LoaderCircle size={14} strokeWidth={2} class="animate-spin" />{:else}<ArrowUp size={15} strokeWidth={2.2} />{/if}
    </button>
    <IconButton aria-label="Close AI input" title="Close" class="self-center" on:click={onClose} disabled={busy}><X size={12} /></IconButton>
  </form>
</div>

<style>
</style>
