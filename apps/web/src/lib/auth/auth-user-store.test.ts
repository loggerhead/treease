import { describe, expect, it } from 'vitest';
import type { User } from '@supabase/supabase-js';
import { authUserDetails } from './auth-user-store';

function user(overrides: Partial<User>): User {
  return {
    id: 'user-1',
    app_metadata: {},
    user_metadata: {},
    aud: 'authenticated',
    created_at: '2026-01-01T00:00:00Z',
    ...overrides,
  };
}

describe('authUserDetails', () => {
  it('uses provider profile metadata for the account panel', () => {
    expect(authUserDetails(user({
      email: 'ada@example.com',
      user_metadata: { full_name: 'Ada Lovelace', avatar_url: 'https://example.com/ada.png' },
    }))).toEqual({
      name: 'Ada Lovelace',
      email: 'ada@example.com',
      avatarUrl: 'https://example.com/ada.png',
      initial: 'A',
    });
  });

  it('falls back to the email name and initial when no avatar is available', () => {
    expect(authUserDetails(user({ email: 'grace@example.com' }))).toEqual({
      name: 'grace',
      email: 'grace@example.com',
      avatarUrl: null,
      initial: 'G',
    });
  });
});
