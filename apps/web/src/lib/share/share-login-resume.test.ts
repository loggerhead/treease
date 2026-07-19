import { describe, expect, it } from 'vitest';
import { consumeShareResume, requestShareResume, type SessionStorageLike } from './share-login-resume';

function createStorage(): SessionStorageLike {
  const values = new Map<string, string>();
  return {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
    removeItem: (key) => values.delete(key),
  };
}

describe('share login resume', () => {
  it('consumes a pending share request exactly once after login', () => {
    const storage = createStorage();

    requestShareResume(storage);

    expect(consumeShareResume(storage)).toBe(true);
    expect(consumeShareResume(storage)).toBe(false);
  });
});
