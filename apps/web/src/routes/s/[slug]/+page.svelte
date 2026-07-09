<script lang="ts">
  import { onMount } from 'svelte';
  import { Copy, ExternalLink, FileJson, LoaderCircle } from 'lucide-svelte';
  import { toast } from 'svelte-sonner';
  import { getPublicShare, type PublicShare } from '../../../lib/services/treease-server';
  import { Button } from '../../../lib/components/ui/button';

  let share: PublicShare | null = null;
  let error = '';
  let copied = false;

  onMount(async () => {
    const slug = window.location.pathname.split('/').filter(Boolean).at(-1);
    if (!slug) return;
    try {
      share = await getPublicShare(slug);
    } catch (cause) {
      error = cause instanceof Error ? cause.message : '无法加载分享内容';
    }
  });

  $: sharedText = typeof share?.resourcePayload.text === 'string' ? share.resourcePayload.text : JSON.stringify(share?.resourcePayload, null, 2);

  async function copyText() {
    if (!sharedText) return;
    await navigator.clipboard.writeText(sharedText);
    copied = true;
    toast.success('内容已复制');
  }
</script>

<svelte:head><title>Shared document · Treease</title></svelte:head>

<main class="min-h-screen bg-[var(--app-bg)] px-4 py-10 text-[var(--text-primary)] sm:px-8">
  <div class="mx-auto max-w-5xl">
    {#if !share && !error}
      <div class="flex min-h-[40vh] items-center justify-center text-[var(--text-muted)]"><LoaderCircle size={20} class="animate-spin" /></div>
    {:else if error}
      <section class="mx-auto max-w-md rounded-[18px] border border-[var(--border-muted)] bg-white p-8 text-center shadow-sm">
        <h1 class="text-xl font-semibold">Share link unavailable</h1>
        <p class="mt-2 text-sm text-[var(--text-muted)]">{error}</p>
        <a class="mt-6 inline-flex rounded-[9px] bg-[var(--accent)] px-4 py-2 text-sm text-white" href="/editor">Open Treease</a>
      </section>
    {:else if share}
      <section class="overflow-hidden rounded-[18px] border border-[var(--border-muted)] bg-white shadow-sm">
        <header class="flex flex-wrap items-center justify-between gap-3 border-b border-[var(--border-muted)] px-5 py-4">
          <div class="flex items-center gap-3"><FileJson size={18} class="text-[var(--accent)]" /><div><h1 class="font-semibold">Shared document</h1><p class="text-xs text-[var(--text-muted)]">Read-only snapshot · {share.resourcePayload.languageId ?? 'text'}</p></div></div>
          <div class="flex gap-2"><Button variant="outline" size="sm" on:click={copyText}><Copy size={13} class="mr-1" />{copied ? 'Copied' : 'Copy'}</Button><a href="/editor" class="inline-flex items-center rounded-[8px] bg-[var(--accent)] px-3 py-1.5 text-xs font-medium text-white">Open editor <ExternalLink size={12} class="ml-1" /></a></div>
        </header>
        <pre class="max-h-[70vh] overflow-auto p-5 text-[13px] leading-6 text-[var(--text-primary)]">{sharedText}</pre>
      </section>
    {/if}
  </div>
</main>
