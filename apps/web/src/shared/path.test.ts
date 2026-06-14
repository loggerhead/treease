import { describe, expect, it } from 'vitest';
import { PathSegTag } from '@core-wasm/index';
import { pathSegKeyValue } from './path';

describe('shared path helpers', () => {
  it('pathSegKeyValue preserves the protocol key string', () => {
    expect(pathSegKeyValue({ tag: PathSegTag.KEY, key: 'protocol-key' as any, index: 0 } as any)).toBe('protocol-key');
  });
});
