import type { BillingPriceId } from '../config/pricing';
import {
  BillingAuthenticationRequiredError,
  createBillingCheckoutLink,
  prewarmBillingPricing,
  type BillingCheckoutLink,
  type BillingPricingPrewarm,
} from '../services/treease-server';
import { openLemonSqueezyCheckout, preloadLemonSqueezyCheckout } from './lemon-squeezy-checkout';

export type CheckoutReturnUrl = {
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

export type BillingCheckoutActionOptions = {
  priceId: BillingPriceId;
  returnUrl: CheckoutReturnUrl;
  prepared?: PreparedBillingCheckout | Promise<PreparedBillingCheckout | null> | null;
  onLoginRequired?: () => void;
  onFailed?: (message: string) => void;
};

function preloadCheckoutInBackground(dependencies: CheckoutDependencies): void {
  void dependencies.preloadCheckout().catch(() => {});
}

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
  const pricing = await dependencies.prewarmPricing(returnUrl);
  preloadCheckoutInBackground(dependencies);
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
  const checkout = await dependencies.createCheckoutLink(priceId, returnUrl);
  preloadCheckoutInBackground(dependencies);
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

/** Runs the shared checkout action for UI surfaces that may have a prepared URL. */
export async function runBillingCheckout(options: BillingCheckoutActionOptions): Promise<CheckoutStartOutcome> {
  let outcome: CheckoutStartOutcome;
  try {
    const prepared = await options.prepared;
    outcome = prepared
      ? await openPreparedBillingCheckout(prepared)
      : await startBillingCheckout(options.priceId, options.returnUrl);
  } catch {
    outcome = await startBillingCheckout(options.priceId, options.returnUrl);
  }

  if (outcome.status === 'login-required') options.onLoginRequired?.();
  if (outcome.status === 'failed') options.onFailed?.(outcome.message);
  return outcome;
}
