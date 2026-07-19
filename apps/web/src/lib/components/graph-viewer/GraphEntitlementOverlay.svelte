<script lang="ts">
  import { ArrowUpRight, LockKeyhole, RefreshCw } from 'lucide-svelte';
  import { toast } from 'svelte-sonner';
  import { createBillingCheckoutLink } from '../../services/treease-server';
  import type { UsageBlock } from '../../billing/entitlement-gate';

  export let block: UsageBlock;
  export let onRefresh: () => Promise<void> = async () => {};

  let checkoutBusy = false;
  let refreshing = false;
  let capabilityLabel = '';

  $: capabilityLabel = block.capability === 'bidirectional_edit' ? '图编辑' : '大文件处理';

  async function startUpgrade(): Promise<void> {
    if (checkoutBusy) return;
    checkoutBusy = true;
    try {
      const checkout = await createBillingCheckoutLink('monthly', { successUrl: window.location.href });
      window.location.assign(checkout.url);
    } catch {
      checkoutBusy = false;
      toast.error('暂时无法打开结账页，请稍后重试。');
    }
  }

  async function refreshEntitlement(): Promise<void> {
    if (refreshing) return;
    refreshing = true;
    try {
      await onRefresh();
    } finally {
      refreshing = false;
    }
  }
</script>

<section class="graph-entitlement-overlay" data-testid="graph-entitlement-overlay" aria-live="polite">
  <div class="graph-entitlement-overlay__grid" aria-hidden="true"></div>
  <div class="graph-entitlement-overlay__card">
    <div class="graph-entitlement-overlay__mark"><LockKeyhole size={17} strokeWidth={2.25} /></div>
    <p class="graph-entitlement-overlay__eyebrow">本月 {capabilityLabel} 额度已用完</p>
    <h2>结果已经算好，升级后即可继续使用</h2>
    <p>当前图保持在这里，不会丢失。升级到 Pro 后可立即解除限制。</p>
    <div class="graph-entitlement-overlay__actions">
      <button type="button" class="graph-entitlement-overlay__upgrade" disabled={checkoutBusy} on:click={() => void startUpgrade()}>
        {checkoutBusy ? '正在打开结账页…' : '升级到 Pro'}
        <ArrowUpRight size={15} strokeWidth={2.2} />
      </button>
      <button type="button" class="graph-entitlement-overlay__refresh" disabled={refreshing} on:click={() => void refreshEntitlement()}>
        <RefreshCw class={refreshing ? 'animate-spin' : ''} size={14} strokeWidth={2} />
        我已升级
      </button>
    </div>
  </div>
</section>

<style>
  .graph-entitlement-overlay {
    position: absolute;
    inset: 0;
    z-index: 8;
    display: grid;
    place-items: center;
    overflow: hidden;
    background: color-mix(in srgb, var(--panel-bg-alt) 79%, rgba(15, 23, 42, 0.16));
    backdrop-filter: blur(8px) saturate(0.72);
  }

  .graph-entitlement-overlay__grid {
    position: absolute;
    inset: -30%;
    opacity: 0.34;
    background-image: linear-gradient(color-mix(in srgb, var(--border-muted) 62%, transparent) 1px, transparent 1px),
      linear-gradient(90deg, color-mix(in srgb, var(--border-muted) 62%, transparent) 1px, transparent 1px);
    background-size: 24px 24px;
    transform: rotate(-7deg) scale(1.16);
  }

  .graph-entitlement-overlay__card {
    position: relative;
    width: min(420px, calc(100% - 40px));
    padding: 28px;
    border: 1px solid color-mix(in srgb, var(--accent) 30%, var(--border-muted));
    border-radius: 16px;
    background: color-mix(in srgb, var(--panel-bg) 91%, transparent);
    box-shadow: 0 22px 70px rgba(15, 23, 42, 0.2);
  }

  .graph-entitlement-overlay__mark {
    display: grid;
    width: 34px;
    height: 34px;
    margin-bottom: 18px;
    place-items: center;
    border-radius: 10px;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }

  .graph-entitlement-overlay__eyebrow {
    margin: 0 0 7px;
    color: var(--accent);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
  }

  h2 {
    margin: 0;
    color: var(--text-primary);
    font-size: 19px;
    font-weight: 680;
    letter-spacing: -0.025em;
  }

  p:not(.graph-entitlement-overlay__eyebrow) {
    margin: 10px 0 0;
    color: var(--text-secondary);
    font-size: 13px;
    line-height: 1.65;
  }

  .graph-entitlement-overlay__actions {
    display: flex;
    flex-wrap: wrap;
    gap: 9px;
    margin-top: 22px;
  }

  button {
    display: inline-flex;
    min-height: 34px;
    align-items: center;
    justify-content: center;
    gap: 7px;
    border-radius: 8px;
    padding: 0 13px;
    font-size: 12px;
    font-weight: 650;
    transition: transform 140ms ease, box-shadow 140ms ease, background-color 140ms ease;
  }

  button:disabled { cursor: wait; opacity: 0.62; }

  .graph-entitlement-overlay__upgrade {
    border: 1px solid var(--accent);
    color: white;
    background: var(--accent);
    box-shadow: 0 7px 16px color-mix(in srgb, var(--accent) 24%, transparent);
  }

  .graph-entitlement-overlay__upgrade:not(:disabled):hover { transform: translateY(-1px); }

  .graph-entitlement-overlay__refresh {
    border: 1px solid var(--border-muted);
    color: var(--text-primary);
    background: var(--panel-bg);
  }

  .graph-entitlement-overlay__refresh:not(:disabled):hover { background: var(--panel-bg-alt); }
</style>
