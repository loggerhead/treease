<script lang="ts">
  import { browser } from '$app/environment';
  import { assetUrl, r2Assets } from '$lib/assets';
  import HomeHeroDemoDeck from '$lib/components/HomeHeroDemoDeck.svelte';
  import SiteFooter from '$lib/components/SiteFooter.svelte';
  import SiteHeader from '$lib/components/SiteHeader.svelte';
  import LoginDialog from '$lib/components/LoginDialog.svelte';
  import {
    openPreparedBillingCheckout,
    prewarmBillingCheckout,
    startBillingCheckout,
    type PreparedBillingCheckout,
  } from '$lib/billing/checkout-flow';
  import { authUser } from '$lib/auth/auth-user-store';
  import {
    fixedYearlySavingsPercent,
    pricingConfig,
    type BillingPriceId,
    type PricingFeature,
  } from '$lib/config/pricing';
  import { homeHeaderNavItems } from '$lib/navigation/home-header-nav';
  import { trackEvent, trackSeoConversion } from '$lib/analytics/ga4';
  import { toast } from 'svelte-sonner';
  import SeoHead from '$lib/components/SeoHead.svelte';
  import { signOut } from '$lib/auth/supabase-auth';
  import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '$lib/components/ui/dialog';
  import {
    changeBillingPlan,
    createBillingPortalLink,
    type BillingPricingPrewarm,
  } from '$lib/services/treease-server';

  let cliInstallExpanded = false;
  let checkoutBusy = false;
  let loginOpen = false;
  let pricingPrewarmKey: string | null = null;
  let pricingPrewarm: Promise<BillingPricingPrewarm> | null = null;
  let checkoutPreparations: Partial<Record<BillingPriceId, Promise<PreparedBillingCheckout>>> = {};
  let currentSubscription: BillingPricingPrewarm['subscription'] = null;
  let billingPlanPrices: BillingPricingPrewarm['plans'] = [];
  let subscriptionLookupState: 'idle' | 'loading' | 'ready' | 'error' = 'idle';
  let planChangeBusy: BillingPriceId | null = null;
  let planChangeNotice: string | null = null;
  let scheduledPlanChange: BillingPriceId | null = null;
  let planChangeDialogOpen = false;
  let pendingPlanChange: { priceId: BillingPriceId; plan: typeof pricingConfig.plans[number] } | null = null;
  const yearlySavings = fixedYearlySavingsPercent;
  import { serializeJsonLd, siteOrigin, socialLinks } from '$lib/seo/site-seo';
  const pageTitle = 'Treease: JSON, YAML & Structured Text Viewer with Graphs';
  const pageDescription =
    'View, format, compare, and edit JSON, YAML, TOML, CSV, and embedded payloads in a private browser workspace with graph and source views.';
  const homeJsonLd = serializeJsonLd({
    '@context': 'https://schema.org',
    '@type': 'WebApplication',
    name: 'Treease',
    url: siteOrigin,
    description: pageDescription,
    applicationCategory: 'DeveloperApplication',
    operatingSystem: 'Web browser',
    publisher: {
      '@type': 'Organization',
      name: 'Treease',
      url: siteOrigin,
      sameAs: ['https://github.com/loggerhead/treease', 'https://crates.io/crates/treease-cli', socialLinks.x, socialLinks.discord],
    },
  });

  function checkoutReturnUrl() {
    return { successUrl: new URL('/editor', window.location.origin).toString() };
  }

  function splitFeatureLabel(feature: PricingFeature): [string, string | null, string] {
    if (!feature.emphasis) return [feature.label, null, ''];

    const index = feature.label.indexOf(feature.emphasis);
    if (index === -1) return [feature.label, null, ''];

    return [
      feature.label.slice(0, index),
      feature.emphasis,
      feature.label.slice(index + feature.emphasis.length),
    ];
  }

  function startPricingPrewarm(): void {
    const key = $authUser?.id ?? 'anonymous';
    if (pricingPrewarmKey === key) return;

    pricingPrewarmKey = key;
    checkoutPreparations = {};
    currentSubscription = null;
    billingPlanPrices = [];
    subscriptionLookupState = 'loading';
    planChangeNotice = null;
    scheduledPlanChange = null;
    const prewarm = prewarmBillingCheckout(checkoutReturnUrl());
    pricingPrewarm = prewarm;
    void prewarm.then((result) => {
      if (pricingPrewarm !== prewarm) return;
      currentSubscription = result.subscription;
      billingPlanPrices = result.plans;
      subscriptionLookupState = 'ready';
      checkoutPreparations = Object.fromEntries(
        (result.checkouts ?? []).map((checkout) => [
          checkout.priceId,
          Promise.resolve({ priceId: checkout.priceId, checkoutUrl: checkout.url }),
        ]),
      );
    }).catch(() => {
      if (pricingPrewarm !== prewarm) return;
      subscriptionLookupState = 'error';
      billingPlanPrices = [];
      pricingPrewarm = null;
    });
  }

  $: if (browser) {
    const authKey = $authUser?.id ?? 'anonymous';
    if (pricingPrewarmKey !== authKey) startPricingPrewarm();
  }

  async function handleLogout(): Promise<void> {
    try {
      await signOut();
      toast.success('You are now logged out.');
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : String(cause);
      toast.error(`Logout failed: ${message}`);
    }
  }

  async function startCheckout(priceId: 'monthly' | 'yearly'): Promise<void> {
    checkoutBusy = true;
    try {
      const preparation = checkoutPreparations[priceId];
      let outcome;
      try {
        let prepared: PreparedBillingCheckout | null = preparation ? await preparation : null;
        if (!prepared) {
          const checkout = (await pricingPrewarm)?.checkouts?.find((entry) => entry.priceId === priceId);
          prepared = checkout ? { priceId: checkout.priceId, checkoutUrl: checkout.url } : null;
        }
        outcome = prepared
          ? await openPreparedBillingCheckout(prepared)
          : await startBillingCheckout(priceId, checkoutReturnUrl());
      } catch {
        outcome = await startBillingCheckout(priceId, checkoutReturnUrl());
      }
      if (outcome.status === 'login-required') {
        loginOpen = true;
        return;
      }
      if (outcome.status === 'failed') toast.error(outcome.message);
    } finally {
      checkoutBusy = false;
    }
  }

  function currentPlanIsPaid(subscription = currentSubscription): boolean {
    if (!subscription || subscription.tier !== 'pro') return false;
    if (subscription.currentPeriodEnd) return Date.parse(subscription.currentPeriodEnd) > Date.now();
    return subscription.status === 'active';
  }

  function billingNeedsManagement(subscription = currentSubscription): boolean {
    if (!subscription?.providerSubscriptionId) return false;
    if (subscription.currentPeriodEnd && Date.parse(subscription.currentPeriodEnd) <= Date.now()) return false;
    return subscription.status === 'past_due'
      || subscription.status === 'canceled'
      || (subscription.status === 'inactive' && subscription.tier === 'free');
  }

  function planActionLabel(
    plan: typeof pricingConfig.plans[number],
    subscription: BillingPricingPrewarm['subscription'],
    lookupState: typeof subscriptionLookupState,
    busyPriceId: BillingPriceId | null,
    scheduledPriceId: BillingPriceId | null,
  ): string {
    if (!plan.billingPriceId) return plan.ctaLabel;
    if (busyPriceId === plan.billingPriceId) return 'Updating plan…';
    if (billingNeedsManagement(subscription)) return 'Manage billing';
    if (lookupState !== 'ready' || !currentPlanIsPaid(subscription)) return plan.ctaLabel;
    if (scheduledPriceId === plan.billingPriceId) return 'Scheduled';
    if (subscription?.billingCadence === plan.billingPriceId) return 'Current plan';
    if (subscription?.status === 'past_due') return 'Manage billing';
    return 'Change plan';
  }

  function formatPlanPrice(priceId: BillingPriceId): string {
    const price = billingPlanPrices.find((entry) => entry.priceId === priceId);
    if (price) {
      const amount = new Intl.NumberFormat('en-US', { style: 'currency', currency: price.currency }).format(price.amount / 100);
      const interval = price.intervalCount === 1 ? price.interval : `${price.intervalCount} ${price.interval}s`;
      return `${amount} / ${interval}`;
    }

    const plan = pricingConfig.plans.find((entry) => entry.billingPriceId === priceId);
    return `${plan?.price ?? 'Price unavailable'} ${plan?.cadence ?? ''}`.trim();
  }

  function formatRenewalDate(subscription = currentSubscription): string {
    return subscription?.currentPeriodEnd
      ? new Intl.DateTimeFormat('en-US', { dateStyle: 'medium' }).format(new Date(subscription.currentPeriodEnd))
      : 'your next renewal';
  }

  function formatTodayDate(): string {
    return new Intl.DateTimeFormat('en-US', { dateStyle: 'medium' }).format(new Date());
  }

  function planChangeChargesImmediately(priceId: BillingPriceId, subscription = currentSubscription): boolean {
    return subscription?.billingCadence === 'monthly' && priceId === 'yearly';
  }

  function planActionTooltip(
    plan: typeof pricingConfig.plans[number],
    subscription: BillingPricingPrewarm['subscription'],
    lookupState: typeof subscriptionLookupState,
    scheduledPriceId: BillingPriceId | null,
  ): string | null {
    const label = planActionLabel(plan, subscription, lookupState, null, scheduledPriceId);
    if (label === 'Scheduled' && plan.billingPriceId) {
      return `Scheduled: switches to ${plan.name} on ${formatRenewalDate(subscription)}. Next charge: ${formatPlanPrice(plan.billingPriceId)}.`;
    }
    if (label === 'Current plan') {
      return `Current plan: Pro ${subscription?.billingCadence ?? 'monthly'} · access through ${formatRenewalDate(subscription)}`;
    }
    if (label !== 'Change plan' || !plan.billingPriceId) return null;

    const timing = planChangeChargesImmediately(plan.billingPriceId, subscription)
      ? 'A prorated payment is required today.'
      : `Starts at your next renewal on ${formatRenewalDate(subscription)}.`;
    return `Switch to ${plan.name}. ${timing} Next charge: ${formatPlanPrice(plan.billingPriceId)}.`;
  }

  function openPlanChangeDialog(priceId: BillingPriceId): void {
    const plan = pricingConfig.plans.find((entry) => entry.billingPriceId === priceId);
    if (!plan) return;
    pendingPlanChange = { priceId, plan };
    planChangeDialogOpen = true;
  }

  async function openBillingPortal(): Promise<void> {
    try {
      const { url } = await createBillingPortalLink();
      window.location.assign(url);
    } catch {
      toast.error('Unable to open plan management. Please try again later.');
    }
  }

  async function handlePlanAction(priceId: BillingPriceId): Promise<void> {
    if (planChangeBusy || checkoutBusy) return;

    if (billingNeedsManagement()) {
      await openBillingPortal();
      return;
    }

    if (!currentPlanIsPaid()) {
      await startCheckout(priceId);
      return;
    }

    if (currentSubscription?.billingCadence === priceId) return;
    if (currentSubscription?.status === 'past_due' || currentSubscription?.status === 'canceled') {
      await openBillingPortal();
      return;
    }

    openPlanChangeDialog(priceId);
  }

  async function confirmPlanChange(): Promise<void> {
    const pending = pendingPlanChange;
    if (!pending || planChangeBusy || checkoutBusy) return;

    const { priceId } = pending;
    const cadenceLabel = priceId === 'yearly' ? 'yearly' : 'monthly';
    planChangeDialogOpen = false;
    planChangeBusy = priceId;
    try {
      await changeBillingPlan(priceId);
      if (planChangeChargesImmediately(priceId)) {
        currentSubscription = currentSubscription
          ? { ...currentSubscription, billingCadence: priceId }
          : currentSubscription;
        scheduledPlanChange = null;
        planChangeNotice = `Pro ${cadenceLabel} is active after the prorated payment is processed.`;
      } else {
        scheduledPlanChange = priceId;
        planChangeNotice = `Pro ${cadenceLabel} will start at your next renewal on ${formatRenewalDate()}.`;
      }
      toast.success(planChangeNotice);
    } catch (cause) {
      toast.error(cause instanceof Error ? cause.message : 'Unable to update your plan.');
    } finally {
      planChangeBusy = null;
      pendingPlanChange = null;
    }
  }

  const valuePoints = [
    'Open a local file and see its structure without leaving the source.',
    'Trace fields across graph, tree path, and source text without losing place.',
    'Review, convert, and export only after the structure looks right.'
  ];

  const faqItems = [
    {
      question: 'Why visualize structured text in Treease?',
      answer:
        'Because raw JSON, YAML, TOML, CSV, or embedded payloads get hard to follow long before they become hard to edit. Treease turns source text into a graph so users can see the structure first, then trace, edit, compare, and export with context.'
    },
    {
      question: 'What kinds of files and workflows does Treease cover?',
      answer:
        'Treease supports local import, format and minify commands, key sorting, active-container isolation inside mixed logs, graph search with tree path reveal, local value preview, synchronized text and graph edits, structural compare with text fallback, export preview, and visible progress for large JSON imports.'
    },
    {
      question: 'What makes Treease different from a plain editor or JSON viewer?',
      answer:
        'Treease keeps source text and graph context attached to the same document state. Instead of showing a disconnected preview, it lets users move from text to graph to tree path to output while staying anchored to the same file.'
    },
    {
      question: 'When should I use Treease instead of a terminal tool or general editor?',
      answer:
        'Use Treease when seeing the structure matters: inspecting nested data, tracing fields, checking changes, or previewing converted output. General-purpose editing, custom CLI filters, and batch automation still belong in an editor or terminal when the job stops being document-centric.'
    }
  ];
  const homeFaqJsonLd = serializeJsonLd({
    '@context': 'https://schema.org',
    '@type': 'FAQPage',
    mainEntity: faqItems.map(({ question, answer }) => ({
      '@type': 'Question',
      name: question,
      acceptedAnswer: { '@type': 'Answer', text: answer },
    })),
  });

</script>

<SeoHead title={pageTitle} description={pageDescription} canonical={`${siteOrigin}/`} jsonLd={[homeJsonLd, homeFaqJsonLd]} />

<div class="landing">
  <div class="landing-shell">
    <SiteHeader navItems={[...homeHeaderNavItems]} onLogin={() => (loginOpen = true)} onLogout={handleLogout} />

    <main class="landing-main" id="main-content" tabindex="-1" aria-labelledby="hero-title">
      <section class="hero">
        <div class="hero-copy">
          <p class="hero-kicker">Visualize structured text</p>
          <h1 id="hero-title">JSON, YAML, and structured text viewer with graphs.</h1>
          <p class="hero-lede">
            Treease turns JSON, YAML, TOML, CSV, and embedded payloads into a
            graph you can inspect, trace, edit, compare, and export with
            confidence.
          </p>
          <div class="hero-cta-stack">
            <div class="hero-actions">
              <a
                class="primary-cta"
                href="/editor"
                on:click={() => trackSeoConversion('editor_open', { source: 'landing' })}
              >Open Editor</a>
              <button
                type="button"
                class="secondary-cta"
                aria-expanded={cliInstallExpanded}
                aria-controls="hero-cli-install"
                on:click={() => {
                  cliInstallExpanded = !cliInstallExpanded;
                }}
              >
                Install the CLI
              </button>
            </div>
            {#if cliInstallExpanded}
              <div class="cli-quickstart" id="hero-cli-install" role="region" aria-label="Treease CLI quick start">
                <div class="cli-quickstart__label">
                  <strong>Install the CLI, then open a readonly local graph view.</strong>
                </div>
                <div class="cli-quickstart__commands">
                  <code>cargo install treease-cli</code>
                  <code>treease web '.services.api' config.yaml</code>
                </div>
              </div>
            {/if}
          </div>
        </div>

        <div class="hero-demo">
          <HomeHeroDemoDeck />
        </div>
      </section>

      <section class="value-strip" aria-labelledby="value-title">
        <div class="section-copy section-copy--compact">
          <h2 id="value-title">Visual structure stays attached to the file.</h2>
          <p>
            Treease keeps source text, graph context, compare, and export
            preview attached to the same working document, so users can
            understand the structure before they act on it.
          </p>
        </div>

        <div class="value-points">
          {#each valuePoints as point}
            <article class="value-point">
              <p>{point}</p>
            </article>
          {/each}
        </div>
      </section>

      <section class="capabilities-section" id="features" aria-labelledby="features-title">
        <div class="section-copy">
          <h2 id="features-title">See, trace, and edit with context.</h2>
          <p>
            Treease is built for the repetitive work around structured files:
            opening them, seeing their shape, tracing exact fields, and making
            changes without losing the source.
          </p>
        </div>

        <div class="capability-grid" role="region" aria-labelledby="features-title">
          <article class="capability-card capability-card--hero">
            <div class="story-copy">
              <h3>Open a local file and see the real structure.</h3>
              <p>
                Load supported structured files directly into the editor and
                visualize their shape from the first step.
              </p>
            </div>
            <div class="story-media story-media--contain">
              <img
                src={assetUrl(r2Assets.landingFeatureImport)}
                alt="Importing a YAML file into the Treease editor."
                loading="lazy"
              />
            </div>
          </article>

          <article class="capability-card capability-card--soft">
            <div class="story-copy">
              <h3>Clean up the current document in place.</h3>
              <p>
                Format, minify, or sort keys without leaving the editor or
                breaking visual context.
              </p>
            </div>
            <div class="story-media">
              <img
                src={assetUrl(r2Assets.landingFeatureFormat)}
                alt="Sorting JSON keys from the command search inside Treease."
                loading="lazy"
              />
            </div>
          </article>

          <article class="capability-card capability-card--dark">
            <div class="story-copy">
              <h3>Lift the active JSON container out of mixed logs.</h3>
              <p>
                Put the cursor inside a JSONL row or embedded payload and let
                the graph isolate the active container from surrounding log
                noise.
              </p>
            </div>
            <div class="story-media story-media--dark story-media--contain story-media--wide-short">
              <img
                src={assetUrl(r2Assets.landingFeatureContainer)}
                alt="Isolating an embedded JSON payload from mixed log text in Treease."
                loading="lazy"
              />
            </div>
          </article>

          <article class="capability-card capability-card--surface">
            <div class="story-copy">
              <h3>Trace a field from graph to source.</h3>
              <p>
                Search the graph, land on the exact node, and keep tree path,
                highlight, and source reveal aligned.
              </p>
            </div>
            <div class="story-media story-media--contain story-media--wide-short">
              <img
                src={assetUrl(r2Assets.landingFeatureReveal)}
                alt="Tracing a selected field through tree path breadcrumbs in Treease."
                loading="lazy"
              />
            </div>
          </article>

          <article class="capability-card capability-card--copy">
            <div class="story-copy">
              <h3>Edit either side and keep both in sync.</h3>
              <p>
                Update a value in graph or source text and keep both views
                anchored to the same document state. Hover previews and local
                graph context stay nearby while you work.
              </p>
            </div>
          </article>

          <article class="capability-card capability-card--cli">
            <div class="story-copy">
              <h3>Query, convert, and visualize from the CLI.</h3>
              <p>
                Use jq-style expressions for structured data work, then open a
                readonly local graph view when the result needs visual
                structure.
              </p>
            </div>
            <div class="cli-card">
              <div class="cli-card__row">
                <span class="cli-card__hint">Query</span>
                <code>treease '.services.api.url' example.json</code>
              </div>
              <div class="cli-card__row">
                <span class="cli-card__hint">Convert</span>
                <code>treease -o yaml '.' example.json</code>
              </div>
              <div class="cli-card__row">
                <span class="cli-card__hint">Visualize</span>
                <code>treease web '.services.api' example.json</code>
              </div>
            </div>
          </article>
        </div>
      </section>

      <section class="workflow-story" id="workflow" aria-labelledby="workflow-title">
        <div class="section-copy">
          <h2 id="workflow-title">Act on structure once it is clear.</h2>
          <p>
            Treease keeps import progress, conversion preview, and compare
            decisions inside the same visual workflow, so users can verify
            structure before exporting or trusting a diff.
          </p>
        </div>

        <div class="workflow-story__grid" role="region" aria-labelledby="workflow-title">
          <article class="workflow-story__card workflow-story__card--progress">
            <div class="story-copy">
              <h3>Keep large JSON imports transparent.</h3>
              <p>
                See visible progress while large JSON is parsed and rendered,
                instead of waiting on a blank screen.
              </p>
            </div>
            <div class="ship-media ship-media--dark">
              <video
                src={assetUrl(r2Assets.workflowProgressVideo)}
                poster={assetUrl(r2Assets.workflowProgressPoster)}
                autoplay
                muted
                loop
                playsinline
                preload="auto"
                aria-label="Streaming graph progress while importing a 2MB JSON file in Treease."
              ></video>
            </div>
          </article>

          <article class="workflow-story__card workflow-story__card--export">
            <div class="story-copy">
              <h3>Preview converted output before download.</h3>
              <p>
                Confirm the target format in the same workspace before the file
                leaves the browser.
              </p>
            </div>
            <div class="ship-media ship-media--contain">
              <img
                src={assetUrl(r2Assets.workflowExport)}
                alt="Previewing converted YAML output before export in Treease."
                loading="lazy"
              />
            </div>
          </article>

          <article class="workflow-story__card workflow-story__card--compare">
            <div class="story-copy">
              <h3>Compare structure before you trust the diff.</h3>
              <p>
                Prefer structural comparison first. Fall back to text diff only
                when structure cannot be compared safely.
              </p>
            </div>
          </article>
        </div>
      </section>

      <section class="pricing-section" id="pricing" aria-labelledby="pricing-title">
        <div class="section-copy pricing-intro">
          <p class="section-kicker">Pricing</p>
          <h2 id="pricing-title">{pricingConfig.title}</h2>
          <p>{pricingConfig.description}</p>
        </div>

        <div class="pricing-grid">
          {#each pricingConfig.plans as plan}
            <article
              class:pricing-card--featured={plan.featured}
              class:pricing-card--free={plan.id === 'free'}
              class="pricing-card"
            >
              {#if plan.billingPriceId === 'yearly'}
                <p class="pricing-card__eyebrow" aria-live="polite" aria-atomic="true">
                  SAVE {#if yearlySavings === null}<span class="pricing-card__loading-dash" aria-hidden="true">-</span><span class="sr-only">Loading annual saving</span>{:else}{yearlySavings}{/if}%
                </p>
              {:else if plan.eyebrow}
                <p class="pricing-card__eyebrow">{plan.eyebrow}</p>
              {/if}
              <div class="pricing-card__heading">
                <h3>{plan.name}</h3>
                <p class="pricing-card__description">{plan.description}</p>
              </div>
              <div class="pricing-card__price">
                <strong>{plan.price}</strong>
                <span>{plan.cadence}</span>
              </div>
              {#if plan.billingPriceId}
                {@const actionLabel = planActionLabel(plan, currentSubscription, subscriptionLookupState, planChangeBusy, scheduledPlanChange)}
                {@const actionTooltip = planActionTooltip(plan, currentSubscription, subscriptionLookupState, scheduledPlanChange)}
                <span
                  class="pricing-card__cta-wrap"
                  class:pricing-card__cta-wrap--tooltip={actionTooltip !== null}
                  data-tooltip={actionTooltip ?? undefined}
                >
                  <button
                    type="button"
                    class="pricing-card__cta"
                    disabled={checkoutBusy || planChangeBusy !== null || scheduledPlanChange === plan.billingPriceId || (subscriptionLookupState === 'ready' && currentSubscription?.billingCadence === plan.billingPriceId && currentPlanIsPaid() && !billingNeedsManagement())}
                    on:click={() => handlePlanAction(plan.billingPriceId!)}
                  >{actionLabel}</button>
                </span>
              {:else}
                <a class="pricing-card__cta" href={plan.ctaHref}>{plan.ctaLabel}</a>
              {/if}
              <ul class="pricing-card__features">
                {#each plan.features as feature}
                  {@const [prefix, emphasis, suffix] = splitFeatureLabel(feature)}
                  <li class:pricing-card__feature--without-check={feature.showCheck === false}>
                    {prefix}{#if emphasis}<mark class="pricing-card__feature-emphasis">{emphasis}</mark>{/if}{suffix}
                  </li>
                {/each}
              </ul>
            </article>
          {/each}
        </div>
        {#if planChangeNotice}
          <p class="pricing-change-notice" role="status">{planChangeNotice}</p>
        {/if}
      </section>

      <section class="faq-section" id="faq" aria-labelledby="faq-title">
        <div class="section-copy section-copy--compact">
          <h2 id="faq-title">FAQ</h2>
          <p>
            Treease works best when the page is explicit about what it does
            well, and what still belongs outside the graph view.
          </p>
        </div>

        <div class="faq-list">
          {#each faqItems as item}
            <details class="faq-item">
              <summary>{item.question}</summary>
              <p>{item.answer}</p>
            </details>
          {/each}
        </div>
      </section>

      <section class="cta-section" aria-labelledby="cta-title">
        <h2 id="cta-title">Try it with a real file.</h2>
        <p>
          Open a real document, see the structure, and keep every next step
          anchored to what you already understand.
        </p>
        <div class="hero-actions hero-actions--centered">
          <a class="primary-cta" href="/editor">Open Editor</a>
        </div>
      </section>
    </main>

    <SiteFooter />
  </div>
  {#if pendingPlanChange}
    <Dialog bind:open={planChangeDialogOpen}>
      <DialogContent aria-labelledby="plan-change-dialog-title" aria-describedby="plan-change-dialog-description" data-testid="plan-change-dialog" class="plan-change-dialog">
        <DialogHeader>
          <DialogTitle id="plan-change-dialog-title">Change your billing plan?</DialogTitle>
          <DialogDescription id="plan-change-dialog-description">
            Your Pro access stays active. The new billing plan starts at your next renewal.
          </DialogDescription>
        </DialogHeader>

        <div class="plan-change-summary">
          <div class="plan-change-summary__row">
            <span>Current plan</span>
            <strong>Pro {currentSubscription?.billingCadence ?? 'monthly'}</strong>
          </div>
          <div class="plan-change-summary__arrow" aria-hidden="true">↓</div>
          <div class="plan-change-summary__row">
            <span>New plan</span>
            <strong>{pendingPlanChange.plan.name}</strong>
          </div>
          <div class="plan-change-summary__row">
            <span>Next renewal</span>
            <strong>
              {planChangeChargesImmediately(pendingPlanChange.priceId) ? formatTodayDate() : formatRenewalDate()}
            </strong>
          </div>
          {#if planChangeChargesImmediately(pendingPlanChange.priceId)}
            <div class="plan-change-summary__row">
              <span>Payment today</span>
              <strong>Prorated amount</strong>
            </div>
          {/if}
          <div class="plan-change-summary__row">
            <span>Next charge</span>
            <strong>{formatPlanPrice(pendingPlanChange.priceId)}</strong>
          </div>
        </div>

        <p class="plan-change-dialog__note">
          {#if planChangeChargesImmediately(pendingPlanChange.priceId)}
            Your saved payment method will be charged the prorated difference today. The plan change takes effect after payment succeeds.
          {:else}
            There is no charge today. Your current plan remains active until the renewal date above, then the new plan price will apply.
          {/if}
        </p>

        <div class="plan-change-dialog__actions">
          <button type="button" class="plan-change-dialog__cancel" on:click={() => (planChangeDialogOpen = false)}>
            Keep current plan
          </button>
          <button type="button" class="plan-change-dialog__confirm" disabled={planChangeBusy !== null} on:click={confirmPlanChange}>
            {planChangeChargesImmediately(pendingPlanChange.priceId) ? 'Confirm and pay' : 'Confirm change'}
          </button>
        </div>
      </DialogContent>
    </Dialog>
  {/if}
  <LoginDialog bind:open={loginOpen} />
</div>

<style>
  .landing {
    --accent: #2d63e2;
    --accent-strong: #1745b5;
    --accent-soft: rgba(45, 99, 226, 0.1);
    --accent-tint: rgba(124, 160, 255, 0.18);
    --bg: #f4f7fb;
    --bg-strong: #fbfdff;
    --bg-muted: #e9eef6;
    --surface: rgba(255, 255, 255, 0.92);
    --surface-strong: #ffffff;
    --surface-muted: #f0f4fa;
    --surface-dark: #0f172a;
    --ink: #10192a;
    --muted: #536273;
    --muted-soft: #718196;
    --line: rgba(16, 25, 42, 0.1);
    --line-strong: rgba(45, 99, 226, 0.2);
    --shadow: 0 26px 60px rgba(18, 29, 46, 0.08);
    --font-sans: "SF Pro Text", "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", "Segoe UI", sans-serif;
    --font-display: "Avenir Next", "SF Pro Display", "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", sans-serif;
    --font-mono: "SF Mono", "SFMono-Regular", Menlo, Consolas, monospace;

    position: relative;
    isolation: isolate;
    min-height: 100svh;
    background:
      radial-gradient(circle at 100% 0%, rgba(45, 99, 226, 0.08), transparent 24%),
      radial-gradient(circle at 0% 18%, rgba(125, 160, 255, 0.1), transparent 24%),
      linear-gradient(180deg, var(--bg-strong) 0%, var(--bg) 100%);
    color: var(--ink);
    font-family: var(--font-sans);
  }

  .landing::before {
    content: '';
    position: absolute;
    inset: 0;
    pointer-events: none;
    background:
      linear-gradient(90deg, rgba(16, 25, 42, 0.035) 1px, transparent 1px),
      linear-gradient(rgba(16, 25, 42, 0.035) 1px, transparent 1px);
    background-size: 88px 88px;
    mask-image: linear-gradient(180deg, rgba(0, 0, 0, 0.55), transparent 72%);
  }

  .landing-shell {
    box-sizing: border-box;
    position: relative;
    z-index: 1;
    width: min(1220px, 100%);
    margin: 0 auto;
    padding: 28px 24px 72px;
  }

  .primary-cta,
  .secondary-cta {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 48px;
    padding: 0 22px;
    border-radius: 999px;
    font-size: 15px;
    font-weight: 700;
    text-decoration: none;
    transition:
      transform 140ms ease,
      border-color 160ms ease,
      background-color 160ms ease,
      color 160ms ease,
      box-shadow 160ms ease;
  }

  .primary-cta {
    background: linear-gradient(135deg, var(--accent) 0%, #4a7dff 100%);
    color: #fff;
    box-shadow: 0 16px 30px rgba(45, 99, 226, 0.22);
  }

  .secondary-cta {
    border: 1px solid var(--line);
    background: rgba(255, 255, 255, 0.88);
    color: var(--ink);
    box-shadow: 0 10px 24px rgba(15, 23, 42, 0.06);
  }

  .primary-cta:hover {
    transform: translateY(-1px);
    box-shadow: 0 18px 34px rgba(45, 99, 226, 0.28);
  }

  .secondary-cta:hover {
    border-color: rgba(45, 99, 226, 0.32);
    background: #ffffff;
  }

  .primary-cta:active,
  .secondary-cta:active {
    transform: translateY(1px) scale(0.99);
  }

  .landing-main {
    display: flex;
    flex-direction: column;
    gap: 72px;
  }

  .hero {
    display: grid;
    align-items: center;
    gap: clamp(32px, 5vw, 72px);
    grid-template-columns: minmax(0, 0.84fr) minmax(420px, 1.16fr);
    min-height: min(620px, calc(100dvh - 168px));
    padding-top: 0;
  }

  .hero-copy {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 18px;
    max-width: 520px;
    padding-top: 0;
  }

  .hero-kicker {
    margin: 0;
    color: var(--accent-strong);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
  }

  h1,
  h2,
  h3 {
    margin: 0;
    font-family: var(--font-display);
    font-weight: 700;
    letter-spacing: -0.04em;
    text-wrap: balance;
  }

  h1 {
    max-width: 8.4ch;
    font-size: clamp(3.45rem, 7vw, 5.9rem);
    line-height: 0.9;
  }

  h2 {
    max-width: 11ch;
    font-size: clamp(2.1rem, 4vw, 3.2rem);
    line-height: 0.98;
  }

  h3 {
    font-size: clamp(1.28rem, 2.2vw, 1.72rem);
    line-height: 1.08;
  }

  .hero-lede,
  .section-copy p,
  .story-copy p,
  .value-point p,
  .cta-section p,
  .faq-item p {
    color: var(--muted);
    font-size: 16px;
    line-height: 1.75;
    text-wrap: pretty;
  }

  .hero-lede {
    max-width: 38ch;
    margin: 0;
    font-size: 18px;
  }

  .hero-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
  }

  .hero-cta-stack {
    display: grid;
    gap: 14px;
    width: min(100%, 640px);
  }

  .cli-quickstart {
    display: grid;
    gap: 14px;
    padding: 18px 20px;
    border: 1px solid var(--line);
    border-radius: 20px;
    background: #fff;
    box-shadow: 0 18px 40px rgba(15, 23, 42, 0.06);
  }

  .cli-quickstart__label {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .cli-quickstart__label strong {
    font-size: 15px;
    line-height: 1.45;
  }

  .cli-card__hint {
    color: var(--muted-soft);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .cli-quickstart__commands,
  .cli-card {
    display: grid;
    gap: 10px;
  }

  .cli-quickstart code,
  .cli-card code {
    font-family: var(--font-mono);
    display: block;
    overflow-x: auto;
    padding: 12px 14px;
    border: 1px solid rgba(15, 23, 42, 0.08);
    border-radius: 14px;
    background: rgba(15, 23, 42, 0.94);
    color: #dbeafe;
    font-size: 13px;
    line-height: 1.45;
    white-space: nowrap;
  }

  .hero-demo {
    min-width: 0;
    position: relative;
    padding: 0;
  }

  .section-copy {
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-width: 64ch;
  }

  .section-copy--compact {
    max-width: 58ch;
  }

  .section-kicker,
  .pricing-card__eyebrow {
    margin: 0;
    color: var(--accent-strong);
    font-size: 12px;
    font-weight: 800;
    letter-spacing: 0.14em;
    text-transform: uppercase;
  }

  .value-strip,
  .capabilities-section,
  .workflow-story,
  .pricing-section,
  .faq-section,
  .cta-section {
    padding-top: 6px;
  }

  .value-strip {
    display: grid;
    gap: 24px;
    grid-template-columns: minmax(0, 0.95fr) minmax(0, 1.05fr);
    align-items: start;
    padding-top: 24px;
    border-top: 1px solid var(--line);
  }

  .value-points {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 14px;
  }

  .value-point,
  .capability-card,
  .workflow-story__card,
  .faq-item,
  .cta-section {
    border: 1px solid var(--line);
    border-radius: 22px;
    background: var(--surface);
    box-shadow: var(--shadow);
  }

  .value-point {
    padding: 20px 20px 18px;
    background: linear-gradient(180deg, #ffffff 0%, #f7faff 100%);
  }

  .value-point p,
  .story-copy p,
  .faq-item p {
    margin: 0;
  }

  .capability-grid {
    display: grid;
    gap: 18px;
    grid-template-columns: repeat(12, minmax(0, 1fr));
    margin-top: 34px;
  }

  .capability-card {
    display: flex;
    flex-direction: column;
    gap: 18px;
    min-width: 0;
    padding: 24px;
  }

  .capability-card--hero {
    grid-column: span 7;
    grid-row: span 2;
    background:
      linear-gradient(180deg, #ffffff 0%, #f3f7ff 100%);
  }

  .capability-card--soft {
    grid-column: span 5;
    background: #fbfcff;
  }

  .capability-card--surface {
    grid-column: span 5;
    background: linear-gradient(180deg, #f6f9fd 0%, #eef3fa 100%);
  }

  .capability-card--dark {
    grid-column: span 7;
    background:
      linear-gradient(180deg, #16233a 0%, #0f172a 100%);
    color: #f8fafc;
  }

  .capability-card--dark .story-copy p {
    color: rgba(226, 232, 240, 0.84);
  }

  .capability-card--copy {
    grid-column: span 4;
    align-self: start;
    justify-content: center;
    min-height: 100%;
    padding-top: 34px;
    background:
      radial-gradient(circle at top right, rgba(96, 165, 250, 0.14), transparent 26%),
      #fcfdff;
  }

  .capability-card--cli {
    grid-column: span 8;
    background: linear-gradient(180deg, #ffffff 0%, #f2f6fd 100%);
  }

  .capability-card--copy h3,
  .workflow-story__card--compare h3 {
    max-width: 12ch;
  }

  .capability-card--copy .story-copy,
  .workflow-story__card--compare .story-copy {
    gap: 14px;
  }

  .story-copy,
  .workflow-story__card .story-copy {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .cli-card {
    margin-top: auto;
    padding: 18px;
    border: 1px solid rgba(16, 25, 42, 0.08);
    border-radius: 18px;
    background: #eef3fa;
  }

  .cli-card__row {
    display: grid;
    gap: 8px;
  }

  .story-media {
    position: relative;
    overflow: hidden;
    min-height: 228px;
    border-radius: 18px;
    border: 1px solid rgba(16, 25, 42, 0.08);
    background: #eff4fb;
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.5);
  }

  .story-media--dark {
    border-color: rgba(148, 163, 184, 0.18);
    background:
      linear-gradient(180deg, rgba(15, 23, 42, 0.92), rgba(30, 41, 59, 0.88));
  }

  .story-media img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: top left;
  }

  .story-media--contain img {
    object-fit: contain;
    background: transparent;
  }

  .story-media--wide-short {
    min-height: 0;
    aspect-ratio: 2.1 / 1;
  }

  .workflow-story__grid {
    display: grid;
    gap: 20px;
    grid-template-columns: repeat(12, minmax(0, 1fr));
    margin-top: 34px;
  }

  .workflow-story__card {
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding: 0;
    border: 0;
    border-radius: 0;
    background: transparent;
    box-shadow: none;
  }

  .workflow-story__card--progress {
    grid-column: span 7;
    grid-row: span 2;
  }

  .workflow-story__card--export {
    grid-column: span 5;
  }

  .workflow-story__card--compare {
    grid-column: span 5;
    justify-content: center;
    min-height: 240px;
    padding: 30px 0 0 22px;
    border-left: 1px solid rgba(45, 99, 226, 0.2);
  }

  .workflow-story__card--progress .story-copy,
  .workflow-story__card--export .story-copy {
    padding-bottom: 14px;
  }

  .ship-media {
    position: relative;
    overflow: hidden;
    min-height: 208px;
    border-radius: 18px;
    border: 1px solid rgba(16, 25, 42, 0.08);
    background: #eef3fa;
    box-shadow: 0 20px 44px rgba(16, 25, 42, 0.08);
  }

  .ship-media--dark {
    border-color: rgba(148, 163, 184, 0.18);
    background:
      linear-gradient(180deg, rgba(15, 23, 42, 0.92), rgba(30, 41, 59, 0.88));
  }

  .ship-media img,
  .ship-media video {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: top left;
  }

  .ship-media--contain {
    min-height: 152px;
  }

  .ship-media--contain img {
    object-fit: contain;
  }

  .workflow-story__card--progress .ship-media {
    min-height: 420px;
    background: linear-gradient(180deg, #f7faff 0%, #edf3fb 100%);
  }

  .workflow-story__card--export .ship-media {
    min-height: 244px;
    background: linear-gradient(180deg, #ffffff 0%, #f2f6fd 100%);
  }

  .pricing-section {
    display: grid;
    gap: 30px;
  }

  .pricing-intro {
    max-width: 56ch;
  }

  .pricing-grid {
    display: grid;
    align-items: stretch;
    gap: 18px;
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }

  .pricing-card {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 22px;
    min-width: 0;
    padding: 30px 26px 26px;
    border: 1px solid var(--line);
    border-radius: 24px;
    background: rgba(255, 255, 255, 0.8);
    box-shadow: var(--shadow);
  }

  .pricing-card--featured {
    border-color: rgba(45, 99, 226, 0.42);
    background:
      radial-gradient(circle at 100% 0%, rgba(93, 143, 255, 0.24), transparent 34%),
      linear-gradient(180deg, #1a315f 0%, #102044 100%);
    color: #f8fbff;
    box-shadow: 0 28px 60px rgba(24, 61, 135, 0.22);
    transform: translateY(-10px);
  }

  .pricing-card--featured .pricing-card__eyebrow {
    color: #a9c6ff;
  }

  .pricing-card__heading {
    display: grid;
    gap: 10px;
  }

  .pricing-card__heading h3 {
    font-size: 28px;
  }

  .pricing-card__description {
    min-height: 50px;
    margin: 0;
    color: var(--muted);
    font-size: 14px;
    line-height: 1.6;
  }

  .pricing-card--featured .pricing-card__description {
    color: rgba(226, 232, 240, 0.8);
  }

  .pricing-card__price {
    display: flex;
    align-items: baseline;
    gap: 6px;
    padding: 16px 0;
    border-top: 1px solid var(--line);
    border-bottom: 1px solid var(--line);
  }

  .pricing-card--featured .pricing-card__price {
    border-color: rgba(191, 219, 254, 0.2);
  }

  .pricing-card__price strong {
    font-family: var(--font-display);
    font-size: clamp(2.2rem, 4vw, 3rem);
    letter-spacing: -0.06em;
  }

  .pricing-card__price span {
    color: var(--muted-soft);
    font-size: 14px;
  }

  .pricing-card--featured .pricing-card__price span {
    color: #abc0e3;
  }

  .pricing-card__cta {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 46px;
    border: 1px solid rgba(45, 99, 226, 0.3);
    border-radius: 999px;
    color: var(--accent-strong);
    background: rgba(255, 255, 255, 0.76);
    cursor: pointer;
    font-size: 14px;
    font-weight: 800;
    text-decoration: none;
    box-shadow: 0 4px 0 rgba(45, 99, 226, 0.08);
    transition: transform 180ms cubic-bezier(0.22, 1, 0.36, 1), box-shadow 180ms ease,
      background-color 180ms ease, border-color 180ms ease;
  }

  .pricing-card__cta:not(:disabled):hover {
    transform: translateY(-4px) scale(1.02);
    background: #ffffff;
    border-color: rgba(45, 99, 226, 0.62);
    box-shadow: 0 12px 22px rgba(45, 99, 226, 0.2), 0 3px 0 rgba(45, 99, 226, 0.14);
  }

  .pricing-card__cta:disabled {
    cursor: default;
    opacity: 0.72;
    transform: none;
    color: var(--muted-soft);
    background: #e8edf5;
    border-color: #d6dfef;
    box-shadow: none;
  }

  .pricing-card--featured .pricing-card__cta {
    border-color: transparent;
    color: #173b8e;
    background: #dbeafe;
  }

  .pricing-card--featured .pricing-card__cta:disabled {
    color: var(--muted-soft);
    background: #e8edf5;
    border-color: #d6dfef;
  }

  .pricing-change-notice {
    margin: 18px auto 0;
    max-width: 720px;
    color: var(--muted);
    font-size: 14px;
    line-height: 1.6;
    text-align: center;
  }

  .pricing-card__cta-wrap {
    position: relative;
    display: flex;
    width: 100%;
  }

  .pricing-card__cta-wrap .pricing-card__cta {
    width: 100%;
  }

  .pricing-card__cta-wrap--tooltip::after {
    position: absolute;
    z-index: 3;
    right: 0;
    bottom: calc(100% + 10px);
    left: 0;
    padding: 9px 12px;
    border: 1px solid rgba(45, 99, 226, 0.25);
    border-radius: 10px;
    color: #eaf2ff;
    background: #142a53;
    box-shadow: 0 10px 24px rgba(15, 32, 68, 0.2);
    content: attr(data-tooltip);
    font-size: 12px;
    font-weight: 600;
    line-height: 1.45;
    opacity: 0;
    pointer-events: none;
    text-align: center;
    transform: translateY(4px);
    transition: opacity 150ms ease, transform 150ms ease;
  }

  .pricing-card__cta-wrap--tooltip:hover::after,
  .pricing-card__cta-wrap--tooltip:focus-visible::after,
  .pricing-card__cta-wrap--tooltip:focus-within::after {
    opacity: 1;
    transform: translateY(0);
  }

  :global(.plan-change-dialog) {
    max-width: 440px;
    gap: 20px;
    border-color: #dce5f0;
    background: #ffffff;
    color: #10192a;
  }

  :global(.plan-change-summary) {
    display: grid;
    gap: 11px;
    padding: 16px;
    border: 1px solid #dce5f0;
    border-radius: 14px;
    background: #f0f4fa;
  }

  :global(.plan-change-summary__row) {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 20px;
    color: #536273;
    font-size: 13px;
  }

  :global(.plan-change-summary__row strong) {
    color: #10192a;
    font-size: 14px;
    text-align: right;
  }

  :global(.plan-change-summary__arrow) {
    color: #1745b5;
    font-size: 18px;
    line-height: 1;
    text-align: center;
  }

  :global(.plan-change-dialog__note) {
    margin: 0;
    color: #536273;
    font-size: 13px;
    line-height: 1.6;
  }

  :global(.plan-change-dialog__actions) {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }

  :global(.plan-change-dialog__cancel),
  :global(.plan-change-dialog__confirm) {
    min-height: 40px;
    padding: 0 16px;
    border-radius: 999px;
    font-size: 13px;
    font-weight: 800;
    transition: transform 160ms ease, background-color 160ms ease, border-color 160ms ease;
  }

  :global(.plan-change-dialog__cancel) {
    border: 1px solid #dce5f0;
    color: #536273;
    background: transparent;
  }

  :global(.plan-change-dialog__confirm) {
    border: 1px solid #1745b5;
    color: #ffffff;
    background: #1745b5;
  }

  :global(.plan-change-dialog__cancel:hover),
  :global(.plan-change-dialog__confirm:not(:disabled):hover) {
    transform: translateY(-1px);
  }

  :global(.plan-change-dialog__cancel:hover) {
    border-color: #2d63e2;
    background: #f0f4fa;
  }

  :global(.plan-change-dialog__confirm:not(:disabled):hover) {
    background: #2d63e2;
  }

  :global(.plan-change-dialog__confirm:disabled) {
    cursor: default;
    opacity: 0.65;
  }

  .pricing-card__features {
    display: grid;
    gap: 13px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .pricing-card__features li {
    position: relative;
    padding-left: 24px;
    color: var(--muted);
    font-size: 14px;
    line-height: 1.45;
  }

  .pricing-card__features li::before {
    content: '✓';
    position: absolute;
    left: 0;
    color: var(--accent-strong);
    font-weight: 900;
  }

  .pricing-card__features .pricing-card__feature--without-check::before {
    content: none;
  }

  .pricing-card__feature-emphasis {
    padding: 1px 3px;
    border-radius: 3px;
    color: var(--accent-strong);
    background: rgba(45, 99, 226, 0.12);
    font-weight: 700;
  }

  .pricing-card--free .pricing-card__feature-emphasis {
    color: inherit;
    font-weight: inherit;
  }

  .pricing-card--featured .pricing-card__features li {
    color: rgba(226, 232, 240, 0.88);
  }

  .pricing-card--featured .pricing-card__features li::before {
    color: #93c5fd;
  }

  .pricing-card--featured .pricing-card__feature-emphasis {
    color: #dbeafe;
    background: rgba(147, 197, 253, 0.18);
  }

  .faq-list {
    display: grid;
    gap: 0;
    margin-top: 28px;
    border-top: 1px solid var(--line);
  }

  .faq-item {
    padding: 22px 0;
    border: 0;
    border-bottom: 1px solid var(--line);
    border-radius: 0;
    background: transparent;
    box-shadow: none;
  }

  .faq-item summary {
    cursor: pointer;
    list-style: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    color: var(--ink);
    font-size: 18px;
    font-weight: 700;
    line-height: 1.4;
  }

  .faq-item summary::after {
    content: '+';
    flex: 0 0 auto;
    color: var(--accent-strong);
    font-size: 24px;
    line-height: 1;
  }

  .faq-item[open] summary::after {
    content: '−';
  }

  .faq-item summary::-webkit-details-marker {
    display: none;
  }

  .faq-item p {
    max-width: 74ch;
    margin-top: 14px;
  }

  .cta-section {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    padding: 54px 28px;
    text-align: center;
    background:
      linear-gradient(135deg, rgba(45, 99, 226, 0.14), rgba(255, 255, 255, 0.95));
  }

  .cta-section p {
    max-width: 44ch;
    margin: 0;
  }

  .hero-actions--centered {
    justify-content: center;
  }

  .hero-demo :global(.deck-stage) {
    position: relative;
    z-index: 1;
    filter: drop-shadow(0 18px 34px rgba(16, 25, 42, 0.12));
    transform: translateY(18px);
  }

  .primary-cta:focus-visible,
  .secondary-cta:focus-visible,
  .pricing-card__cta:focus-visible,
  .faq-item summary:focus-visible {
    outline: 2px solid var(--accent-strong);
    outline-offset: 3px;
  }

  @media (max-width: 1120px) {
    .hero,
    .value-strip,
    .capability-grid,
    .workflow-story__grid,
    .pricing-grid {
      grid-template-columns: 1fr;
    }

    .hero {
      min-height: auto;
    }

    .hero-demo {
      padding: 0;
    }

    .hero-demo :global(.deck-stage) {
      transform: none;
    }

    .workflow-story__card--progress {
      grid-row: auto;
    }

    .capability-card--hero {
      grid-row: auto;
    }

    .capability-card--hero,
    .capability-card--soft,
    .capability-card--surface,
    .capability-card--dark,
    .capability-card--copy,
    .capability-card--cli,
    .workflow-story__card--progress,
    .workflow-story__card--export,
    .workflow-story__card--compare {
      grid-column: span 12;
    }

    .workflow-story__card--compare {
      min-height: 0;
      padding: 18px 0 0;
      border-left: 0;
      border-top: 1px solid rgba(45, 99, 226, 0.2);
    }

    .pricing-card--featured {
      transform: none;
    }
  }

  @media (max-width: 860px) {
    .landing-shell {
      padding: 20px 18px 46px;
    }

    h1 {
      max-width: 12ch;
      font-size: clamp(2.8rem, 11vw, 4.3rem);
      line-height: 0.95;
    }

    h2 {
      font-size: clamp(1.9rem, 8vw, 2.6rem);
    }

    .value-points {
      grid-template-columns: 1fr;
    }

    .pricing-card__description {
      min-height: 0;
    }
  }

  @media (max-width: 640px) {
    .primary-cta,
    .secondary-cta {
      width: 100%;
    }

    .hero-actions {
      width: 100%;
      flex-direction: column;
    }

    .value-point,
    .capability-card,
    .workflow-story__card,
    .cta-section,
    .faq-item {
      padding: 20px;
      border-radius: 20px;
    }

    .story-media {
      min-height: 180px;
      border-radius: 18px;
    }

    .story-media--wide-short {
      min-height: 0;
      aspect-ratio: 2 / 1;
    }

    .ship-media {
      min-height: 180px;
      border-radius: 18px;
    }

    .ship-media--contain {
      min-height: 146px;
    }

    .workflow-story__card--progress .ship-media {
      min-height: 220px;
    }

    .workflow-story__card--export .ship-media {
      min-height: 180px;
    }

    .faq-item {
      padding-left: 0;
      padding-right: 0;
      border-radius: 0;
    }
  }

  @media (prefers-color-scheme: dark) {
    .landing {
      --accent: #60a5fa;
      --accent-strong: #93c5fd;
      --accent-soft: rgba(96, 165, 250, 0.16);
      --accent-tint: rgba(59, 130, 246, 0.2);
      --bg: #07111f;
      --bg-strong: #0b1526;
      --surface: rgba(13, 23, 38, 0.78);
      --surface-strong: rgba(15, 23, 42, 0.96);
      --surface-muted: rgba(18, 29, 47, 0.94);
      --surface-dark: #020617;
      --ink: #e5eefc;
      --muted: #b7c5dd;
      --muted-soft: #8ea4c4;
      --line: rgba(148, 163, 184, 0.18);
      --line-strong: rgba(96, 165, 250, 0.28);
      --shadow: 0 28px 70px rgba(2, 6, 23, 0.42);

      background:
        radial-gradient(circle at 10% 8%, rgba(14, 165, 233, 0.12), transparent 28%),
        radial-gradient(circle at 88% 14%, rgba(37, 99, 235, 0.16), transparent 34%),
        linear-gradient(180deg, var(--bg-strong) 0%, var(--bg) 100%);
    }

    .landing::before {
      background:
        linear-gradient(90deg, rgba(255, 255, 255, 0.045) 1px, transparent 1px),
        linear-gradient(rgba(255, 255, 255, 0.045) 1px, transparent 1px);
    }

    .value-point,
    .pricing-card:not(.pricing-card--featured),
    .capability-card--hero,
    .capability-card--soft,
    .capability-card--surface,
    .capability-card--copy,
    .capability-card--cli,
    .workflow-story__card--progress,
    .workflow-story__card--export,
    .workflow-story__card--compare,
    .cta-section {
      background: rgba(11, 20, 36, 0.82);
    }

    .story-media,
    .ship-media,
    .cli-card {
      background: rgba(15, 23, 42, 0.92);
    }

    .faq-item {
      background: transparent;
    }

    .cli-quickstart {
      background: rgba(11, 20, 36, 0.84);
    }

    .workflow-story__card--compare {
      border-left-color: rgba(147, 197, 253, 0.28);
      border-top-color: rgba(147, 197, 253, 0.28);
    }

    .workflow-story__card--progress .ship-media,
    .workflow-story__card--export .ship-media {
      background: rgba(11, 20, 36, 0.9);
      box-shadow: 0 24px 50px rgba(2, 6, 23, 0.24);
    }

    .cli-quickstart code,
    .cli-card code {
      border-color: rgba(148, 163, 184, 0.18);
      background: rgba(2, 6, 23, 0.92);
      color: #dbeafe;
    }

    .faq-item summary {
      color: var(--ink);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .primary-cta,
    .secondary-cta {
      transition: none;
    }
  }
</style>
