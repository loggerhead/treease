import type { DocumentTextEdit } from '@core-wasm/index';

const encoder = new TextEncoder();
const decoder = new TextDecoder();
const MULTI_RANGE_SHARED_RUN_THRESHOLD = 2;

export type MonacoTextChange = { rangeOffset: number; rangeLength: number; text: string };

function computePoint(bytes: Uint8Array, offset: number) {
  let row = 0;
  let col = 0;
  for (let i = 0; i < offset && i < bytes.length; i += 1) {
    if (bytes[i] === 10) {
      row += 1;
      col = 0;
    } else if (bytes[i] !== 13) {
      col += 1;
    }
  }
  return { row, col };
}

type SingleEditWindow = {
  prefix: number;
  suffix: number;
  startByte: number;
  oldEndByte: number;
  newEndByte: number;
};

function computeSingleEditWindow(prev: Uint8Array, next: Uint8Array): SingleEditWindow {
  const minLen = Math.min(prev.length, next.length);
  let prefix = 0;
  while (prefix < minLen && prev[prefix] === next[prefix]) prefix += 1;
  let suffix = 0;
  while (suffix < minLen - prefix && prev[prev.length - 1 - suffix] === next[next.length - 1 - suffix]) suffix += 1;
  return {
    prefix,
    suffix,
    startByte: prefix,
    oldEndByte: prev.length - suffix,
    newEndByte: next.length - suffix,
  };
}

function hasSharedInteriorRun(a: Uint8Array, b: Uint8Array, threshold: number): boolean {
  if (threshold <= 0 || a.length < threshold || b.length < threshold) return false;
  const previous = new Uint16Array(b.length + 1);
  const current = new Uint16Array(b.length + 1);
  for (let i = 0; i < a.length; i += 1) {
    current.fill(0);
    for (let j = 0; j < b.length; j += 1) {
      if (a[i] !== b[j]) continue;
      current[j + 1] = previous[j] + 1;
      if (current[j + 1] >= threshold) return true;
    }
    previous.set(current);
  }
  return false;
}

export function canRepresentAsSingleDocumentTextEdit(prev: Uint8Array, next: Uint8Array): boolean {
  const { prefix, suffix } = computeSingleEditWindow(prev, next);
  const oldMiddle = prev.subarray(prefix, prev.length - suffix);
  const newMiddle = next.subarray(prefix, next.length - suffix);
  if (oldMiddle.length === 0 || newMiddle.length === 0) return true;
  return !hasSharedInteriorRun(oldMiddle, newMiddle, MULTI_RANGE_SHARED_RUN_THRESHOLD);
}

export function computeSingleDocumentTextEdit(prev: Uint8Array, next: Uint8Array): DocumentTextEdit {
  const { startByte, oldEndByte, newEndByte } = computeSingleEditWindow(prev, next);
  const startPoint = computePoint(prev, startByte);
  const oldEndPoint = computePoint(prev, oldEndByte);
  const newEndPoint = computePoint(next, newEndByte);
  return {
    startByte,
    oldEndByte,
    newEndByte,
    startRow: startPoint.row,
    startColumn: startPoint.col,
    oldEndRow: oldEndPoint.row,
    oldEndColumn: oldEndPoint.col,
    newEndRow: newEndPoint.row,
    newEndColumn: newEndPoint.col,
    text: decoder.decode(next.subarray(startByte, newEndByte)),
  };
}

export function monacoChangesToDocumentTextEdits(
  prevBytes: Uint8Array,
  nextBytes: Uint8Array,
  changes: MonacoTextChange[],
): DocumentTextEdit[] {
  if (!changes || changes.length === 0) return [];
  const prevText = decoder.decode(prevBytes);
  const ordered = [...changes].sort((a, b) => a.rangeOffset - b.rangeOffset);
  let deltaBytes = 0;
  let cursorOffset = 0;
  let cursorBytes = 0;

  const advanceTo = (offset: number) => {
    if (offset <= cursorOffset) return cursorBytes;
    const delta = prevText.slice(cursorOffset, offset);
    cursorBytes += encoder.encode(delta).length;
    cursorOffset = offset;
    return cursorBytes;
  };

  return ordered.map((change) => {
    const startByteOld = advanceTo(change.rangeOffset);
    const oldEndByte = advanceTo(change.rangeOffset + change.rangeLength);
    const insertedBytesLen = encoder.encode(change.text).length;
    const startByteNew = startByteOld + deltaBytes;
    const newEndByte = startByteNew + insertedBytesLen;
    deltaBytes += insertedBytesLen - (oldEndByte - startByteOld);

    const startPoint = computePoint(prevBytes, startByteOld);
    const oldEndPoint = computePoint(prevBytes, oldEndByte);
    const newEndPoint = computePoint(nextBytes, newEndByte);
    return {
      startByte: startByteOld,
      oldEndByte,
      newEndByte,
      startRow: startPoint.row,
      startColumn: startPoint.col,
      oldEndRow: oldEndPoint.row,
      oldEndColumn: oldEndPoint.col,
      newEndRow: newEndPoint.row,
      newEndColumn: newEndPoint.col,
      text: change.text,
    };
  });
}
