import { afterEach, describe, expect, it, vi } from 'vitest';

import { graphViewTopologyKey } from './graph-view-usage';

describe('graph view topology usage', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('derives a versioned usage key from canonical topology bytes', async () => {
    const digest = vi.fn(async () => new Uint8Array([0, 1, 254, 255]).buffer);
    vi.stubGlobal('crypto', { subtle: { digest } });

    await expect(graphViewTopologyKey(new Uint8Array([0b11010000]))).resolves.toBe(
      'graph-topology-v1:0001feff',
    );
    expect(digest).toHaveBeenCalledWith('SHA-256', expect.any(ArrayBuffer));
  });
});
