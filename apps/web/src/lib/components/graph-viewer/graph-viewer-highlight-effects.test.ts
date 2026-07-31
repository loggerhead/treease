import { describe, expect, it } from 'vitest';
import { buildGraphHighlightSignature } from './graph-viewer-highlight-effects';

describe('graph highlight effects', () => {
  it('distinguishes repeated breadcrumb reveal requests for the same path', () => {
    const first = {
      path: [{ tag: 0, key: 'preview', index: 0 }],
      target: 'key' as const,
      revision: 4,
      source: 'breadcrumb' as const,
      revealToken: 1,
    };
    const repeated = { ...first, revealToken: 2 };
    const pathKey = (path: typeof first.path) => path.map((segment) => segment.key).join('.');

    expect(buildGraphHighlightSignature(repeated, pathKey))
      .not.toBe(buildGraphHighlightSignature(first, pathKey));
  });
});
