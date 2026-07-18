const fallbackReturnPath = '/';

export function currentAuthReturnPath(location: Pick<Location, 'pathname' | 'search' | 'hash'>): string {
  return `${location.pathname}${location.search}${location.hash}`;
}

export function authCallbackUrl(
  location: Pick<Location, 'origin' | 'pathname' | 'search' | 'hash'>,
  desktop: boolean,
): string {
  const callback = desktop
    ? new URL('treease://auth/callback')
    : new URL('/auth/callback', location.origin);
  callback.searchParams.set('next', currentAuthReturnPath(location));
  return callback.toString();
}

export function resolveAuthReturnPath(value: string | null): string {
  // OAuth return targets are local paths only; accepting an absolute or callback URL
  // would turn the login flow into an open redirect or a callback loop.
  if (!value || !value.startsWith('/') || value.startsWith('//')) return fallbackReturnPath;

  const parsed = new URL(value, 'https://treease.local');
  if (parsed.origin !== 'https://treease.local' || parsed.pathname === '/auth/callback') {
    return fallbackReturnPath;
  }
  return `${parsed.pathname}${parsed.search}${parsed.hash}`;
}
