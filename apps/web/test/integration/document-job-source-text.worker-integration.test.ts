import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import {
  buildDocumentJobSettings,
  runTextDocumentJobForGraph,
} from '../../src/lib/graph-stream/document-job-runner';
import { initWasmWorkerForTests, shutdownWasmWorkerForTests } from '../wasm-test-helpers';

const documentJobSettings = buildDocumentJobSettings({
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

describe('document job sourceText worker integration', () => {
  beforeAll(async () => {
    await initWasmWorkerForTests();
  }, 5_000);

  afterAll(async () => {
    await shutdownWasmWorkerForTests();
  });

  it('recursively materializes nested JSON strings into sourceText', async () => {
    const text = JSON.stringify('{"a":1,"b":"{\\"c\\":\\"d\\"}"}');

    const result = await runTextDocumentJobForGraph({
      documentKey: 'vitest://document-job/source-text-recursive-nest',
      language: 'json',
      text,
      settings: documentJobSettings,
      outputAnalysis: true,
      outputGraph: true,
    });

    expect(result.snapshotId).not.toBeNull();
    expect(result.sourceText).toBe('{"a":1,"b":{"c":"d"}}');
    expect(result.analysis).toEqual(
      expect.objectContaining({
        documentKey: 'vitest://document-job/source-text-recursive-nest',
        language: 'json',
        value: null,
      }),
    );
  });
});
