<script lang="ts">
  import type { BillingPriceId } from '$lib/config/pricing';
  import type { UsageBlock } from '../billing/entitlement-gate';
  import type { PricingUsageNotice } from './PricingPlanGrid.svelte';

  type PricingPlanGridComponent = typeof import('./PricingPlanGrid.svelte').default;

  export let block: UsageBlock | null = null;
  export let pricingPlanGridComponent: PricingPlanGridComponent | null = null;
  export let usageNotice: PricingUsageNotice | null = null;
  export let onSelectPlan: (priceId: BillingPriceId) => void = () => {};
  export let actionDisabled: (plan: { billingPriceId?: BillingPriceId }) => boolean = () => false;
  export let actionLabel: (plan: { ctaLabel: string }) => string = (plan) => plan.ctaLabel;

  $: effectiveUsageNotice = usageNotice ?? (block
    ? {
        capability: block.capability === 'large_file_processing' ? 'Large-file processing' : 'Graph views',
        used: block.used,
        limit: block.limit,
        periodLabel: 'this month',
      }
    : null);
  $: capabilityLabel = block?.capability === 'large_file_processing' ? 'large-file processing' : 'graph views';
</script>

<section class="entitlement-overlay" data-testid="entitlement-overlay" aria-live="polite">
  <div class="entitlement-overlay__grid" aria-hidden="true"></div>
  <div class="entitlement-overlay__card">
    {#if pricingPlanGridComponent}
      <svelte:component
        this={pricingPlanGridComponent}
        compact
        title="Usage limit reached"
        titleId="entitlement-pricing-title"
        titleNoWrap
        descriptionNoWrap
        showKicker={false}
        showPlanHeading={false}
        description={block ? `Your ${capabilityLabel} limit has been reached. Upgrade to continue.` : 'Your last action used the final monthly run. Upgrade to continue.'}
        visiblePlanIds={['pro-monthly', 'pro-yearly']}
        usageNotice={effectiveUsageNotice}
        actionDisabled={actionDisabled}
        actionLabel={actionLabel}
        onSelectPlan={onSelectPlan}
      />
    {:else}
      <div class="entitlement-overlay__loading" aria-busy="true" aria-label="Loading pricing options">
        <span class="entitlement-overlay__spinner" aria-hidden="true"></span>
        <span>Loading pricing options…</span>
      </div>
    {/if}
  </div>
</section>

<style>
  .entitlement-overlay {
    position: absolute;
    inset: 0;
    z-index: 8;
    display: grid;
    place-items: center;
    overflow: hidden;
    padding: 28px;
    background: color-mix(in srgb, var(--panel-bg-alt) 79%, rgba(15, 23, 42, 0.16));
    backdrop-filter: blur(8px) saturate(0.72);
  }

  .entitlement-overlay__grid {
    position: absolute;
    inset: -30%;
    opacity: 0.34;
    background-image: linear-gradient(color-mix(in srgb, var(--border-muted) 62%, transparent) 1px, transparent 1px),
      linear-gradient(90deg, color-mix(in srgb, var(--border-muted) 62%, transparent) 1px, transparent 1px);
    background-size: 24px 24px;
    transform: rotate(-7deg) scale(1.16);
  }

  .entitlement-overlay__card {
    position: relative;
    width: min(560px, 100%);
    max-height: 100%;
    overflow: auto;
    padding: 24px;
    border: 1px solid color-mix(in srgb, var(--accent) 30%, var(--border-muted));
    border-radius: 16px;
    background: var(--panel-bg, #fff);
    box-shadow: 0 22px 70px rgba(15, 23, 42, 0.2);
    scrollbar-gutter: stable;
  }

  .entitlement-overlay__loading {
    display: grid;
    min-height: 420px;
    place-items: center;
    gap: 10px;
    color: var(--text-muted, #64748b);
    font-size: 14px;
  }

  .entitlement-overlay__spinner {
    width: 22px;
    height: 22px;
    border: 2px solid var(--border-muted, #e5e7eb);
    border-top-color: var(--accent, #2563eb);
    border-radius: 999px;
    animation: entitlement-overlay-spin 700ms linear infinite;
  }

  @keyframes entitlement-overlay-spin {
    to { transform: rotate(360deg); }
  }

  @media (max-width: 640px) {
    .entitlement-overlay { padding: 16px; place-items: center; }
    .entitlement-overlay__card { padding: 16px; }
  }
</style>
