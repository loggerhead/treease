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
      strictSourceLiteral: message.strictSourceLiteral,
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
      rawReplacement: message.rawReplacement,
    });
    if (planned.status === 'ready' && planned.data.mode === 'edits' && planned.data.edits.length > 0) {
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
