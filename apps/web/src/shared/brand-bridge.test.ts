import { describe, expect, it } from 'vitest';
import { createEmptyDiffResult, toWasmTreeNode } from './brand-bridge';

describe('shared brand bridge helpers', () => {
  it('createEmptyDiffResult preserves the empty diff payload shape', () => {
    expect(createEmptyDiffResult()).toEqual({ pairs: [] });
  });

  it('toWasmTreeNode preserves the plain tree node fields', () => {
    expect(
      toWasmTreeNode({
        kind: 2 as any,
        semType: 3 as any,
        tag: '',
        value: 'Ada',
        children: [],
      }),
    ).toEqual({
      kind: 2,
      semType: 3,
      tag: '',
      value: 'Ada',
      children: [],
    });
  });
});
