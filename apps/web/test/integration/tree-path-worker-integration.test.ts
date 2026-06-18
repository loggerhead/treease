import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import {
  buildDocumentJobSettings,
  runTextDocumentJobForGraph,
} from '../../src/lib/graph-stream/document-job-runner';
import { resolvePathSpan, resolveTreePath } from '../../src/lib/services/TreePathService';
import { toWasmPathSeg } from '../../src/shared/brand-bridge';
import { initWasmWorkerForTests, shutdownWasmWorkerForTests } from '../wasm-test-helpers';

const COMPLEX_FIXTURE_PATH = join(process.cwd(), '..', '..', 'test', 'fixtures', 'json', 'complex.1.json');
const COMPLEX_LONG_KEY =
  'we___are___such___stuff___as___dreams___are___made___on___and___our___little___life___is___rounded___with___sleep';
const DOCUMENT_JOB_SETTINGS = buildDocumentJobSettings({
  enableNest: true,
  formatSourceOnClose: false,
  formatting: {
    indent: 2,
    smart: true,
    maxLineLength: 100,
    maxInlineComplexity: 1,
    maxArrayInlineItems: 6,
    alignObjectArrays: true,
  },
});

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

async function seedStoredSnapshot(documentKey: string, language: string, text: string) {
  const result = await runTextDocumentJobForGraph({
    documentKey,
    language,
    text,
    settings: DOCUMENT_JOB_SETTINGS,
    outputAnalysis: true,
    outputGraph: true,
  });
  return result.snapshotId;
}

function readComplexFixture() {
  return readFileSync(COMPLEX_FIXTURE_PATH, 'utf-8');
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
    const snapshotId = await seedStoredSnapshot('vitest://tree-path/utf8', 'json', text);
    const model = createModel(text);
    const position = getEditorPosition(text, '值');

    const path = await resolveTreePath(model, position as any, 'vitest://tree-path/utf8', 'json' as any, true, snapshotId);

    expect(Array.isArray(path)).toBe(true);
  });

  it('returns null for a path that does not exist in the stored document', async () => {
    const text = '{"a":1}';
    const snapshotId = await seedStoredSnapshot('vitest://tree-path/missing', 'json', text);
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

  it('resolves top-level key path in complex.1.json fixture', async () => {
    const text = readComplexFixture();
    const snapshotId = await seedStoredSnapshot('vitest://tree-path/complex-simple', 'json', text);

    const path = [toWasmPathSeg({ tag: 0, key: 'empty array', index: 0 })];

    const model = createModel(text);
    const resolved = await resolvePathSpan(
      model,
      path,
      'vitest://tree-path/complex-simple',
      'json' as any,
      'value',
      true,
      snapshotId,
    );

    expect(resolved).not.toBeNull();
  });

  it('resolves deep empty-key path in complex.1.json fixture', async () => {
    const text = readComplexFixture();
    const snapshotId = await seedStoredSnapshot('vitest://tree-path/complex-deep', 'json', text);

    const path = [
      toWasmPathSeg({ tag: 0, key: COMPLEX_LONG_KEY, index: 0 }),
      toWasmPathSeg({ tag: 1, key: '', index: 43 }),
      toWasmPathSeg({ tag: 0, key: '', index: 0 }),
    ];

    const model = createModel(text);
    const resolved = await resolvePathSpan(
      model,
      path,
      'vitest://tree-path/complex-deep',
      'json' as any,
      'value',
      true,
      snapshotId,
    );

    expect(resolved).not.toBeNull();
    expect(resolved!.startByte).toBeLessThan(resolved!.endByte);
  });
});
