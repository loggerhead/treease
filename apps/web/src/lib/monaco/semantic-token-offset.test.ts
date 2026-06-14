import { describe, expect, it } from 'vitest';
import { offsetSemanticTokens } from './semantic-token-offset';

function asArray(buffer: ArrayBuffer): number[] {
  return [...new Uint32Array(buffer)];
}

describe('offsetSemanticTokens', () => {
  it('offsets single-line tokens by the block start position', () => {
    const tokens = new Uint32Array([
      0, 1, 3, 0, 0,
      0, 5, 1, 1, 0,
    ]);

    expect(asArray(offsetSemanticTokens(tokens, 3, 8))).toEqual([
      2, 8, 3, 0, 0,
      0, 5, 1, 1, 0,
    ]);
  });

  it('only applies the column offset to the first block line', () => {
    const tokens = new Uint32Array([
      0, 0, 1, 0, 0,
      1, 2, 4, 1, 0,
    ]);

    expect(asArray(offsetSemanticTokens(tokens, 5, 12))).toEqual([
      4, 11, 1, 0, 0,
      1, 2, 4, 1, 0,
    ]);
  });

  it('keeps zero-based source positions unchanged for top-left blocks', () => {
    const tokens = new Uint32Array([0, 0, 6, 2, 0]);

    expect(asArray(offsetSemanticTokens(tokens, 1, 1))).toEqual([0, 0, 6, 2, 0]);
  });

  it('returns empty tokens for empty or malformed input', () => {
    expect(asArray(offsetSemanticTokens(new Uint32Array(), 2, 4))).toEqual([]);
    expect(asArray(offsetSemanticTokens(new Uint32Array([0, 0, 1, 0]), 2, 4))).toEqual([]);
  });
});
