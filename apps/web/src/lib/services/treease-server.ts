import { getSupabaseClient, getSupabaseConfiguration } from '../auth/supabase-auth';
import type { BillingPriceId } from '../config/pricing';
import { workspaceHost } from '../workspace-host';
import { parseShareResource, type ShareResource } from '../share/share-resource';

export type { ShareResource } from '../share/share-resource';

export type ShareLink = {
  id: string;
  shareUrl: string;
  expiresAt: string;
  createdAt: string;
};

export type BillingCheckoutLink = {
  priceId: BillingPriceId;
  url: string;
};

export type BillingPlanPrice = {
  priceId: BillingPriceId;
  amount: number;
  currency: string;
  interval: 'day' | 'week' | 'month' | 'year';
  intervalCount: number;
};

export type BillingPricingPrewarm = {
  plans: BillingPlanPrice[];
  checkouts: BillingCheckoutLink[] | null;
};

export type CurrentSubscription = {
  id: string;
  userId: string;
  tier: 'free' | 'pro';
  billingCadence: 'monthly' | 'yearly' | null;
  status: 'active' | 'inactive' | 'past_due' | 'canceled';
  currentPeriodEnd: string | null;
  createdAt: string;
  updatedAt: string;
};

export type BillingPortalLink = {
  url: string;
};

export type UsageSummary = {
  tier: 'free' | 'pro';
  periodKey: string;
  limits: {
    bidirectionalEditDocumentsMonthly: { kind: 'limited'; limit: number } | { kind: 'unlimited' };
    largeFileProcessingRunsMonthly: { kind: 'limited'; limit: number } | { kind: 'unlimited' };
    aiSuggestionsMonthly: { kind: 'limited'; limit: number } | { kind: 'unlimited' };
    shareMaxAgeDays: number;
  };
  usage: Partial<Record<UsageCapability, number>>;
};

export type UsageCapability = 'bidirectional_edit' | 'large_file_processing' | 'ai_suggestion';

export type UsageReservation = {
  id: string;
  capability: UsageCapability;
  state: 'reserved' | 'consumed' | 'released';
};

export class TreeaseServerError extends Error {
  constructor(
    message: string,
    readonly status: number,
    readonly code: string | null,
    readonly details: Record<string, unknown> | null,
  ) {
    super(message);
    this.name = 'TreeaseServerError';
  }
}

export class BillingAuthenticationRequiredError extends Error {
  constructor() {
    super('请先登录 Treease，再继续购买。');
    this.name = 'BillingAuthenticationRequiredError';
  }
}

const apiOrigin = import.meta.env.PROD ? 'https://api.treease.com' : 'http://localhost:3000';

async function getAccessToken(): Promise<string | null> {
  const host = await workspaceHost;
  if (host.surface === 'desktop') {
    const { url, anonKey } = getSupabaseConfiguration();
    return host.refreshAccessToken(url, anonKey);
  }
  const { data, error } = await getSupabaseClient().auth.getSession();
  if (error) throw error;
  return data.session?.access_token ?? null;
}

async function readError(response: Response): Promise<TreeaseServerError> {
  let message = `Treease server request failed (${response.status})`;
  let code: string | null = null;
  let details: Record<string, unknown> | null = null;
  try {
    const body = (await response.json()) as { message?: string; error?: string; details?: unknown };
    message = body.message || body.error || message;
    code = body.error ?? null;
    details = body.details && typeof body.details === 'object' && !Array.isArray(body.details)
      ? body.details as Record<string, unknown>
      : null;
  } catch {
    // Keep the HTTP status when the server does not return JSON.
  }
  return new TreeaseServerError(message, response.status, code, details);
}

export async function createShareLink(resource: ShareResource, expiresInDays = 7): Promise<ShareLink> {
  const token = await getAccessToken();
  if (!token) throw new Error('Sign in to create a share link.');

  const response = await fetch(`${apiOrigin}/v1/share-links`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({ resource, expiresInDays }),
  });
  if (!response.ok) throw await readError(response);
  return (await response.json()) as ShareLink;
}

export async function createBillingCheckoutLink(
  priceId: BillingPriceId,
  returnUrl: { successUrl: string },
): Promise<BillingCheckoutLink> {
  const token = await getAccessToken();
  if (!token) throw new BillingAuthenticationRequiredError();

  const response = await fetch(`${apiOrigin}/v1/billing/checkout-link`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({ priceId, ...returnUrl }),
  });
  if (!response.ok) throw await readError(response);
  return (await response.json()) as BillingCheckoutLink;
}

export async function prewarmBillingPricing(
  returnUrl: { successUrl: string },
): Promise<BillingPricingPrewarm> {
  const token = await getAccessToken();
  const response = await fetch(`${apiOrigin}/v1/billing/pricing-prewarm`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      ...(token ? { authorization: `Bearer ${token}` } : {}),
    },
    body: JSON.stringify(returnUrl),
  });
  if (!response.ok) throw await readError(response);
  return (await response.json()) as BillingPricingPrewarm;
}

export async function getCurrentSubscription(): Promise<CurrentSubscription> {
  const token = await getAccessToken();
  if (!token) throw new BillingAuthenticationRequiredError();

  const response = await fetch(`${apiOrigin}/v1/billing/subscription`, {
    headers: { authorization: `Bearer ${token}` },
  });
  if (!response.ok) throw await readError(response);
  return (await response.json()) as CurrentSubscription;
}

export async function createBillingPortalLink(returnUrl: string): Promise<BillingPortalLink> {
  const token = await getAccessToken();
  if (!token) throw new BillingAuthenticationRequiredError();

  const response = await fetch(`${apiOrigin}/v1/billing/portal-link`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({ returnUrl }),
  });
  if (!response.ok) throw await readError(response);
  return (await response.json()) as BillingPortalLink;
}

export async function getUsageSummary(): Promise<UsageSummary> {
  const token = await getAccessToken();
  if (!token) throw new BillingAuthenticationRequiredError();
  const response = await fetch(`${apiOrigin}/v1/usage`, { headers: { authorization: `Bearer ${token}` } });
  if (!response.ok) throw await readError(response);
  return (await response.json()) as UsageSummary;
}

export async function createUsageReservation(input: {
  capability: UsageCapability;
  idempotencyKey: string;
  metadata?: Record<string, unknown>;
}): Promise<UsageReservation> {
  const token = await getAccessToken();
  if (!token) throw new BillingAuthenticationRequiredError();
  const response = await fetch(`${apiOrigin}/v1/usage/reservations`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      authorization: `Bearer ${token}`,
    },
    body: JSON.stringify(input),
  });
  if (!response.ok) throw await readError(response);
  return (await response.json()) as UsageReservation;
}

export async function finalizeUsageReservation(
  id: string,
  state: 'consumed' | 'released',
): Promise<void> {
  const token = await getAccessToken();
  if (!token) throw new BillingAuthenticationRequiredError();
  const response = await fetch(`${apiOrigin}/v1/usage/reservations/${encodeURIComponent(id)}`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({ state }),
  });
  if (!response.ok) throw await readError(response);
}

export type PublicShare = {
  resource: ShareResource;
};

export async function getPublicShare(shareID: string): Promise<PublicShare> {
  const response = await fetch(`${apiOrigin}/v1/public/shares/${encodeURIComponent(shareID)}`);
  if (!response.ok) throw await readError(response);
  const body = await response.json() as { resourceType?: unknown; resourcePayload?: unknown };
  const resource = parseShareResource({ type: body.resourceType, payload: body.resourcePayload });
  if (!resource) throw new Error('分享内容无效或已损坏。');
  return { resource };
}
