import { describe, expect, it } from 'vitest';
import { PathSegTag } from '@core-wasm/index'
import { buildReadablePath, breadcrumbTargetForPath, isPathSegIndex, isPathSegKey, pathSegKeyValue } from './tree-path';

describe('tree-path', () => {
  it('isPathSegKey detects key segments', () => {
    expect(isPathSegKey({ tag: PathSegTag.KEY, key: '' as any, index: 0 } as any)).toBe(true);
    expect(isPathSegKey({ tag: PathSegTag.INDEX, key: '' as any, index: 0 } as any)).toBe(false);
  });

  it('isPathSegIndex detects index segments', () => {
    expect(isPathSegIndex({ tag: PathSegTag.INDEX, key: '' as any, index: 1 } as any)).toBe(true);
    expect(isPathSegIndex({ tag: PathSegTag.KEY, key: '' as any, index: 1 } as any)).toBe(false);
  });

  it('pathSegKeyValue stringifies key', () => {
    expect(pathSegKeyValue({ tag: PathSegTag.KEY, key: 'foo' as any, index: 0 } as any)).toBe('foo');
  });

  it('pathSegKeyValue returns the protocol key string unchanged', () => {
    expect(pathSegKeyValue({ tag: PathSegTag.KEY, key: 'protocol-key' as any, index: 0 } as any)).toBe('protocol-key');
  });

  it('buildReadablePath formats root path and escaped keys', () => {
    expect(buildReadablePath([])).toBe('$');
    expect(
      buildReadablePath([
        { tag: PathSegTag.KEY, key: 'meta' as any, index: 0 } as any,
        { tag: PathSegTag.KEY, key: 'b-c' as any, index: 0 } as any,
        { tag: PathSegTag.INDEX, key: '' as any, index: 2 } as any,
      ]),
    ).toBe('$.meta["b-c"][2]');
  });
  it('breadcrumbTargetForPath prefers key segments and value for index segments', () => {
    expect(breadcrumbTargetForPath([])).toBeUndefined();
    expect(breadcrumbTargetForPath([{ tag: PathSegTag.KEY, key: 'nested' as any, index: 0 } as any])).toBe('key');
    expect(
      breadcrumbTargetForPath([
        { tag: PathSegTag.KEY, key: 'flags' as any, index: 0 } as any,
        { tag: PathSegTag.INDEX, key: '' as any, index: 2 } as any,
      ]),
    ).toBe('value');
  });
});
