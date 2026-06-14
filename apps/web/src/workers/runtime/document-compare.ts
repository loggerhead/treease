// 职责：Worker 侧文档比较 handler：compareStructured、diffText
import { compareStructured, diffText, type DiffResult } from '@core-wasm/index';
import { createEmptyDiffResult } from '../../shared/brand-bridge';
import { postOk } from './logging';
import type { CompareResponse, WorkerContext, WorkerRequest } from './protocol';

function createEqualCompareResponse(mode: CompareResponse['mode']): CompareResponse {
  return { mode, equal: true, result: createEmptyDiffResult() };
}

function createTextCompareResponse(left: string, right: string, result: DiffResult): CompareResponse {
  const hasOnlyWhitespaceChanges = result.pairs.every((pair) => {
    const leftSlice = pair.hasLeft ? left.slice(pair.left.offset, pair.left.offset + pair.left.length) : '';
    const rightSlice = pair.hasRight ? right.slice(pair.right.offset, pair.right.offset + pair.right.length) : '';
    return leftSlice.trim().length === 0 && rightSlice.trim().length === 0;
  });

  if (hasOnlyWhitespaceChanges) {
    return createEqualCompareResponse('text');
  }

  return { mode: 'text', equal: result.pairs.length === 0, result };
}

async function buildTextCompareResponse(left: string, right: string): Promise<CompareResponse> {
  return createTextCompareResponse(left, right, await diffText(left, right));
}

export async function handleCompare(
  ctx: WorkerContext,
  message: Extract<WorkerRequest, { type: 'compare' }>,
): Promise<void> {
  const left = message.left ?? '';
  const right = message.right ?? '';
  const leftLanguage = message.leftLanguage ?? message.language;
  const rightLanguage = message.rightLanguage ?? message.language;
  const sameLanguage = leftLanguage === rightLanguage;

  if (left === right) {
    postOk(ctx, message.id, createEqualCompareResponse(sameLanguage ? 'tree' : 'text'));
    return;
  }

  if (!sameLanguage) {
    postOk(ctx, message.id, await buildTextCompareResponse(left, right));
    return;
  }

  try {
    const structuredEqual = await compareStructured(leftLanguage, left, right);
    if (structuredEqual) {
      postOk(ctx, message.id, createEqualCompareResponse('tree'));
      return;
    }
  } catch (error) {
    console.warn('[compare] structured compare failed, falling back to text mode', {
      leftLanguage,
      rightLanguage,
      error: error instanceof Error ? error.message : String(error),
    });
  }

  postOk(ctx, message.id, await buildTextCompareResponse(left, right));
}
