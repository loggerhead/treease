import { describe, expect, it } from 'vitest';
import { SemType } from '@core-wasm/index';

import {
  buildRootScalarSemanticTokens,
  resolveRootScalarHighlightKind,
  resolveRootScalarHighlightKindFromSemType,
} from './root-scalar-highlight';

describe('root scalar highlighting', () => {
  it('uses the Core analysis-tree semantic type instead of the JavaScript value', () => {
    expect(resolveRootScalarHighlightKind({ tree: { semType: SemType.FLOAT }, value: 1 })).toBe('float');
    expect(resolveRootScalarHighlightKind({ tree: { semType: SemType.NIL }, value: null })).toBe('nil');
  });

  it('seeds the existing Monaco semantic-token provider from an exact Core SemType', () => {
    const tokenTypes = ['map', 'key', 'seq', 'str', 'int', 'float', 'boolean', 'nil'];
    expect(resolveRootScalarHighlightKindFromSemType(SemType.FLOAT)).toBe('float');
    expect(Array.from(new Uint32Array(buildRootScalarSemanticTokens('1.0', SemType.FLOAT, tokenTypes)!))).toEqual([
      0, 0, 3, 5, 0,
    ]);
    expect(buildRootScalarSemanticTokens('{}', SemType.MAP, tokenTypes)).toBeNull();
  });
});
