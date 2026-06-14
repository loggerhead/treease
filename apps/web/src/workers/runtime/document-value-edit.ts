// 职责：Worker 侧值编辑 handler：applyValueEdit、parseValueForPath
import {
  applyValueEdit as applyValueEditText,
  applyValueEditCanonical as applyValueEditCanonicalText,
  parseValueForPath,
  planGraphValueEdit as planGraphValueEditFromSnapshot,
} from '@core-wasm/index';
import { postOk } from './logging';
import type { PlanGraphValueEditResponse, ReplaceReason, WorkerContext, WorkerRequest } from './protocol';
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
  ctx: WorkerContext,
  message: Extract<WorkerRequest, { type: 'parseValueForPath' }>,
): Promise<void> {
  const tree = await parseValueForPath(
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
  postOk(ctx, message.id, tree);
}

export async function handleValueToTreeNode(
  ctx: WorkerContext,
  message: Extract<WorkerRequest, { type: 'valueToTreeNode' }>,
): Promise<void> {
  postOk(ctx, message.id, await valueToTreeNode(message.value));
}

export async function handleApplyValueEdit(
  ctx: WorkerContext,
  message: Extract<WorkerRequest, { type: 'applyValueEdit' }>,
): Promise<void> {
  const normalizedValue = normalizeEditValue(message.value);
  postOk(
    ctx,
    message.id,
    await applyValueEditText(
      message.language,
      message.text,
      normalizePathSegs(message.path),
      message.preferKey,
      normalizedValue.plainValue,
    ),
  );
}

export async function handleApplyValueEditCanonical(
  ctx: WorkerContext,
  message: Extract<WorkerRequest, { type: 'applyValueEditCanonical' }>,
): Promise<void> {
  const normalizedValue = normalizeEditValue(message.value);
  const result = await applyValueEditCanonicalText(
    message.language,
    message.text,
    normalizePathSegs(message.path),
    message.preferKey,
    normalizedValue.plainValue,
  );
  postOk(ctx, message.id, result);
}

export async function handlePlanGraphValueEdit(
  ctx: WorkerContext,
  message: Extract<WorkerRequest, { type: 'planGraphValueEdit' }>,
): Promise<void> {
  const normalizedPath = normalizePathSegs(message.path);
  const normalizedValue = normalizeEditValue(message.value);
  const { plainValue } = normalizedValue;

  let fallbackReason: ReplaceReason;
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
      postOk(ctx, message.id, {
        mode: 'edits',
        edits: planned.data.edits,
        tree: normalizedValue.tree,
        value: plainValue,
        text: message.text,
      } satisfies PlanGraphValueEditResponse);
      return;
    }
    fallbackReason = planned.status === 'ready' ? (planned.data.reason ?? 'unsupportedEdit') : 'snapshotNotReady';
  } else if (message.snapshotId == null) {
    fallbackReason = 'snapshotNotReady';
  } else {
    fallbackReason = 'unsupportedEdit';
  }
  postOk(
    ctx,
    message.id,
    await createCanonicalReplaceResult(
      message.language,
      message.text,
      normalizedPath,
      message.preferKey,
      plainValue,
      fallbackReason,
    ),
  );
}
