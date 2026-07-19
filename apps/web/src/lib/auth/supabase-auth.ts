import { createClient, type Provider, type SupabaseClient } from '@supabase/supabase-js';
import type { Session } from '@supabase/supabase-js';
import { workspaceHost } from '../workspace-host';
import { authCallbackUrl } from './auth-redirect';

let client: SupabaseClient | null = null;

type TurnstileApi = {
  render: (container: HTMLElement, options: {
    sitekey: string;
    size: 'invisible';
    callback: (token: string) => void;
    'error-callback': () => void;
    'expired-callback': () => void;
  }) => string;
  execute: (widgetId: string) => void;
  remove: (widgetId: string) => void;
};

declare global {
  interface Window {
    turnstile?: TurnstileApi;
  }
}

let turnstileApiPromise: Promise<TurnstileApi> | null = null;

export function getSupabaseConfiguration(): { url: string; anonKey: string } {
  const url = String(import.meta.env.SUPABASE_URL ?? '').trim();
  const anonKey = String(import.meta.env.SUPABASE_ANON_KEY ?? '').trim();
  if (!url || !anonKey) {
    throw new Error('Supabase login is not configured. Set SUPABASE_URL and SUPABASE_ANON_KEY.');
  }
  return { url, anonKey };
}

export function getSupabaseClient(): SupabaseClient {
  if (client) return client;

  const { url, anonKey } = getSupabaseConfiguration();

  const desktop = import.meta.env.PUBLIC_WORKSPACE_SURFACE === 'desktop';
  client = createClient(url, anonKey, {
    auth: {
      flowType: 'pkce',
      persistSession: !desktop,
      autoRefreshToken: !desktop,
      detectSessionInUrl: false,
    },
  });
  return client;
}

export function isAnonymousUser(user: { is_anonymous?: boolean } | null | undefined): boolean {
  return user?.is_anonymous === true;
}

async function loadTurnstile(): Promise<TurnstileApi> {
  if (typeof window === 'undefined' || typeof document === 'undefined') {
    throw new Error('Turnstile can only run in a browser session.');
  }
  if (window.turnstile) return window.turnstile;
  if (!turnstileApiPromise) {
    turnstileApiPromise = new Promise<TurnstileApi>((resolve, reject) => {
      const existing = document.querySelector<HTMLScriptElement>('script[data-treease-turnstile]');
      if (existing) {
        existing.addEventListener('load', () => window.turnstile ? resolve(window.turnstile) : reject(new Error('Turnstile failed to initialize.')), { once: true });
        existing.addEventListener('error', () => reject(new Error('Turnstile failed to load.')), { once: true });
        return;
      }

      const script = document.createElement('script');
      script.src = 'https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit';
      script.async = true;
      script.defer = true;
      script.dataset.treeaseTurnstile = 'true';
      script.onload = () => window.turnstile ? resolve(window.turnstile) : reject(new Error('Turnstile failed to initialize.'));
      script.onerror = () => reject(new Error('Turnstile failed to load.'));
      document.head.appendChild(script);
    });
  }
  return turnstileApiPromise;
}

async function getTurnstileToken(): Promise<string> {
  const siteKey = String(import.meta.env.PUBLIC_TURNSTILE_SITE_KEY ?? '').trim();
  if (!siteKey) throw new Error('Supabase CAPTCHA is enabled but PUBLIC_TURNSTILE_SITE_KEY is missing.');

  const turnstile = await loadTurnstile();
  const container = document.createElement('div');
  container.setAttribute('aria-hidden', 'true');
  container.style.position = 'fixed';
  container.style.width = '1px';
  container.style.height = '1px';
  container.style.opacity = '0';
  container.style.pointerEvents = 'none';
  document.body.appendChild(container);

  let widgetId: string | null = null;
  try {
    const token = await new Promise<string>((resolve, reject) => {
      widgetId = turnstile.render(container, {
        sitekey: siteKey,
        size: 'invisible',
        callback: resolve,
        'error-callback': () => reject(new Error('Turnstile verification failed.')),
        'expired-callback': () => reject(new Error('Turnstile verification expired.')),
      });
      turnstile.execute(widgetId);
    });
    return token;
  } finally {
    if (widgetId) turnstile.remove(widgetId);
    container.remove();
  }
}

async function restoreDesktopSession(): Promise<Session | null> {
  const host = await workspaceHost;
  if (!(await host.hasRefreshToken())) return null;

  const { url, anonKey } = getSupabaseConfiguration();
  const tokens = await host.refreshSession(url, anonKey);
  const { data, error } = await getSupabaseClient().auth.setSession({
    access_token: tokens.accessToken,
    refresh_token: tokens.refreshToken,
  });
  if (error) throw error;
  return data.session;
}

export async function ensureAuthSession(): Promise<Session | null> {
  const supabase = getSupabaseClient();
  const desktop = import.meta.env.PUBLIC_WORKSPACE_SURFACE === 'desktop';
  const current = desktop ? await restoreDesktopSession() : (await supabase.auth.getSession()).data.session;
  if (current) return current;

  const captchaToken = await getTurnstileToken();
  const { data, error } = await supabase.auth.signInAnonymously({ options: { captchaToken } });
  if (error) throw error;
  if (desktop && data.session?.refresh_token) {
    await (await workspaceHost).storeRefreshToken(data.session.refresh_token);
  }
  return data.session;
}

function authRedirectUrl(): string {
  return authCallbackUrl(window.location, import.meta.env.PUBLIC_WORKSPACE_SURFACE === 'desktop');
}

export async function signInWithProvider(provider: Extract<Provider, 'google' | 'github'>): Promise<void> {
  const desktop = import.meta.env.PUBLIC_WORKSPACE_SURFACE === 'desktop';
  const supabase = getSupabaseClient();
  const session = (await ensureAuthSession()) ?? (await supabase.auth.getSession()).data.session;
  if (isAnonymousUser(session?.user)) {
    // Formal login is intentionally independent from the anonymous identity.
    // This keeps existing provider accounts authoritative and leaves anonymous
    // usage untouched instead of silently merging two ledgers.
    await clearAuthSession();
  }

  const { data, error } = await supabase.auth.signInWithOAuth({
    provider,
    options: { redirectTo: authRedirectUrl(), skipBrowserRedirect: desktop },
  });
  if (error) throw error;
  if (desktop) {
    if (!data.url) throw new Error('The authentication provider did not return a browser URL.');
    await (await workspaceHost).openExternal(new URL(data.url));
  }
}

async function clearAuthSession(): Promise<void> {
  if (import.meta.env.PUBLIC_WORKSPACE_SURFACE === 'desktop') {
    await (await workspaceHost).clearRefreshToken();
  }
  const { error } = await getSupabaseClient().auth.signOut();
  if (error) throw error;
}

export async function sendEmailOtp(email: string): Promise<void> {
  const supabase = getSupabaseClient();
  const session = (await ensureAuthSession()) ?? (await supabase.auth.getSession()).data.session;
  if (isAnonymousUser(session?.user)) {
    await clearAuthSession();
  }

  const { error } = await supabase.auth.signInWithOtp({
    email,
    options: {
      shouldCreateUser: true,
      emailRedirectTo: authRedirectUrl(),
    },
  });
  if (error) throw error;
}

export async function verifyEmailOtp(email: string, token: string): Promise<Session | null> {
  const { data, error } = await getSupabaseClient().auth.verifyOtp({ email, token, type: 'email' });
  if (error) throw error;
  return data.session;
}

export async function exchangeAuthCode(code: string): Promise<Session | null> {
  const { data, error } = await getSupabaseClient().auth.exchangeCodeForSession(code);
  if (error) throw error;
  return data.session;
}

export async function signOut(): Promise<void> {
  await clearAuthSession();
  await ensureAuthSession();
}
