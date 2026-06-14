import { PathSegTag, type PathSeg } from '@core-wasm/index'
import { pathSegKeyValue } from '../../shared/path';

function isSimpleKey(key: string): boolean {
  return /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(key);
}

function appendReadablePathSegment(path: string, segment: PathSeg): string {
  if (isPathSegIndex(segment)) return `${path}[${segment.index}]`;
  const key = pathSegKeyValue(segment);
  if (isSimpleKey(key)) return `${path}.${key}`;
  return `${path}[${JSON.stringify(key)}]`;
}

export function buildReadablePath(path: PathSeg[]): string {
  if (!Array.isArray(path) || path.length === 0) return '$';
  let current = '$';
  for (const segment of path) {
    current = appendReadablePathSegment(current, segment);
  }
  return current;
}

export type { PathSeg };
export { pathSegKeyValue };

export function isPathSegKey(seg: PathSeg): boolean {
  return seg.tag === PathSegTag.KEY;
}

export function isPathSegIndex(seg: PathSeg): boolean {
  return seg.tag === PathSegTag.INDEX;
}

export function breadcrumbTargetForPath(path: PathSeg[]): 'key' | 'value' | undefined {
  if (!Array.isArray(path) || path.length === 0) return undefined;
  return isPathSegIndex(path[path.length - 1]!) ? 'value' : 'key';
}
