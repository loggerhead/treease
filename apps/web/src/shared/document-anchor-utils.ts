import { PathSegTag, type PathSeg } from '@core-wasm/index';
import { pathSegKeyValue } from './path';

const textEncoder = new TextEncoder();

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
    const charBytes = textEncoder.encode(char).length;
    if (bytesSeen + charBytes > safe) return utf16Offset;
    bytesSeen += charBytes;
    utf16Offset += char.length;
  }
  return utf16Offset;
}
