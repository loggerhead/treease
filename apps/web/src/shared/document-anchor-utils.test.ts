import { describe, it, expect } from 'vitest';
import { buildUtf8ByteSegments, byteOffsetToRowColumn, byteOffsetToUtf16Offset, byteOffsetToUtf16Position, serializePath } from './document-anchor-utils';

const COMPLEX_LONG_KEY =
  'we___are___such___stuff___as___dreams___are___made___on___and___our___little___life___is___rounded___with___sleep';

describe('serializePath', () => {
  it('returns $ for empty path', () => {
    expect(serializePath([])).toBe('$');
  });

  it('serializes a key path segment', () => {
    const path = [{ tag: 0 as const, key: 'foo', index: 0 }];
    expect(serializePath(path)).toBe('$.foo');
  });

  it('serializes nested key path segments', () => {
    const path = [
      { tag: 0 as const, key: 'foo', index: 0 },
      { tag: 0 as const, key: 'bar', index: 0 },
    ];
    expect(serializePath(path)).toBe('$.foo.bar');
  });

  it('serializes an index path segment', () => {
    const path = [{ tag: 1 as const, key: '', index: 2 }];
    expect(serializePath(path)).toBe('$[2]');
  });

  it('serializes mixed key and index segments', () => {
    const path = [
      { tag: 0 as const, key: 'foo', index: 0 },
      { tag: 1 as const, key: '', index: 3 },
      { tag: 0 as const, key: 'bar', index: 0 },
    ];
    expect(serializePath(path)).toBe('$.foo[3].bar');
  });

  it('uses bracket notation for keys with special characters', () => {
    const path = [{ tag: 0 as const, key: 'foo.bar', index: 0 }];
    expect(serializePath(path)).toBe('$["foo.bar"]');
  });

  it('uses bracket notation for keys with spaces', () => {
    const path = [{ tag: 0 as const, key: 'my key', index: 0 }];
    expect(serializePath(path)).toBe('$["my key"]');
  });

  it('serializes a long key with array index', () => {
    const path = [
      { tag: 0 as const, key: COMPLEX_LONG_KEY, index: 0 },
      { tag: 1 as const, key: '', index: 16 },
    ];
    expect(serializePath(path)).toBe(`$.${COMPLEX_LONG_KEY}[16]`);
  });
});

describe('byteOffsetToRowColumn', () => {
  it('returns row 0 column 0 for offset 0', () => {
    const result = byteOffsetToRowColumn('hello\nworld', 0);
    expect(result).toEqual({ row: 0, column: 0 });
  });

  it('returns correct row and column for a multi-line string', () => {
    const result = byteOffsetToRowColumn('hello\nworld', 6);
    expect(result).toEqual({ row: 1, column: 0 });
  });

  it('handles offset at end of a line', () => {
    const result = byteOffsetToRowColumn('abc\ndef', 3);
    expect(result).toEqual({ row: 0, column: 3 });
  });

  it('handles UTF-8 characters that use multiple bytes', () => {
    const result = byteOffsetToRowColumn('héllo', 1);
    expect(result).toEqual({ row: 0, column: 1 });
  });

  it('handles offset beyond text length', () => {
    const result = byteOffsetToRowColumn('hi', 100);
    expect(result.row).toBe(0);
    expect(typeof result.column).toBe('number');
  });

  it('clamps negative offset to 0', () => {
    const result = byteOffsetToRowColumn('hello', -5);
    expect(result).toEqual({ row: 0, column: 0 });
  });
});

describe('byteOffsetToUtf16Offset', () => {
  it('returns 0 for offset 0', () => {
    expect(byteOffsetToUtf16Offset('hello', 0)).toBe(0);
  });

  it('returns correct offset for ASCII text', () => {
    expect(byteOffsetToUtf16Offset('abc', 2)).toBe(2);
  });

  it('handles multi-byte UTF-8 characters', () => {
    // 'é' is 2 bytes in UTF-8, 1 char in UTF-16
    expect(byteOffsetToUtf16Offset('é', 1)).toBe(0);
    expect(byteOffsetToUtf16Offset('é', 2)).toBe(1);
  });

  it('handles emoji (4 bytes UTF-8, 2 surrogates UTF-16)', () => {
    // '😀' is 4 bytes in UTF-8, 2 code units in UTF-16
    expect(byteOffsetToUtf16Offset('😀', 4)).toBe(2);
  });

  it('handles mixed ASCII and multi-byte', () => {
    // 'a'=1b U8/1cu U16, 'é'=2b U8/1cu U16, '😀'=4b U8/2cu U16, total=7 bytes
    expect(byteOffsetToUtf16Offset('aé😀', 7)).toBe(4);
    expect(byteOffsetToUtf16Offset('aé😀', 1)).toBe(1);
  });

  it('clamps to valid range', () => {
    expect(byteOffsetToUtf16Offset('hi', 100)).toBe(2);
  });
});

describe('byteOffsetToUtf16Position', () => {
  it('keeps start/end bias stable inside multibyte characters', () => {
    const segments = buildUtf8ByteSegments('中a');

    expect(byteOffsetToUtf16Position(segments, 1, 'start')).toEqual({ row: 0, column: 0 });
    expect(byteOffsetToUtf16Position(segments, 1, 'end')).toEqual({ row: 0, column: 1 });
  });

  it('resets utf16 column after newline boundaries', () => {
    const segments = buildUtf8ByteSegments('中\n😀');

    expect(byteOffsetToUtf16Position(segments, 4, 'start')).toEqual({ row: 1, column: 0 });
    expect(byteOffsetToUtf16Position(segments, 8, 'end')).toEqual({ row: 1, column: 2 });
  });
});
