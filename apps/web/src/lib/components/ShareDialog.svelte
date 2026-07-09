<script lang="ts">
  import { tick } from 'svelte';
  import { Check, Copy, Link2, LoaderCircle } from 'lucide-svelte';
  import { toast } from 'svelte-sonner';
  import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from './ui/dialog';
  import { Button } from './ui/button';
  import { createShareLink } from '../services/treease-server';
  import type { SupportedEditorLanguageId } from '../monaco/language-support';

  export let open = false;
  export let text = '';
  export let languageId: SupportedEditorLanguageId = 'json';

  let expiresInDays = 7;
  let shareUrl = '';
  let busy = false;
  let copied = false;

  $: if (!open) {
    shareUrl = '';
    copied = false;
    busy = false;
  }

  async function handleCreate() {
    busy = true;
    try {
      const share = await createShareLink({
        type: 'editor_text_snapshot',
        payload: { text, languageId },
      }, expiresInDays);
      shareUrl = share.shareUrl;
      await copyLink();
      toast.success('分享链接已创建并复制');
    } catch (error) {
      toast.error(error instanceof Error ? error.message : '创建分享链接失败');
    } finally {
      busy = false;
    }
  }

  async function copyLink() {
    if (!shareUrl || typeof navigator === 'undefined') return;
    await navigator.clipboard.writeText(shareUrl);
    copied = true;
    await tick();
  }
</script>

<Dialog bind:open>
  <DialogContent aria-label="Share document" data-testid="share-dialog" class="max-w-md">
    <DialogHeader>
      <DialogTitle>Share this document</DialogTitle>
    </DialogHeader>
    <div class="flex flex-col gap-4 text-[13px] text-[var(--text-muted)]">
      <p>创建一个只读快照链接，接收者无需登录即可查看。</p>
      <label class="flex items-center justify-between gap-3">
        <span>Link expires in</span>
        <select bind:value={expiresInDays} class="rounded-[8px] border border-[var(--border-muted)] bg-white px-2 py-1.5 text-[var(--text-primary)]">
          <option value={1}>1 day</option>
          <option value={7}>7 days</option>
          <option value={30}>30 days</option>
        </select>
      </label>
      {#if shareUrl}
        <div class="flex items-center gap-2 rounded-[10px] border border-[var(--border-muted)] bg-[var(--panel-bg-alt)] p-2">
          <Link2 size={14} class="shrink-0 text-[var(--accent)]" />
          <input readonly value={shareUrl} class="min-w-0 flex-1 bg-transparent text-[12px] text-[var(--text-primary)] outline-none" aria-label="Share URL" />
          <Button variant="outline" size="xs" iconOnly={true} aria-label="Copy share URL" on:click={copyLink}>
            {#if copied}<Check size={13} />{:else}<Copy size={13} />{/if}
          </Button>
        </div>
      {/if}
    </div>
    <DialogFooter>
      <Button variant="outline" on:click={() => (open = false)}>Close</Button>
      <Button disabled={busy || !text.trim()} on:click={handleCreate}>
        {#if busy}<LoaderCircle size={13} class="mr-1 animate-spin" />{/if}
        {shareUrl ? 'Create a new link' : 'Create share link'}
      </Button>
    </DialogFooter>
  </DialogContent>
</Dialog>
