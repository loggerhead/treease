// 职责：Worker 侧文档比较 handler：结构化判等、结构化 diff、文本 diff
import { diffStructured, diffText, isStructurallyEqual, type DiffResult } from '@core-wasm/index';
import { createEmptyDiffResult } from '../../shared/brand-bridge';
import type { CompareResponse, WorkerRequest } from './protocol';

function createEqualCompareResponse(mode: CompareResponse['mode']): CompareResponse {
  return { mode, equal: true, result: createEmptyDiffResult() };
}

function createTextCompareResponse(left: string, right: string, result: DiffResult): CompareResponse {
  const hasOnlyWhitespaceChanges = result.pairs.every((pair) => {
    const leftSlice = pair.hasLeft ? left.slice(pair.left.byteOffset, pair.left.byteOffset + pair.left.byteLength) : '';
    const rightSlice = pair.hasRight ? right.slice(pair.right.byteOffset, pair.right.byteOffset + pair.right.byteLength) : '';
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
  message: Extract<WorkerRequest, { type: 'compare' }>,
): Promise<CompareResponse> {
  const left = message.left ?? '';
  const right = message.right ?? '';
  const leftLanguage = message.leftLanguage ?? message.language;
  const rightLanguage = message.rightLanguage ?? message.language;
  const sameLanguage = leftLanguage === rightLanguage;

  if (left === right) {
    return createEqualCompareResponse(sameLanguage ? 'tree' : 'text');
  }

  if (!sameLanguage) {
    return buildTextCompareResponse(left, right);
  }

  try {
    const structuredEqual = await isStructurallyEqual(leftLanguage, left, right);
    if (structuredEqual) {
      return createEqualCompareResponse('tree');
    }
    return { mode: 'tree', equal: false, result: await diffStructured(leftLanguage, left, right) };
  } catch (error) {
    console.warn('[compare] structured compare failed, falling back to text mode', {
      leftLanguage,
      rightLanguage,
      error: error instanceof Error ? error.message : String(error),
    });
  }

  return buildTextCompareResponse(left, right);
}
