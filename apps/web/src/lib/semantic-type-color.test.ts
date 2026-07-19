import { describe, expect, it } from 'vitest';
import { SemType } from '@core-wasm/index';

import { resolveSemanticTypeColor, semanticTypeToColorKey } from './semantic-type-color';

const colors = {
  map: '#01', key: '#02', seq: '#03', str: '#04', int: '#05', float: '#06', boolean: '#07', nil: '#08',
};

describe('semantic type colors', () => {
  it('keeps every Core value semantic type distinct without inferring from JavaScript number', () => {
    expect(semanticTypeToColorKey(SemType.NIL)).toBe('nil');
    expect(semanticTypeToColorKey(SemType.BOOLEAN)).toBe('boolean');
    expect(semanticTypeToColorKey(SemType.INT)).toBe('int');
    expect(semanticTypeToColorKey(SemType.FLOAT)).toBe('float');
    expect(semanticTypeToColorKey(SemType.STR)).toBe('str');
    expect(semanticTypeToColorKey(SemType.MAP)).toBe('map');
    expect(semanticTypeToColorKey(SemType.SEQ)).toBe('seq');
    expect(resolveSemanticTypeColor(colors, SemType.FLOAT)).toBe('#06');
    expect(resolveSemanticTypeColor(colors, SemType.INT)).toBe('#05');
  });
});
