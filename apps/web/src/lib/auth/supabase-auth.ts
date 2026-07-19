import { createClient, type Provider, type SupabaseClient } from '@supabase/supabase-js';
import type { Session } from '@supabase/supabase-js';
import { workspaceHost } from '../workspace-host';
import { authCallbackUrl } from './auth-redirect';

let client: SupabaseClient | null = null;

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

export type EmailAuthFlow = 'sign-in' | 'anonymous-link';

export function isAnonymousUser(user: { is_anonymous?: boolean } | null | undefined): boolean {
  return user?.is_anonymous === true;
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

  const { data, error } = await supabase.auth.signInAnonymously();
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
  const { data, error } = isAnonymousUser(session?.user)
    ? await supabase.auth.linkIdentity({
        provider,
        options: { redirectTo: authRedirectUrl(), skipBrowserRedirect: desktop },
      })
    : await supabase.auth.signInWithOAuth({
        provider,
        options: { redirectTo: authRedirectUrl(), skipBrowserRedirect: desktop },
      });
  if (error) throw error;
  if (desktop) {
    if (!data.url) throw new Error('The authentication provider did not return a browser URL.');
    await (await workspaceHost).openExternal(new URL(data.url));
  }
}

export async function sendEmailOtp(email: string): Promise<EmailAuthFlow> {
  const supabase = getSupabaseClient();
  const session = (await ensureAuthSession()) ?? (await supabase.auth.getSession()).data.session;
  if (isAnonymousUser(session?.user)) {
    const { error } = await supabase.auth.updateUser({ email });
    if (error) throw error;
    return 'anonymous-link';
  }

  const { error } = await supabase.auth.signInWithOtp({
    email,
    options: {
      shouldCreateUser: true,
      emailRedirectTo: authRedirectUrl(),
    },
  });
  if (error) throw error;
  return 'sign-in';
}

export async function verifyEmailOtp(email: string, token: string, flow: EmailAuthFlow = 'sign-in'): Promise<Session | null> {
  const { data, error } = await getSupabaseClient().auth.verifyOtp({
    email,
    token,
    type: flow === 'anonymous-link' ? 'email_change' : 'email',
  });
  if (error) throw error;
  return data.session;
}

export async function exchangeAuthCode(code: string): Promise<Session | null> {
  const { data, error } = await getSupabaseClient().auth.exchangeCodeForSession(code);
  if (error) throw error;
  return data.session;
}

export async function signOut(): Promise<void> {
  if (import.meta.env.PUBLIC_WORKSPACE_SURFACE === 'desktop') {
    await (await workspaceHost).clearRefreshToken();
  }
  const { error } = await getSupabaseClient().auth.signOut();
  if (error) throw error;
  await ensureAuthSession();
}
