import { PathSegTag, type PathSeg } from '@core-wasm/index';
import { pathSegKeyValue } from './path';

const textEncoder = new TextEncoder();

export type Utf8ByteSegment = {
  startByte: number;
  endByte: number;
  row: number;
  startUtf16Column: number;
  endUtf16Column: number;
  endsWithNewline: boolean;
};

export type ByteOffsetBias = 'start' | 'end';

function isPathSegIndex(seg: PathSeg): boolean {
  return seg.tag === PathSegTag.INDEX;
}

/**
 * Serialize a path segment array to a dot/bracket notation string.
 *
 * Examples:
 *   []                     → "$"
 *   [{key:"foo"}]          → "$.foo"
 *   [{key:"foo.bar"}]      → `$["foo.bar"]`
 *   [{index:2}]            → "$[2]"
 *   [{key:"a"},{index:1}]  → "$.a[1]"
 */
export function serializePath(path: PathSeg[]): string {
  if (!path.length) return '$';
  return path.reduce<string>((acc, segment) => {
    if (isPathSegIndex(segment)) {
      return `${acc}[${segment.index ?? 0}]`;
    }
    const key = pathSegKeyValue(segment);
    return /^[A-Za-z0-9_$]+$/.test(key) ? `${acc}.${key}` : `${acc}[${JSON.stringify(key)}]`;
  }, '$');
}

function utf8ByteLength(char: string): number {
  const codePoint = char.codePointAt(0) ?? 0;
  if (codePoint <= 0x7f) return 1;
  if (codePoint <= 0x7ff) return 2;
  if (codePoint <= 0xffff) return 3;
  return 4;
}

export function buildUtf8ByteSegments(text: string): Utf8ByteSegment[] {
  const segments: Utf8ByteSegment[] = [];
  let byteOffset = 0;
  let row = 0;
  let utf16Column = 0;

  for (const char of text) {
    const startByte = byteOffset;
    const startUtf16Column = utf16Column;
    byteOffset += utf8ByteLength(char);
    if (char === '\n') {
      segments.push({
        startByte,
        endByte: byteOffset,
        row,
        startUtf16Column,
        endUtf16Column: utf16Column,
        endsWithNewline: true,
      });
      row += 1;
      utf16Column = 0;
      continue;
    }
    utf16Column += char.length;
    segments.push({
      startByte,
      endByte: byteOffset,
      row,
      startUtf16Column,
      endUtf16Column: utf16Column,
      endsWithNewline: false,
    });
  }

  return segments;
}

export function byteOffsetToUtf16Position(
  segments: Utf8ByteSegment[],
  byteOffset: number,
  bias: ByteOffsetBias,
): { row: number; column: number } {
  const target = Math.max(0, byteOffset);
  if (segments.length === 0) return { row: 0, column: 0 };

  for (const segment of segments) {
    if (target <= segment.startByte) {
      return { row: segment.row, column: segment.startUtf16Column };
    }
    if (target < segment.endByte) {
      return bias === 'start'
        ? { row: segment.row, column: segment.startUtf16Column }
        : { row: segment.row, column: segment.endUtf16Column };
    }
    if (target === segment.endByte) {
      if (segment.endsWithNewline) {
        return { row: segment.row + 1, column: 0 };
      }
      return { row: segment.row, column: segment.endUtf16Column };
    }
  }

  const last = segments[segments.length - 1];
  return { row: last.row, column: last.endUtf16Column };
}

/**
 * Convert a byte offset in text to 0-based row and column.
 */
export function byteOffsetToRowColumn(text: string, byteOffset: number): { row: number; column: number } {
  const safe = Math.max(0, byteOffset);
  const lines = text.split('\n');
  let offset = 0;
  for (let row = 0; row < lines.length; row++) {
    const lineBytes = textEncoder.encode(lines[row] ?? '').length;
    if (safe <= offset + lineBytes) {
      return { row, column: Math.max(0, safe - offset) };
    }
    offset += lineBytes + 1;
  }
  return {
    row: Math.max(0, lines.length - 1),
    column: Math.max(0, safe - offset),
  };
}

/**
 * Convert a byte offset to a UTF-16 code unit offset.
 */
export function byteOffsetToUtf16Offset(text: string, byteOffset: number): number {
  const safe = Math.max(0, byteOffset);
  let utf16Offset = 0;
  let bytesSeen = 0;
  for (const char of text) {
    const charBytes = utf8ByteLength(char);
    if (bytesSeen + charBytes > safe) return utf16Offset;
    bytesSeen += charBytes;
    utf16Offset += char.length;
  }
  return utf16Offset;
}
