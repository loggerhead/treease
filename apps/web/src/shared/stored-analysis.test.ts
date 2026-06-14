import { describe, expect, it } from 'vitest';
import { semanticTokensToArrayBuffer } from './stored-analysis';

describe('shared stored-analysis helpers', () => {
  it('semanticTokensToArrayBuffer copies the transferred payload bytes', () => {
    const raw = new Uint32Array([1, 2, 3, 4]);

    const buffer = semanticTokensToArrayBuffer(raw);
    const copied = new Uint32Array(buffer);
    expect(Array.from(copied)).toEqual([1, 2, 3, 4]);

    raw[0] = 99;
    expect(Array.from(new Uint32Array(buffer))).toEqual([1, 2, 3, 4]);
  });
});
