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

type OAuthProvider = Extract<Provider, 'google' | 'github'>;

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
  return current;
}

function authRedirectUrl(): string {
  return authCallbackUrl(window.location, import.meta.env.PUBLIC_WORKSPACE_SURFACE === 'desktop');
}

export async function signInWithProvider(provider: OAuthProvider): Promise<void> {
  const desktop = import.meta.env.PUBLIC_WORKSPACE_SURFACE === 'desktop';
  const supabase = getSupabaseClient();
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

export async function clearAuthSession(): Promise<void> {
  if (import.meta.env.PUBLIC_WORKSPACE_SURFACE === 'desktop') {
    await (await workspaceHost).clearRefreshToken();
  }
  const { error } = await getSupabaseClient().auth.signOut({ scope: 'local' });
  if (error) throw error;
}

export async function sendEmailSignInLink(email: string): Promise<void> {
  const supabase = getSupabaseClient();
  const { error } = await supabase.auth.signInWithOtp({
    email,
    options: {
      shouldCreateUser: true,
      emailRedirectTo: authRedirectUrl(),
    },
  });
  if (error) throw error;
}

export async function exchangeAuthCode(code: string): Promise<Session | null> {
  const { data, error } = await getSupabaseClient().auth.exchangeCodeForSession(code);
  if (error) throw error;
  return data.session;
}

export async function signOut(): Promise<void> {
  await clearAuthSession();
}
