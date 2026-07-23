<script lang="ts">
  import { onDestroy, tick } from 'svelte';
  import { Check, Copy, Link2, LoaderCircle } from 'lucide-svelte';
  import { toast } from 'svelte-sonner';
  import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from './ui/dialog';
  import { Button } from './ui/button';
  import { createShareLink, getCurrentSubscription } from '../services/treease-server';
  import type { ShareResource } from '../share/share-resource';

  export let open = false;
  export let createResource: (() => Promise<ShareResource>) | null = null;

  let expiresInDays = 7;
  let shareUrl = '';
  let busy = false;
  let copied = false;
  let isPaidUser = false;
  let subscriptionRequest = 0;

  $: if (open) {
    void loadSubscription();
  } else {
    resetDialog();
  }

  onDestroy(() => {
    subscriptionRequest += 1;
  });

  function resetDialog(): void {
    subscriptionRequest += 1;
    expiresInDays = 7;
    isPaidUser = false;
    shareUrl = '';
    copied = false;
    busy = false;
  }

  async function loadSubscription(): Promise<void> {
    const request = ++subscriptionRequest;
    try {
      const subscription = await getCurrentSubscription();
      if (request !== subscriptionRequest || !open) return;
      isPaidUser = subscription.tier === 'pro';
    } catch {
      if (request !== subscriptionRequest || !open) return;
      isPaidUser = false;
    }
  }

  async function handleCreate() {
    busy = true;
    try {
      if (!createResource) throw new Error('Editor is not ready to share.');
      const share = await createShareLink(await createResource(), expiresInDays);
      shareUrl = share.shareUrl;
      await copyLink();
      toast.success('Share link created and copied.');
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Unable to create share link.');
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
  <DialogContent aria-labelledby="share-dialog-title" data-testid="share-dialog" class="max-w-md">
    <DialogHeader>
      <DialogTitle id="share-dialog-title">Share this document</DialogTitle>
    </DialogHeader>
    <div class="flex flex-col gap-4 text-[13px] text-[var(--text-muted)]">
      <p>Create a read-only snapshot link that anyone can open without signing in.</p>
      <div class="flex items-center justify-between gap-3">
        <span>Link expires in</span>
        {#if isPaidUser}
          <select bind:value={expiresInDays} class="rounded-[8px] border border-[var(--border-muted)] bg-white px-2 py-1.5 text-[var(--text-primary)]" aria-label="Link expiration">
            <option value={1}>1 day</option>
            <option value={7}>7 days</option>
            <option value={30}>30 days</option>
            <option value={365}>365 days</option>
          </select>
        {:else}
          <span class="text-[var(--text-primary)]">7 days</span>
        {/if}
      </div>
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
      <Button disabled={busy || !createResource} on:click={handleCreate}>
        {#if busy}<LoaderCircle size={13} class="mr-1 animate-spin" />{/if}
        {shareUrl ? 'Create a new link' : 'Create share link'}
      </Button>
    </DialogFooter>
  </DialogContent>
</Dialog>
