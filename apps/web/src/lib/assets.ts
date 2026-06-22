import { dev } from '$app/environment';
import { env } from '$env/dynamic/public';

const ASSET_BASE_URL = (
  env.PUBLIC_ASSET_BASE_URL ||
  (dev ? '' : 'https://assets.treease.com')
).replace(/\/+$/, '');

export function assetUrl(path: string): string {
  return `${ASSET_BASE_URL}${path.startsWith('/') ? path : `/${path}`}`;
}
