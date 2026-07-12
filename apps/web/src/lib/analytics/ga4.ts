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
  | 'language_selected';

export type AnalyticsEventParams = Record<string, string | number | boolean>;

const DEFAULT_MEASUREMENT_ID = 'G-N8DW5G72ZQ';
const configuredMeasurementId = import.meta.env.GA_MEASUREMENT_ID ?? '';
const measurementId = (configuredMeasurementId || (import.meta.env.PROD ? DEFAULT_MEASUREMENT_ID : '')).trim();
const consentRequired = import.meta.env.GA_CONSENT_REQUIRED === '1';
const desktopSurface = import.meta.env.PUBLIC_WORKSPACE_SURFACE === 'desktop';
const allowedParamKeys = new Set([
  'source',
  'language',
  'result',
  'format',
  'operation',
  'mode',
  'edit_type',
]);

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
  scopedWindow.gtag ??= (...args: unknown[]) => scopedWindow.dataLayer?.push(args);
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

function enqueueEvent(name: string, params: AnalyticsEventParams): void {
  if (desktopSurface) return;
  if (!measurementId || (consentRequired && !consentGranted) || queuedEvents.length >= 100) return;
  const safeParams: AnalyticsEventParams = {};
  for (const [key, value] of Object.entries(params)) {
    if (!allowedParamKeys.has(key)) continue;
    if (typeof value === 'string') safeParams[key] = value.slice(0, 100);
    else if (typeof value === 'number' && Number.isFinite(value)) safeParams[key] = value;
    else if (typeof value === 'boolean') safeParams[key] = value;
  }
  queuedEvents.push({ name, params: safeParams });
  scheduleFlush();
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
  gtag('consent', 'default', {
    ad_storage: 'denied',
    ad_user_data: 'denied',
    ad_personalization: 'denied',
    analytics_storage: consentRequired ? 'denied' : 'granted',
    wait_for_update: 500,
  });
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
