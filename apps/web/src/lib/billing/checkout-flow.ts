import type { BillingPriceId } from '../config/pricing';
import {
  BillingAuthenticationRequiredError,
  createBillingCheckoutLink,
  prewarmBillingPricing,
  type BillingCheckoutLink,
  type BillingPricingPrewarm,
} from '../services/treease-server';
import { openLemonSqueezyCheckout, preloadLemonSqueezyCheckout } from './lemon-squeezy-checkout';

type CheckoutReturnUrl = {
  successUrl: string;
};

type CheckoutDependencies = {
  createCheckoutLink(priceId: BillingPriceId, returnUrl: CheckoutReturnUrl): Promise<BillingCheckoutLink>;
  openCheckout(checkoutUrl: string): Promise<void>;
  preloadCheckout(): Promise<void>;
  prewarmPricing(returnUrl: CheckoutReturnUrl): Promise<BillingPricingPrewarm>;
};

export type PreparedBillingCheckout = {
  priceId: BillingPriceId;
  checkoutUrl: string;
};

export type CheckoutStartOutcome =
  | { status: 'opened' }
  | { status: 'login-required' }
  | { status: 'failed'; message: string };

const defaultDependencies: CheckoutDependencies = {
  createCheckoutLink: createBillingCheckoutLink,
  openCheckout: openLemonSqueezyCheckout,
  preloadCheckout: preloadLemonSqueezyCheckout,
  prewarmPricing: prewarmBillingPricing,
};

export async function prewarmBillingCheckout(
  returnUrl: CheckoutReturnUrl,
  dependencies = defaultDependencies,
): Promise<BillingPricingPrewarm> {
  const [pricing] = await Promise.all([dependencies.prewarmPricing(returnUrl), dependencies.preloadCheckout()]);
  return pricing;
}

function failedCheckout(cause: unknown): CheckoutStartOutcome {
  if (cause instanceof BillingAuthenticationRequiredError) return { status: 'login-required' };
  return {
    status: 'failed',
    message: cause instanceof Error ? cause.message : 'Unable to start checkout.',
  };
}

/** Prepares the server-issued URL and the Lemon overlay in parallel. */
export async function prepareBillingCheckout(
  priceId: BillingPriceId,
  returnUrl: CheckoutReturnUrl,
  dependencies = defaultDependencies,
): Promise<PreparedBillingCheckout> {
  const [checkout] = await Promise.all([
    dependencies.createCheckoutLink(priceId, returnUrl),
    dependencies.preloadCheckout(),
  ]);
  return { priceId, checkoutUrl: checkout.url };
}

export async function openPreparedBillingCheckout(
  checkout: PreparedBillingCheckout,
  dependencies = defaultDependencies,
): Promise<CheckoutStartOutcome> {
  try {
    await dependencies.openCheckout(checkout.checkoutUrl);
    return { status: 'opened' };
  } catch (cause) {
    return failedCheckout(cause);
  }
}

// The server client owns browser/desktop session recovery; this flow only handles purchase UI state.
export async function startBillingCheckout(
  priceId: BillingPriceId,
  returnUrl: CheckoutReturnUrl,
  dependencies = defaultDependencies,
): Promise<CheckoutStartOutcome> {
  try {
    const checkout = await prepareBillingCheckout(priceId, returnUrl, dependencies);
    return await openPreparedBillingCheckout(checkout, dependencies);
  } catch (cause) {
    return failedCheckout(cause);
  }
}
