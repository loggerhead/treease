import { describe, expect, it } from 'vitest';
import { authCallbackUrl, currentAuthReturnPath, resolveAuthReturnPath } from './auth-redirect';

const editorLocation = {
  origin: 'https://treease.io',
  pathname: '/editor',
  search: '?lang=yaml&text=hello',
  hash: '#viewer',
};

describe('auth redirect', () => {
  it('preserves the complete page-local return path in web callbacks', () => {
    expect(currentAuthReturnPath(editorLocation)).toBe('/editor?lang=yaml&text=hello#viewer');
    expect(authCallbackUrl(editorLocation, false)).toBe(
      'https://treease.io/auth/callback?next=%2Feditor%3Flang%3Dyaml%26text%3Dhello%23viewer',
    );
  });

  it('carries the same return path through desktop callbacks', () => {
    expect(authCallbackUrl(editorLocation, true)).toBe(
      'treease://auth/callback?next=%2Feditor%3Flang%3Dyaml%26text%3Dhello%23viewer',
    );
  });

  it.each([
    [null, '/'],
    ['https://attacker.example/path', '/'],
    ['//attacker.example/path', '/'],
    ['/auth/callback?code=loop', '/'],
    ['/tutorial/getting-started?step=2#example', '/tutorial/getting-started?step=2#example'],
  ])('resolves %s to a safe same-origin destination', (value, expected) => {
    expect(resolveAuthReturnPath(value)).toBe(expected);
  });
});
