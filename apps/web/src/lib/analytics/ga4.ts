type GoogleTagCommand = (...args: unknown[]) => void;
type QueuedAnalyticsEvent = { name: string; params: AnalyticsEventParams };

export type AnalyticsEventName =
  | 'editor_open'
  | 'document_import'
  | 'document_export'
  | 'format_document'
  | 'compare_document'
  | 'graph_view'
  | 'graph_search'
  | 'graph_edit'
  | 'share_started'
  | 'parse_failed'
  | 'language_selected'
  | 'subscription_viewed'
  | 'subscription_management_started'
  | 'entitlement_blocked'
  | 'quota_threshold_reached'
  | 'seo_conversion';

export type AnalyticsEventParams = Record<string, string | number | boolean>;

const DEFAULT_MEASUREMENT_ID = 'G-N8DW5G72ZQ';
const configuredMeasurementId = import.meta.env.GA_MEASUREMENT_ID ?? '';
const measurementId = (configuredMeasurementId || (import.meta.env.PROD ? DEFAULT_MEASUREMENT_ID : '')).trim();
const consentRequired = import.meta.env.GA_CONSENT_REQUIRED === '1';
const CONSENT_REGIONS = [
  'AT',
  'BE',
  'BG',
  'CH',
  'CY',
  'CZ',
  'DE',
  'DK',
  'EE',
  'ES',
  'FI',
  'FR',
  'GB',
  'GR',
  'HR',
  'HU',
  'IE',
  'IS',
  'IT',
  'LI',
  'LT',
  'LU',
  'LV',
  'MT',
  'NL',
  'NO',
  'PL',
  'PT',
  'RO',
  'SE',
  'SI',
  'SK',
] as const;
const desktopSurface = import.meta.env.PUBLIC_WORKSPACE_SURFACE === 'desktop';
const allowedParamKeys = new Set([
  'source',
  'language',
  'result',
  'format',
  'operation',
  'mode',
  'edit_type',
  'plan',
  'surface',
  'status',
  'feature',
  'reason',
  'threshold',
  'page_path',
  'page_title',
  'landing_page',
  'landing_source',
  'landing_medium',
  'landing_campaign',
  'landing_content',
  'landing_referrer',
  'conversion',
]);

const landingAttributionKey = 'treease:landing-attribution';

let initialized = false;
let consentGranted = !consentRequired;
let scriptPromise: Promise<void> | null = null;
let flushScheduled = false;
const queuedEvents: QueuedAnalyticsEvent[] = [];

function getGoogleTag(): GoogleTagCommand | null {
  if (typeof window === 'undefined') return null;
  return (window as Window & { gtag?: GoogleTagCommand }).gtag ?? null;
}

function enqueueGoogleTag(): GoogleTagCommand | null {
  if (typeof window === 'undefined' || !measurementId) return null;

  const scopedWindow = window as Window & {
    dataLayer?: unknown[];
    gtag?: GoogleTagCommand;
  };
  scopedWindow.dataLayer ??= [];
  scopedWindow.gtag ??= function gtag() {
    scopedWindow.dataLayer?.push(arguments);
  };
  return scopedWindow.gtag;
}

function loadGoogleTag(): Promise<void> {
  if (scriptPromise) return scriptPromise;
  if (typeof document === 'undefined' || !measurementId) return Promise.resolve();

  scriptPromise = new Promise<void>((resolve, reject) => {
    const script = document.createElement('script');
    script.async = true;
    script.src = `https://www.googletagmanager.com/gtag/js?id=${encodeURIComponent(measurementId)}`;
    script.onload = () => resolve();
    script.onerror = () => reject(new Error('Failed to load Google Analytics'));
    document.head.appendChild(script);
  });
  return scriptPromise;
}

function flushQueuedEvents(): void {
  flushScheduled = false;
  if (!initialized) return;
  const gtag = getGoogleTag();
  if (!gtag) return;

  const events = queuedEvents.splice(0, queuedEvents.length);
  for (const event of events) gtag('event', event.name, event.params);
}

function scheduleFlush(): void {
  if (flushScheduled) return;
  flushScheduled = true;
  if (typeof window === 'undefined') {
    queueMicrotask(flushQueuedEvents);
    return;
  }
  window.setTimeout(flushQueuedEvents, 0);
}

function setDefaultConsent(gtag: GoogleTagCommand): void {
  const grantedOutsideConsentRegions = consentRequired ? 'denied' : 'granted';
  gtag('consent', 'default', {
    ad_storage: grantedOutsideConsentRegions,
    ad_user_data: grantedOutsideConsentRegions,
    ad_personalization: grantedOutsideConsentRegions,
    analytics_storage: grantedOutsideConsentRegions,
    wait_for_update: 500,
  });

  if (!consentRequired) {
    gtag('consent', 'default', {
      ad_storage: 'denied',
      ad_user_data: 'denied',
      ad_personalization: 'denied',
      analytics_storage: 'denied',
      region: CONSENT_REGIONS,
      wait_for_update: 500,
    });
  }
}

function enqueueEvent(name: string, params: AnalyticsEventParams): void {
  if (desktopSurface) return;
  if (!measurementId || (consentRequired && !consentGranted) || queuedEvents.length >= 100) return;
  const safeParams: AnalyticsEventParams = {};
  const attribution = name === 'page_view' || name === 'seo_conversion' ? captureLandingAttribution() : {};
  for (const [key, value] of Object.entries({ ...attribution, ...params })) {
    if (!allowedParamKeys.has(key)) continue;
    if (typeof value === 'string') safeParams[key] = value.slice(0, 100);
    else if (typeof value === 'number' && Number.isFinite(value)) safeParams[key] = value;
    else if (typeof value === 'boolean') safeParams[key] = value;
  }
  queuedEvents.push({ name, params: safeParams });
  scheduleFlush();
}

type LandingAttribution = Pick<AnalyticsEventParams, 'landing_page' | 'landing_source' | 'landing_medium'> &
  Partial<Pick<AnalyticsEventParams, 'landing_campaign' | 'landing_content' | 'landing_referrer'>>;

function safeAttributionValue(value: string | null | undefined): string | undefined {
  const normalized = value?.trim().slice(0, 100);
  return normalized || undefined;
}

function captureLandingAttribution(): LandingAttribution {
  if (typeof window === 'undefined') {
    return { landing_page: '/', landing_source: 'direct', landing_medium: 'none' };
  }

  try {
    const stored = window.sessionStorage.getItem(landingAttributionKey);
    if (stored) return JSON.parse(stored) as LandingAttribution;
  } catch {
    // Storage may be unavailable in private browsing or embedded contexts.
  }

  const url = new URL(window.location.href);
  let referrer: URL | null = null;
  try {
    if (document.referrer) referrer = new URL(document.referrer);
  } catch {
    referrer = null;
  }
  const utmSource = safeAttributionValue(url.searchParams.get('utm_source'));
  const utmMedium = safeAttributionValue(url.searchParams.get('utm_medium'));
  const attribution: LandingAttribution = {
    landing_page: sanitizePagePath(url.pathname),
    landing_source: utmSource ?? (referrer ? 'referral' : 'direct'),
    landing_medium: utmMedium ?? (referrer ? 'referral' : 'none'),
  };
  const campaign = safeAttributionValue(url.searchParams.get('utm_campaign'));
  const content = safeAttributionValue(url.searchParams.get('utm_content'));
  const referrerOrigin = referrer && referrer.origin !== window.location.origin ? referrer.origin : undefined;
  if (campaign) attribution.landing_campaign = campaign;
  if (content) attribution.landing_content = content;
  if (referrerOrigin) attribution.landing_referrer = safeAttributionValue(referrerOrigin);

  try {
    window.sessionStorage.setItem(landingAttributionKey, JSON.stringify(attribution));
  } catch {
    // The event remains useful even when session storage is unavailable.
  }
  return attribution;
}

export function sanitizePagePath(pathname: string): string {
  if (!pathname || pathname === '/') return '/';
  const normalized = pathname.startsWith('/') ? pathname : `/${pathname}`;
  return normalized.replace(/\/{2,}/g, '/').replace(/\/$/, '') || '/';
}

export async function initializeAnalytics(): Promise<void> {
  if (desktopSurface) return;
  if (initialized || !measurementId || !consentGranted) return;

  const gtag = enqueueGoogleTag();
  if (!gtag) return;

  const timestamp = new Date();
  gtag('js', timestamp);
  setDefaultConsent(gtag);
  gtag('config', measurementId, { send_page_view: false });

  try {
    await loadGoogleTag();
    initialized = true;
    scheduleFlush();
  } catch {
    scriptPromise = null;
  }
}

export async function setAnalyticsConsent(granted: boolean): Promise<void> {
  consentGranted = granted;
  if (!granted) queuedEvents.splice(0, queuedEvents.length);
  if (!measurementId) return;

  if (granted) await initializeAnalytics();

  const gtag = enqueueGoogleTag();
  if (!gtag) return;

  gtag('consent', 'update', {
    analytics_storage: granted ? 'granted' : 'denied',
  });
}

export function trackPageView(pathname: string): void {
  if (desktopSurface) return;
  const params: AnalyticsEventParams = { page_path: sanitizePagePath(pathname) };
  if (typeof document !== 'undefined') params.page_title = document.title;
  enqueueEvent('page_view', params);
}

export function trackEvent(name: AnalyticsEventName, params: AnalyticsEventParams = {}): void {
  enqueueEvent(name, params);
}

export type SeoConversionName = 'editor_open' | 'pricing_cta';

export function trackSeoConversion(
  conversion: SeoConversionName,
  params: Omit<AnalyticsEventParams, 'conversion'> = {},
): void {
  trackEvent('seo_conversion', { ...params, conversion });
}
