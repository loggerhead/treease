import { describe, expect, it, vi } from 'vitest';

import { BillingAuthenticationRequiredError } from '../services/treease-server';
import { prepareBillingCheckout, prewarmBillingCheckout, startBillingCheckout } from './checkout-flow';

const returnUrl = {
  successUrl: 'https://treease.com/editor',
};

describe('startBillingCheckout', () => {
  it('delegates authenticated checkout creation to the server client before opening the overlay', async () => {
    const createCheckoutLink = vi.fn().mockResolvedValue({
      priceId: 'monthly',
      url: 'https://billing.example.com/checkout/buy/monthly',
    });
    const openCheckout = vi.fn().mockResolvedValue(undefined);
    const preloadCheckout = vi.fn().mockResolvedValue(undefined);
    const prewarmPricing = vi.fn();

    await expect(startBillingCheckout('monthly', returnUrl, { createCheckoutLink, openCheckout, preloadCheckout, prewarmPricing })).resolves.toEqual({ status: 'opened' });

    expect(createCheckoutLink).toHaveBeenCalledWith('monthly', returnUrl);
    expect(preloadCheckout).toHaveBeenCalledOnce();
    expect(openCheckout).toHaveBeenCalledWith('https://billing.example.com/checkout/buy/monthly');
  });

  it('prepares the checkout URL and provider overlay before the user clicks', async () => {
    const createCheckoutLink = vi.fn().mockResolvedValue({
      priceId: 'yearly',
      url: 'https://billing.example.com/checkout/buy/yearly',
    });
    const preloadCheckout = vi.fn().mockResolvedValue(undefined);
    const openCheckout = vi.fn();
    const prewarmPricing = vi.fn();

    await expect(
      prepareBillingCheckout('yearly', returnUrl, { createCheckoutLink, openCheckout, preloadCheckout, prewarmPricing }),
    ).resolves.toEqual({ priceId: 'yearly', checkoutUrl: 'https://billing.example.com/checkout/buy/yearly' });

    expect(createCheckoutLink).toHaveBeenCalledWith('yearly', returnUrl);
    expect(preloadCheckout).toHaveBeenCalledOnce();
    expect(openCheckout).not.toHaveBeenCalled();
  });

  it('loads dynamic prices and both checkout links in one prewarm request', async () => {
    const prewarmPricing = vi.fn().mockResolvedValue({
      plans: [],
      checkouts: [],
    });
    const preloadCheckout = vi.fn().mockResolvedValue(undefined);
    const createCheckoutLink = vi.fn();
    const openCheckout = vi.fn();

    await expect(prewarmBillingCheckout(returnUrl, { createCheckoutLink, openCheckout, preloadCheckout, prewarmPricing })).resolves.toEqual({ plans: [], checkouts: [] });
    expect(prewarmPricing).toHaveBeenCalledWith(returnUrl);
    expect(preloadCheckout).toHaveBeenCalledOnce();
  });

  it('shows login only when the canonical server client cannot recover a session', async () => {
    const createCheckoutLink = vi.fn().mockRejectedValue(new BillingAuthenticationRequiredError());
    const openCheckout = vi.fn();
    const preloadCheckout = vi.fn().mockResolvedValue(undefined);
    const prewarmPricing = vi.fn();

    await expect(startBillingCheckout('yearly', returnUrl, { createCheckoutLink, openCheckout, preloadCheckout, prewarmPricing })).resolves.toEqual({ status: 'login-required' });

    expect(openCheckout).not.toHaveBeenCalled();
  });
});
