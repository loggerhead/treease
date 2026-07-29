// Responsibility: Worker-side value-edit handler for applyValueEditCanonical and parseValueForPath.
import {
  applyValueEditCanonical as applyValueEditCanonicalText,
  parseValueForPath,
  planGraphValueEdit as planGraphValueEditFromSnapshot,
} from '@core-wasm/index';
import type { PlanGraphValueEditResponse, ReplaceReason, WorkerRequest } from './protocol';
import { normalizePathSegs } from './tree-path';
import { treeNodeToValue, valueToTreeNode } from '../../shared/tree-node-value';

function isTreeNodeLike(value: unknown): value is { kind: number; semType: number; children: unknown[] } {
  if (!value || typeof value !== 'object') return false;
  const node = value as { kind?: unknown; semType?: unknown; children?: unknown };
  return typeof node.kind === 'number' && typeof node.semType === 'number' && Array.isArray(node.children);
}

function normalizeEditValue(value: unknown) {
  const plainValue = isTreeNodeLike(value) ? treeNodeToValue(value as any) : value;
  return {
    plainValue,
    rawStringValue: typeof plainValue === 'string' ? plainValue : null,
    tree: valueToTreeNode(plainValue),
  };
}

function applyPlannedEditsToText(text: string, edits: Array<{ startByte: number; oldEndByte: number; text: string }>): string | null {
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  let bytes = encoder.encode(text);
  const ordered = [...edits].sort((left, right) => right.startByte - left.startByte);
  for (const edit of ordered) {
    if (edit.startByte < 0 || edit.oldEndByte < edit.startByte || edit.oldEndByte > bytes.length) return null;
    const next = new Uint8Array(bytes.length - (edit.oldEndByte - edit.startByte) + encoder.encode(edit.text).length);
    next.set(bytes.subarray(0, edit.startByte));
    next.set(encoder.encode(edit.text), edit.startByte);
    next.set(bytes.subarray(edit.oldEndByte), edit.startByte + encoder.encode(edit.text).length);
    bytes = next;
  }
  return decoder.decode(bytes);
}

function positionForOffset(text: string, offset: number): { row: number; column: number } {
  const prefix = text.slice(0, offset);
  const lastNewline = prefix.lastIndexOf('\n');
  return {
    row: (prefix.match(/\n/g) ?? []).length,
    column: new TextEncoder().encode(prefix.slice(lastNewline + 1)).length,
  };
}

function createCanonicalTextEdit(before: string, after: string) {
  let prefixEnd = 0;
  const sharedLength = Math.min(before.length, after.length);
  while (prefixEnd < sharedLength && before[prefixEnd] === after[prefixEnd]) prefixEnd += 1;
  let beforeSuffixStart = before.length;
  let afterSuffixStart = after.length;
  while (
    beforeSuffixStart > prefixEnd &&
    afterSuffixStart > prefixEnd &&
    before[beforeSuffixStart - 1] === after[afterSuffixStart - 1]
  ) {
    beforeSuffixStart -= 1;
    afterSuffixStart -= 1;
  }
  const encoder = new TextEncoder();
  const startByte = encoder.encode(before.slice(0, prefixEnd)).length;
  const oldEndByte = encoder.encode(before.slice(0, beforeSuffixStart)).length;
  const newEndByte = encoder.encode(after.slice(0, afterSuffixStart)).length;
  const start = positionForOffset(before, prefixEnd);
  const oldEnd = positionForOffset(before, beforeSuffixStart);
  const newEnd = positionForOffset(after, afterSuffixStart);
  return {
    startByte,
    oldEndByte,
    newEndByte,
    startRow: start.row,
    startColumn: start.column,
    oldEndRow: oldEnd.row,
    oldEndColumn: oldEnd.column,
    newEndRow: newEnd.row,
    newEndColumn: newEnd.column,
    text: after.slice(prefixEnd, afterSuffixStart),
  };
}
async function createCanonicalReplaceResult(
  language: string,
  text: string,
  path: unknown,
  preferKey: boolean,
  value: unknown,
  reason: ReplaceReason,
): Promise<PlanGraphValueEditResponse> {
  const result = await applyValueEditCanonicalText(language, text, path as any, preferKey, value);
  return {
    mode: 'replace',
    reason,
    text: result.text,
    tree: result.tree,
    value: result.value,
  };
}
export async function handleParseValueForPath(
  message: Extract<WorkerRequest, { type: 'parseValueForPath' }>,
): Promise<Awaited<ReturnType<typeof parseValueForPath>>> {
  return parseValueForPath(
    message.language,
    message.documentKey,
    message.text,
    message.path,
    message.rawEdit,
    message.preferKey,
    {
      nest: message.nest,
    },
  );
}

export async function handleValueToTreeNode(
  message: Extract<WorkerRequest, { type: 'valueToTreeNode' }>,
): Promise<Awaited<ReturnType<typeof valueToTreeNode>>> {
  return valueToTreeNode(message.value);
}

export async function handleApplyValueEditCanonical(
  message: Extract<WorkerRequest, { type: 'applyValueEditCanonical' }>,
): Promise<Awaited<ReturnType<typeof applyValueEditCanonicalText>>> {
  const normalizedValue = normalizeEditValue(message.value);
  const result = await applyValueEditCanonicalText(
    message.language,
    message.text,
    normalizePathSegs(message.path),
    message.preferKey,
    normalizedValue.plainValue,
  );
  return result;
}

export async function handlePlanGraphValueEdit(
  message: Extract<WorkerRequest, { type: 'planGraphValueEdit' }>,
): Promise<PlanGraphValueEditResponse> {
  const normalizedPath = normalizePathSegs(message.path);
  const normalizedValue = normalizeEditValue(message.value);
  const { plainValue } = normalizedValue;

  if (message.documentKey && message.snapshotId != null) {
    const planned = await planGraphValueEditFromSnapshot({
      documentKey: message.documentKey,
      snapshotId: message.snapshotId,
      language: message.language,
      path: normalizedPath,
      preferKey: message.preferKey,
      value: plainValue,
    });
    if (planned.status === 'ready' && planned.data.mode === 'edits' && planned.data.edits.length > 0) {
      if (message.verifyText) {
        const canonical = await applyValueEditCanonicalText(
          message.language,
          message.text,
          normalizedPath as any,
          message.preferKey,
          plainValue,
        );
        if (applyPlannedEditsToText(message.text, planned.data.edits) !== canonical.text) {
          return {
            mode: 'edits',
            edits: [createCanonicalTextEdit(message.text, canonical.text)],
            text: canonical.text,
            tree: canonical.tree,
            value: canonical.value,
          } satisfies PlanGraphValueEditResponse;
        }
      }
      return {
        mode: 'edits',
        edits: planned.data.edits,
        tree: normalizedValue.tree,
        value: plainValue,
        text: message.text,
      } satisfies PlanGraphValueEditResponse;
    }
    if (planned.status !== 'ready') {
      return { mode: 'snapshotNotReady' } satisfies PlanGraphValueEditResponse;
    }
    if (planned.data.reason === 'snapshotNotReady') {
      return { mode: 'snapshotNotReady' } satisfies PlanGraphValueEditResponse;
    }
    const fallbackReason: ReplaceReason = planned.data.reason ?? 'unsupportedEdit';
    return createCanonicalReplaceResult(
        message.language,
        message.text,
        normalizedPath,
        message.preferKey,
        plainValue,
        fallbackReason,
    );
  } else if (message.snapshotId == null) {
    return { mode: 'snapshotNotReady' } satisfies PlanGraphValueEditResponse;
  }
  return createCanonicalReplaceResult(
      message.language,
      message.text,
      normalizedPath,
      message.preferKey,
      plainValue,
      'unsupportedEdit',
  );
}
