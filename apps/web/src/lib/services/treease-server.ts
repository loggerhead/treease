import type { ZodType } from 'zod';
import {
  accountSummarySchema,
  billingCheckoutLinkSchema,
  billingPortalLinkSchema,
  billingPricingPrewarmResponseSchema,
  currentSubscriptionSchema,
  errorResponseSchema,
  feedbackResponseSchema,
  publicShareResponseSchema,
  shareLinkSchema,
  structGenerationResponseSchema,
  suggestYqResponseSchema,
  usageSummarySchema,
  type AccountSummary,
  type BillingCheckoutLink,
  type BillingPricingPrewarm,
  type BillingPriceId,
  type BillingPortalLink,
  type CurrentSubscription,
  type RecordedUsageCapability,
  type ShareLink,
  type StructLanguage,
  type UsageSummary,
} from '@treease/api-contracts';
import { getSupabaseClient, getSupabaseConfiguration } from '../auth/supabase-auth';
import { workspaceHost } from '../workspace-host';
import { parseShareResource, type ShareResource } from '../share/share-resource';
import { isUsageCoolingDown, markUsageRequestSucceeded, noteUsageRateLimit } from '../billing/usage-rate-limit';
import { getUsageClientId } from '../billing/client-id';
import { captureFrontendException } from '../observability/sentry';

export type {
  AccountSummary,
  BillingCheckoutLink,
  BillingPricingPrewarm,
  BillingPortalLink,
  CurrentSubscription,
  RecordedUsageCapability,
  ShareLink,
  StructLanguage,
  UsageCapability,
  UsageSummary,
} from '@treease/api-contracts';
export type { ShareResource } from '../share/share-resource';

export class TreeaseServerError extends Error {
  constructor(
    message: string,
    readonly status: number,
    readonly code: string | null,
    readonly details: Record<string, unknown> | null,
    readonly retryAfterMs?: number,
    readonly requestId?: string,
  ) {
    super(message);
    this.name = 'TreeaseServerError';
  }
}

function retryAfterMs(response: Response): number | undefined {
  const value = response.headers.get('retry-after');
  if (!value) return undefined;
  const seconds = Number(value);
  if (Number.isFinite(seconds)) return Math.max(0, seconds * 1000);
  const timestamp = Date.parse(value);
  return Number.isNaN(timestamp) ? undefined : Math.max(0, timestamp - Date.now());
}

export class BillingAuthenticationRequiredError extends Error {
  constructor() {
    super('Please sign in to Treease before continuing with the purchase.');
    this.name = 'BillingAuthenticationRequiredError';
  }
}

const configuredApiOrigin = String(import.meta.env.PUBLIC_API_ORIGIN ?? '').trim();
const productionRuntime = import.meta.env.PROD || import.meta.env.SIMULATE_PROD;
const apiOrigin = configuredApiOrigin || (productionRuntime ? 'https://api.treease.com' : 'http://localhost:3000');

async function getAccessToken(): Promise<string | null> {
  const host = await workspaceHost;
  if (host.surface === 'desktop') {
    if (!(await host.hasRefreshToken())) return null;
    const { url, anonKey } = getSupabaseConfiguration();
    return (await host.refreshSession(url, anonKey)).accessToken;
  }
  const { data, error } = await getSupabaseClient().auth.getSession();
  if (error) throw error;
  return data.session?.access_token ?? null;
}

export async function readError(response: Response): Promise<TreeaseServerError> {
  let message = `Treease server request failed (${response.status})`;
  let code: string | null = null;
  let details: Record<string, unknown> | null = null;
  let requestId = response.headers.get('x-request-id') ?? undefined;
  try {
    const parsed = errorResponseSchema.safeParse(await response.json());
    if (parsed.success) {
      message = parsed.data.message || parsed.data.error || message;
      code = parsed.data.error ?? null;
      requestId = parsed.data.requestId ?? requestId;
      details = parsed.data.details && typeof parsed.data.details === 'object' && !Array.isArray(parsed.data.details)
        ? parsed.data.details as Record<string, unknown>
        : null;
    }
  } catch {
    // Keep the HTTP status when the server does not return JSON.
  }
  const error = new TreeaseServerError(message, response.status, code, details, retryAfterMs(response), requestId);
  if (response.status >= 500) {
    captureFrontendException(error, {
      route: response.url ? new URL(response.url).pathname : typeof window !== 'undefined' ? window.location.pathname : undefined,
      requestId,
      status: response.status,
      code,
    });
  }
  return error;
}

async function readJsonResponse<T>(response: Response, schema: ZodType<T>): Promise<T> {
  try {
    const parsed = schema.safeParse(await response.json());
    if (parsed.success) return parsed.data;
  } catch {
    // Treat invalid JSON and invalid schema output as the same protocol failure.
  }
  const error = new TreeaseServerError(
    'Treease server returned an invalid response.',
    response.status,
    'invalid_server_response',
    null,
    undefined,
    response.headers.get('x-request-id') ?? undefined,
  );
  captureFrontendException(error, {
    route: response.url ? new URL(response.url).pathname : typeof window !== 'undefined' ? window.location.pathname : undefined,
    requestId: error.requestId,
    status: response.status,
    code: error.code,
  });
  throw error;
}

function throwIfUsageCoolingDown(): void {
  if (isUsageCoolingDown()) throw new TreeaseServerError('Usage requests are temporarily paused.', 429, null, null);
}

async function readUsageResponse<T>(response: Response, schema: ZodType<T>): Promise<T> {
  if (!response.ok) {
    const error = await readError(response);
    if (error.status === 429) noteUsageRateLimit(error.retryAfterMs);
    throw error;
  }
  markUsageRequestSucceeded();
  return readJsonResponse(response, schema);
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
  return readJsonResponse(response, shareLinkSchema);
}

export async function suggestYq(input: {
  instruction: string;
  editorTextSnapshot?: string;
  treePathSet?: string[];
}): Promise<{ expression: string }> {
  const token = await getAccessToken();
  const clientId = await getUsageClientId();

  const response = await fetch(`${apiOrigin}/v1/ai/suggest-yq?clientId=${encodeURIComponent(clientId)}`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      ...(token ? { authorization: `Bearer ${token}` } : {}),
    },
    body: JSON.stringify(input),
  });
  if (!response.ok) throw await readError(response);
  return readJsonResponse(response, suggestYqResponseSchema);
}

export async function generateStruct(input: {
  sourceJson: string;
  targetLanguage: StructLanguage;
  rootName?: string;
}): Promise<{ language: StructLanguage; code: string }> {
  const token = await getAccessToken();
  if (!token) throw new Error('Sign in to generate a structure definition.');

  const response = await fetch(`${apiOrigin}/v1/codegen/struct`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', authorization: `Bearer ${token}` },
    body: JSON.stringify(input),
  });
  if (!response.ok) throw await readError(response);
  return readJsonResponse(response, structGenerationResponseSchema);
}

export async function submitFeedback(input: { category: 'bug' | 'feature' | 'question'; description: string; email: string | null; screenshot: string; consoleLogs: string }): Promise<string | undefined> {
  const token = await getAccessToken();
  const form = new FormData();
  form.set('metadata', JSON.stringify({ category: input.category, description: input.description, email: input.email }));
  if (input.screenshot) form.append('screenshot', dataUrlFile(input.screenshot, 'screenshot.png'));
  if (input.consoleLogs) form.append('console_logs', new File([input.consoleLogs], 'console-logs.txt', { type: 'text/plain' }));
  const response = await fetch(`${apiOrigin}/v1/feedback`, { method: 'POST', headers: token ? { authorization: `Bearer ${token}` } : {}, body: form });
  if (!response.ok) throw await readError(response);
  return (await readJsonResponse(response, feedbackResponseSchema)).issueUrl ?? undefined;
}

function dataUrlFile(value: string, name: string): File {
  const [header, encoded] = value.split(',', 2);
  const contentType = header.match(/^data:([^;]+);base64$/)?.[1];
  if (!contentType || !encoded) throw new Error('Invalid screenshot data.');
  const binary = atob(encoded);
  const bytes = Uint8Array.from(binary, (char) => char.charCodeAt(0));
  return new File([bytes], name, { type: contentType });
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
  return readJsonResponse(response, billingCheckoutLinkSchema);
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
  return readJsonResponse(response, billingPricingPrewarmResponseSchema);
}

export async function getCurrentSubscription(): Promise<CurrentSubscription> {
  const token = await getAccessToken();
  if (!token) throw new BillingAuthenticationRequiredError();

  const response = await fetch(`${apiOrigin}/v1/billing/subscription`, {
    headers: { authorization: `Bearer ${token}` },
  });
  if (!response.ok) throw await readError(response);
  return readJsonResponse(response, currentSubscriptionSchema);
}

export async function getAccountSummary(): Promise<AccountSummary> {
  const token = await getAccessToken();
  if (!token) throw new BillingAuthenticationRequiredError();
  const clientId = await getUsageClientId();
  const response = await fetch(`${apiOrigin}/v1/account?clientId=${encodeURIComponent(clientId)}`, {
    headers: { authorization: `Bearer ${token}` },
  });
  if (!response.ok) throw await readError(response);
  return readJsonResponse(response, accountSummarySchema);
}

export async function createBillingPortalLink(): Promise<BillingPortalLink> {
  const token = await getAccessToken();
  if (!token) throw new BillingAuthenticationRequiredError();

  const response = await fetch(`${apiOrigin}/v1/billing/portal-link`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({}),
  });
  if (!response.ok) throw await readError(response);
  return readJsonResponse(response, billingPortalLinkSchema);
}

export async function changeBillingPlan(priceId: BillingPriceId): Promise<CurrentSubscription> {
  const token = await getAccessToken();
  if (!token) throw new BillingAuthenticationRequiredError();

  const response = await fetch(`${apiOrigin}/v1/billing/change-plan`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({ priceId }),
  });
  if (!response.ok) throw await readError(response);
  return readJsonResponse(response, currentSubscriptionSchema);
}

export async function getUsageSummary(clientId?: string): Promise<UsageSummary> {
  throwIfUsageCoolingDown();
  const token = await getAccessToken();
  const query = clientId ? `?clientId=${encodeURIComponent(clientId)}` : '';
  if (!token && !clientId) throw new BillingAuthenticationRequiredError();
  const response = await fetch(`${apiOrigin}/v1/usage${query}`, { headers: token ? { authorization: `Bearer ${token}` } : {} });
  return readUsageResponse(response, usageSummarySchema);
}

export async function recordUsageEvent(input: {
  clientId: string;
  capability: RecordedUsageCapability;
  idempotencyKey: string;
  metadata?: Record<string, unknown>;
}): Promise<UsageSummary | null> {
  throwIfUsageCoolingDown();
  const token = await getAccessToken();
  const response = await fetch(`${apiOrigin}/v1/usage/events`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      ...(token ? { authorization: `Bearer ${token}` } : {}),
    },
    body: JSON.stringify(input),
  });
  return readUsageResponse(response, usageSummarySchema);
}

export type PublicShare = {
  resource: ShareResource;
};

export async function getPublicShare(shareID: string): Promise<PublicShare> {
  const response = await fetch(`${apiOrigin}/v1/public/shares/${encodeURIComponent(shareID)}`);
  if (!response.ok) throw await readError(response);
  const body = await readJsonResponse(response, publicShareResponseSchema);
  const resource = parseShareResource({ type: body.resourceType, payload: body.resourcePayload });
  if (!resource) throw new Error('The shared content is invalid or corrupted.');
  return { resource };
}
