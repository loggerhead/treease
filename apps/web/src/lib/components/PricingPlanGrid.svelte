<script context="module" lang="ts">
  export type PricingUsageNotice = {
    capability: string;
    used: number;
    limit: number;
    periodLabel: string;
  };
</script>

<script lang="ts">
  import NumberFlow from '@number-flow/svelte';
  import { Info } from 'lucide-svelte';
  import {
    fixedYearlySavingsPercent,
    pricingConfig,
    type BillingPriceId,
    type PricingFeature,
    type PricingPlan,
  } from '$lib/config/pricing';

  export let showIntro = true;
  export let showKicker = true;
  export let showPlanHeading = true;
  export let compact = false;
  export let title = pricingConfig.title;
  export let titleId = 'pricing-title';
  export let titleNoWrap = false;
  export let descriptionNoWrap = false;
  export let description = pricingConfig.description;
  export let visiblePlanIds: readonly PricingPlan['id'][] | undefined = undefined;
  export let usageNotice: PricingUsageNotice | null = null;
  export let onSelectPlan: ((priceId: BillingPriceId) => void) | undefined = undefined;
  export let actionLabel: ((plan: PricingPlan) => string) | undefined = undefined;
  export let actionDisabled: ((plan: PricingPlan) => boolean) | undefined = undefined;
  export let actionTooltip: ((plan: PricingPlan) => string | null) | undefined = undefined;

  let visiblePlans: PricingPlan[] = [];
  let billingPlans: PricingPlan[] = [];
  let displayedPlans: PricingPlan[] = [];
  let hasBillingTabs = false;
  let selectedBillingPriceId: BillingPriceId = 'monthly';

  $: visiblePlans = visiblePlanIds
    ? pricingConfig.plans.filter((plan) => visiblePlanIds!.includes(plan.id))
    : pricingConfig.plans;
  $: billingPlans = visiblePlans.filter((plan) => plan.billingPriceId);
  $: hasBillingTabs = billingPlans.some((plan) => plan.billingPriceId === 'monthly') && billingPlans.some((plan) => plan.billingPriceId === 'yearly');
  $: if (hasBillingTabs && !billingPlans.some((plan) => plan.billingPriceId === selectedBillingPriceId)) {
    selectedBillingPriceId = billingPlans[0]?.billingPriceId ?? 'monthly';
  }
  $: displayedPlans = hasBillingTabs
    ? visiblePlans.filter((plan) => !plan.billingPriceId || plan.billingPriceId === selectedBillingPriceId)
    : visiblePlans;

  function parsePlanPrice(price: string | undefined): number | null {
    if (!price) return null;
    const amount = Number(price.replace(/[^\d.-]/g, ''));
    return Number.isFinite(amount) ? amount : null;
  }

  function splitFeatureLabel(feature: PricingFeature): [string, string | null, string] {
    if (!feature.emphasis) return [feature.label, null, ''];
    const index = feature.label.indexOf(feature.emphasis);
    if (index === -1) return [feature.label, null, ''];
    return [feature.label.slice(0, index), feature.emphasis, feature.label.slice(index + feature.emphasis.length)];
  }
</script>

<section class:pricing-plan-grid--compact={compact} class="pricing-plan-grid" aria-labelledby="pricing-title">
  {#if showIntro}
    <div class="pricing-plan-grid__intro">
      {#if showKicker}<p class="pricing-plan-grid__kicker">Pricing</p>{/if}
      <h2 id={titleId} class:pricing-plan-grid__title--no-wrap={titleNoWrap}>{title}</h2>
      <p class:pricing-plan-grid__description--no-wrap={descriptionNoWrap}>{description}</p>
      {#if usageNotice}
        <div class="pricing-plan-grid__usage" role="status">
          <div class="pricing-plan-grid__usage-row">
            <span>{usageNotice.capability}</span>
            <strong>{usageNotice.used} / {usageNotice.limit}</strong>
          </div>
          <span class="pricing-plan-grid__usage-cycle">Monthly allowance · {usageNotice.periodLabel}</span>
        </div>
      {/if}
    </div>
  {/if}

  {#if hasBillingTabs}
    <div class="pricing-plan-grid__billing-tabs" role="tablist" aria-label="Billing cycle">
      <button
        type="button"
        class:pricing-plan-grid__billing-tab--active={selectedBillingPriceId === 'monthly'}
        role="tab"
        aria-selected={selectedBillingPriceId === 'monthly'}
        on:click={() => (selectedBillingPriceId = 'monthly')}
      >
        Monthly
      </button>
      <button
        type="button"
        class:pricing-plan-grid__billing-tab--active={selectedBillingPriceId === 'yearly'}
        role="tab"
        aria-selected={selectedBillingPriceId === 'yearly'}
        on:click={() => (selectedBillingPriceId = 'yearly')}
      >
        Yearly <span>Save {fixedYearlySavingsPercent ?? '-'}%</span>
      </button>
    </div>
  {/if}

  <div
    class:pricing-plan-grid__cards--two={displayedPlans.length === 2}
    class:pricing-plan-grid__cards--one={displayedPlans.length === 1}
    class="pricing-plan-grid__cards"
  >
    {#each displayedPlans as plan}
      {@const priceAmount = parsePlanPrice(plan.price)}
      <article class:pricing-plan-card--featured={plan.featured && displayedPlans.length > 1} class:pricing-plan-card--free={plan.id === 'free'} class:pricing-plan-card--without-heading={!showPlanHeading} class="pricing-plan-card">
        {#if showPlanHeading}
          {#if plan.billingPriceId === 'yearly'}
            <p class="pricing-plan-card__eyebrow">SAVE {fixedYearlySavingsPercent ?? '-'}%</p>
          {:else if plan.eyebrow}
            <p class="pricing-plan-card__eyebrow">{plan.eyebrow}</p>
          {/if}
        {/if}
        {#if showPlanHeading}<h3>{plan.name}</h3>{/if}
        <div class="pricing-plan-card__price">
          <strong aria-label={plan.price ?? ''}>
            {#if priceAmount !== null}
              <NumberFlow value={priceAmount} format={{ style: 'currency', currency: 'USD', minimumFractionDigits: 2 }} />
            {:else}
              {plan.price}
            {/if}
          </strong>
          <span>{plan.cadence}</span>
        </div>
        <p class="pricing-plan-card__description">{plan.description}</p>
        {#if plan.billingPriceId}
          {@const label = actionLabel?.(plan) ?? plan.ctaLabel}
          {@const tooltip = actionTooltip?.(plan)}
          <span class="pricing-plan-card__cta-wrap" data-tooltip={tooltip ?? undefined}>
            <button type="button" class="pricing-plan-card__cta" disabled={actionDisabled?.(plan) ?? false} on:click={() => onSelectPlan?.(plan.billingPriceId!)}>{label}</button>
          </span>
        {:else}
          <a class="pricing-plan-card__cta" href={plan.ctaHref}>{plan.ctaLabel}</a>
        {/if}
        <ul>
          {#each plan.features as feature}
            {@const [prefix, emphasis, suffix] = splitFeatureLabel(feature)}
            <li class:pricing-plan-card__feature--without-check={feature.showCheck === false}>
              {prefix}{#if emphasis}<mark>{emphasis}</mark>{/if}{suffix}
              {#if feature.info}
                <span class="pricing-plan-card__feature-info" role="img" aria-label={feature.info} title={feature.info}>
                  <Info size={12} strokeWidth={2.1} />
                </span>
              {/if}
            </li>
          {/each}
        </ul>
      </article>
    {/each}
  </div>
</section>

<style>
  .pricing-plan-grid { display: grid; gap: 30px; }
  .pricing-plan-grid__intro { display: grid; gap: 12px; max-width: 56ch; }
  .pricing-plan-grid__intro p { margin: 0; color: var(--muted, #536273); line-height: 1.6; }
  .pricing-plan-grid__intro h2 { margin: 0; color: var(--ink, #10192a); font-family: var(--font-display, inherit); font-size: clamp(2rem, 4vw, 3.25rem); letter-spacing: -0.05em; }
  .pricing-plan-grid__title--no-wrap { white-space: nowrap; }
  .pricing-plan-grid__description--no-wrap { white-space: nowrap; }
  .pricing-plan-grid__kicker, .pricing-plan-card__eyebrow { margin: 0; color: var(--accent-strong, #1745b5) !important; font-size: 12px; font-weight: 800; letter-spacing: .14em; text-transform: uppercase; }
  .pricing-plan-grid__usage { display: grid; gap: 5px; width: min(360px, 100%); padding: 10px 12px; border: 1px solid #fed7aa; border-radius: 10px; color: #0f172a; background: #fffaf5; font-size: 13px; line-height: 1.45; }
  .pricing-plan-grid__usage-row { display: flex; align-items: center; justify-content: space-between; gap: 16px; color: #64748b; }
  .pricing-plan-grid__usage-row strong { color: #0f172a; font-size: 14px; }
  .pricing-plan-grid__usage-cycle { color: #9a3412; font-size: 12px; }
  .pricing-plan-grid__billing-tabs { display: inline-flex; width: fit-content; padding: 4px; border: 1px solid var(--line, #dce5f0); border-radius: 999px; background: #f3f6fb; }
  .pricing-plan-grid__billing-tabs button { min-width: 112px; border: 0; border-radius: 999px; padding: 9px 16px; color: var(--muted, #536273); background: transparent; cursor: pointer; font-size: 13px; font-weight: 750; }
  .pricing-plan-grid__billing-tabs button:hover { color: var(--ink, #10192a); }
  .pricing-plan-grid__billing-tab--active { color: var(--ink, #10192a) !important; background: #fff !important; box-shadow: 0 2px 8px rgba(16,25,42,.08); }
  .pricing-plan-grid__billing-tabs span { margin-left: 4px; color: var(--accent-strong, #1745b5); font-size: 11px; }
  .pricing-plan-grid__cards { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 18px; align-items: stretch; }
  .pricing-plan-grid__cards--two { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .pricing-plan-grid__cards--one { grid-template-columns: minmax(0, 1fr); }
  .pricing-plan-card { display: flex; min-width: 0; flex-direction: column; gap: 22px; padding: 30px 26px 26px; border: 1px solid var(--line, #dce5f0); border-radius: 24px; color: var(--ink, #10192a); background: rgba(255,255,255,.8); box-shadow: var(--shadow, 0 12px 30px rgba(16,25,42,.08)); }
  .pricing-plan-card--featured { border-color: rgba(45,99,226,.42); color: #f8fbff; background: radial-gradient(circle at 100% 0%, rgba(93,143,255,.24), transparent 34%), linear-gradient(180deg, #1a315f, #102044); box-shadow: 0 28px 60px rgba(24,61,135,.22); transform: translateY(-10px); }
  h3 { margin: 0; font-size: 28px; }
  .pricing-plan-card__description { min-width: 0; margin: -10px 0 0; overflow: hidden; color: var(--muted, #536273); font-size: 14px; line-height: 1.4; text-overflow: ellipsis; white-space: nowrap; }
  .pricing-plan-card--featured .pricing-plan-card__description { color: rgba(226,232,240,.8); }
  .pricing-plan-card__price { display: flex; align-items: baseline; gap: 6px; padding: 16px 0; border-top: 1px solid var(--line, #dce5f0); border-bottom: 1px solid var(--line, #dce5f0); }
  .pricing-plan-card--without-heading .pricing-plan-card__price { padding-top: 0; border-top: 0; }
  .pricing-plan-card--featured .pricing-plan-card__price { border-color: rgba(191,219,254,.2); }
  .pricing-plan-card__price strong { font-family: var(--font-display, inherit); font-size: clamp(2.2rem, 4vw, 3rem); letter-spacing: -.06em; }
  .pricing-plan-card__price span { color: var(--muted-soft, #8290a3); font-size: 14px; }
  .pricing-plan-card--featured .pricing-plan-card__price span { color: #abc0e3; }
  .pricing-plan-card__cta { display: inline-flex; min-height: 46px; width: 100%; align-items: center; justify-content: center; border: 1px solid rgba(45,99,226,.3); border-radius: 999px; color: var(--accent-strong, #1745b5); background: rgba(255,255,255,.76); cursor: pointer; font-size: 14px; font-weight: 800; text-decoration: none; }
  .pricing-plan-card__cta:not(:disabled):hover { border-color: rgba(45,99,226,.62); background: #fff; transform: translateY(-2px); }
  .pricing-plan-card__cta:disabled { cursor: default; opacity: .72; color: var(--muted-soft, #8290a3); background: #e8edf5; }
  .pricing-plan-card--featured .pricing-plan-card__cta { border-color: transparent; color: #173b8e; background: #dbeafe; }
  ul { display: grid; gap: 13px; margin: 0; padding: 0; list-style: none; }
  li { position: relative; padding-left: 24px; color: var(--muted, #536273); font-size: 14px; line-height: 1.45; }
  li::before { position: absolute; left: 0; color: var(--accent-strong, #1745b5); content: '✓'; font-weight: 900; }
  .pricing-plan-card__feature--without-check::before { content: none; }
  .pricing-plan-card__feature-info { display: inline-flex; margin-left: 4px; color: currentColor; cursor: help; opacity: .62; vertical-align: -2px; transition: opacity 140ms ease; }
  .pricing-plan-card__feature-info:hover { opacity: 1; }
  .pricing-plan-card--featured .pricing-plan-card__feature-info { color: #93c5fd; }
  mark { padding: 1px 3px; border-radius: 3px; color: var(--accent-strong, #1745b5); background: rgba(45,99,226,.12); font-weight: 700; }
  .pricing-plan-card--free mark { color: inherit; background: transparent; font-weight: inherit; }
  .pricing-plan-card--featured li { color: rgba(226,232,240,.88); }
  .pricing-plan-card--featured li::before { color: #93c5fd; }
  .pricing-plan-card--featured mark { color: #dbeafe; background: rgba(147,197,253,.18); }
  @media (max-width: 1120px) { .pricing-plan-grid__cards { grid-template-columns: 1fr; } .pricing-plan-card--featured { transform: none; } }
  @media (max-width: 860px) { .pricing-plan-card__description { min-height: 0; } }
  .pricing-plan-grid--compact { gap: 20px; }
  .pricing-plan-grid--compact .pricing-plan-grid__intro { gap: 8px; }
  .pricing-plan-grid--compact .pricing-plan-grid__intro h2 { font-size: 30px; letter-spacing: -0.035em; }
  .pricing-plan-grid--compact .pricing-plan-grid__intro > p { font-size: 15px; }
  .pricing-plan-grid--compact .pricing-plan-card { padding: 20px; gap: 16px; border-radius: 18px; }
  .pricing-plan-grid--compact .pricing-plan-card__description { min-height: 0; }
  .pricing-plan-grid--compact h3 { font-size: 22px; }
</style>
