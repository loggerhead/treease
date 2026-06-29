import { isPathSegIndex, isPathSegKey, pathSegKeyValue, type PathSeg } from '../store/tree-path';

export function setValueAtPath(data: any, path: PathSeg[], value: unknown): unknown {
  if (path.length == 0) return value;
  let target = data;
  for (let i = 0; i < path.length - 1; i += 1) {
    const seg = path[i];
    const key = isPathSegIndex(seg) ? seg.index : pathSegKeyValue(seg);
    if (target == null) return data;
    target = target[key];
  }
  const lastSeg = path[path.length - 1];
  const lastKey = isPathSegIndex(lastSeg) ? lastSeg.index : pathSegKeyValue(lastSeg);
  if (target != null) target[lastKey] = value;
  return data;
}

export function normalizeKeyInput(raw: string, languageId?: string): string {
  if (languageId !== 'json') return raw;
  try {
    const parsed = JSON.parse(raw);
    if (typeof parsed === 'string') return parsed;
  } catch {
    return raw;
  }
  return raw;
}

export function renameKeyAtPath(data: any, path: PathSeg[], nextKey: string): unknown {
  if (path.length == 0) return data;
  const lastSeg = path[path.length - 1];
  if (!isPathSegKey(lastSeg)) return data;
  const lastKey = pathSegKeyValue(lastSeg);
  let target = data;
  for (let i = 0; i < path.length - 1; i += 1) {
    const seg = path[i];
    const key = isPathSegIndex(seg) ? seg.index : pathSegKeyValue(seg);
    if (target == null) return data;
    target = target[key];
  }
  if (target == null || typeof target !== 'object' || Array.isArray(target)) return data;
  if (lastKey === nextKey) return data;
  const value = target[lastKey];
  delete target[lastKey];
  target[nextKey] = value;
  return data;
}

export function getValueAtPath(data: any, path: PathSeg[]): unknown {
  if (!path || path.length == 0) return data;
  let target = data;
  for (let i = 0; i < path.length; i += 1) {
    if (target == null) return undefined;
    const seg = path[i];
    const key = isPathSegIndex(seg) ? seg.index : pathSegKeyValue(seg);
    target = target[key];
  }
  return target;
}

// NOTE: A namesake `buildPathKey` also exists in
// `apps/web/src/workers/runtime/tree-path.ts` (identical logic).
// Keep in sync if changing the format.
export function buildPathKey(path: PathSeg[]): string {
  if (!path || path.length == 0) return '';
  return path.map((seg) => (isPathSegKey(seg) ? `k:${pathSegKeyValue(seg)}` : `i:${seg.index}`)).join('|');
}
