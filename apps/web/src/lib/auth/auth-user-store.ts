import type { User } from '@supabase/supabase-js';
import { writable } from 'svelte/store';
import { ensureAuthSession, getSupabaseClient } from './supabase-auth';

export const authUser = writable<User | null>(null);
export const authReady = writable(false);

// Landing and editor headers share one Supabase listener. The generation guard
// prevents a retired listener from overwriting the next mounted header's state.
let observerCount = 0;
let stopAuthObserver: (() => void) | null = null;
let observerGeneration = 0;

export function observeAuthUser(): () => void {
  observerCount += 1;
  if (!stopAuthObserver) {
    authReady.set(false);
    const generation = ++observerGeneration;
    const { data } = getSupabaseClient().auth.onAuthStateChange((_event, session) => {
      if (generation === observerGeneration) authUser.set(session?.user ?? null);
    });
    void ensureAuthSession()
      .then((session) => {
        if (generation === observerGeneration) authUser.set(session?.user ?? null);
      })
      .catch(() => {
        if (generation === observerGeneration) authUser.set(null);
      })
      .finally(() => {
        if (generation === observerGeneration) authReady.set(true);
      });
    stopAuthObserver = () => data.subscription.unsubscribe();
  }

  return () => {
    observerCount -= 1;
    if (observerCount > 0) return;
    observerCount = 0;
    observerGeneration += 1;
    stopAuthObserver?.();
    stopAuthObserver = null;
    authUser.set(null);
    authReady.set(false);
  };
}

export function authUserDetails(user: User): {
  name: string;
  email: string;
  avatarUrl: string | null;
  initial: string;
} {
  const metadata = user.user_metadata;
  const email = user.email ?? '';
  const nameCandidates = [metadata.full_name, metadata.name, metadata.user_name, metadata.preferred_username];
  const profileName = nameCandidates.find(
    (value): value is string => typeof value === 'string' && value.trim().length > 0,
  );
  const name = (profileName ?? email.split('@')[0]) || 'User';
  const avatarCandidates = [metadata.avatar_url, metadata.picture];
  const avatarUrl = avatarCandidates.find((value): value is string => typeof value === 'string' && value.trim().length > 0)
    ?? null;
  return { name, email, avatarUrl, initial: name.charAt(0).toUpperCase() || 'U' };
}
