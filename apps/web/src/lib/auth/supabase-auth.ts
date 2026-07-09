import { createClient, type Provider, type SupabaseClient } from '@supabase/supabase-js';

let client: SupabaseClient | null = null;

export function getSupabaseClient(): SupabaseClient {
  if (client) return client;

  const url = String(import.meta.env.SUPABASE_URL ?? '').trim();
  const anonKey = String(import.meta.env.SUPABASE_ANON_KEY ?? '').trim();
  if (!url || !anonKey) {
    throw new Error('Supabase login is not configured. Set SUPABASE_URL and SUPABASE_ANON_KEY.');
  }

  client = createClient(url, anonKey, {
    auth: {
      persistSession: true,
      autoRefreshToken: true,
      detectSessionInUrl: true,
    },
  });
  return client;
}

function authRedirectUrl(): string {
  return `${window.location.origin}/auth/callback`;
}

export async function signInWithProvider(provider: Extract<Provider, 'google' | 'github'>): Promise<void> {
  const { error } = await getSupabaseClient().auth.signInWithOAuth({
    provider,
    options: { redirectTo: authRedirectUrl() },
  });
  if (error) throw error;
}

export async function sendEmailOtp(email: string): Promise<void> {
  const { error } = await getSupabaseClient().auth.signInWithOtp({
    email,
    options: {
      shouldCreateUser: true,
      emailRedirectTo: authRedirectUrl(),
    },
  });
  if (error) throw error;
}

export async function verifyEmailOtp(email: string, token: string): Promise<void> {
  const { error } = await getSupabaseClient().auth.verifyOtp({ email, token, type: 'email' });
  if (error) throw error;
}

export async function exchangeAuthCode(code: string): Promise<void> {
  const { error } = await getSupabaseClient().auth.exchangeCodeForSession(code);
  if (error) throw error;
}
