import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { analyzeDocumentAndStore } from '../../src/lib/services/EditorDiagnostics';
import { resolvePathSpan, resolveTreePath } from '../../src/lib/services/TreePathService';
import { toWasmPathSeg } from '../../src/shared/brand-bridge';
import { initWasmWorkerForTests, shutdownWasmWorkerForTests } from '../wasm-test-helpers';

function createModel(text: string) {
  const lines = text.split('\n');
  return {
    getValue: () => text,
    getLineContent: (lineNumber: number) => lines[lineNumber - 1] ?? '',
  } as any;
}

function getEditorPosition(text: string, needle: string, occurrence = 0) {
  let index = -1;
  let fromIndex = 0;
  for (let i = 0; i <= occurrence; i += 1) {
    index = text.indexOf(needle, fromIndex);
    if (index === -1) {
      throw new Error(`needle not found: ${needle}`);
    }
    fromIndex = index + needle.length;
  }
  const before = text.slice(0, index);
  const lines = before.split('\n');
  return {
    lineNumber: lines.length,
    column: (lines[lines.length - 1]?.length ?? 0) + 1,
  };
}

async function seedStoredAnalysis(documentKey: string, language: string, text: string) {
  const analysis = await analyzeDocumentAndStore(language as any, text, documentKey, true);
  return analysis?.snapshotId ?? null;
}

describe('TreePathService worker integration', () => {
  beforeAll(async () => {
    await initWasmWorkerForTests();
  }, 5_000);

  afterAll(async () => {
    await shutdownWasmWorkerForTests();
  });

  it('resolves UTF-8 positions through the real worker', async () => {
    const text = '{"你":"值"}';
    const snapshotId = await seedStoredAnalysis('vitest://tree-path/utf8', 'json', text);
    const model = createModel(text);
    const position = getEditorPosition(text, '值');

    const path = await resolveTreePath(model, position as any, 'vitest://tree-path/utf8', 'json' as any, true, snapshotId);

    expect(Array.isArray(path)).toBe(true);
  });

  it('returns null for a path that does not exist in the stored document', async () => {
    const text = '{"a":1}';
    const snapshotId = await seedStoredAnalysis('vitest://tree-path/missing', 'json', text);
    const model = createModel(text);

    const resolved = await resolvePathSpan(
      model,
      [toWasmPathSeg({ tag: 0, key: 'missing', index: 0 })],
      'vitest://tree-path/missing',
      'json' as any,
      'value',
      true,
      snapshotId,
    );

    expect(resolved).toBeNull();
  });
});
