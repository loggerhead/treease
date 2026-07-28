import { describe, expect, it } from 'vitest';
import { ensureGraphViewerLayers } from './scene-layers';

describe('ensureGraphViewerLayers', () => {
  it('creates each layer once and preserves existing layers', () => {
    const added: unknown[] = [];
    class Box { constructor(readonly config: Record<string, unknown>) {} }
    const layers = { edgeLayer: null, nodeLayer: null, overlayLayer: null };
    const first = ensureGraphViewerLayers({ root: { add: (target) => added.push(target) }, BoxCtor: Box, layers });
    expect(added).toHaveLength(3);
    const second = ensureGraphViewerLayers({ root: { add: (target) => added.push(target) }, BoxCtor: Box, layers: { ...layers, ...first } });
    expect(second).toEqual({});
    expect(added).toHaveLength(3);
  });
});
