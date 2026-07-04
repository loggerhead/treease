import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { DocumentJobAnalysisPayload, EventBatch, SnapshotId, StartDocumentJobResult } from '@core-wasm/index';
import { toWasmBuilderConfig } from '../../shared/brand-bridge';

const mockGetSharedWasmWorkerClient = vi.hoisted(() => vi.fn());

vi.mock('../wasm/wasm-worker-singleton', () => ({
  getSharedWasmWorkerClient: (...args: any[]) => mockGetSharedWasmWorkerClient(...args),
}));

import { commitApplyEdits } from './DocumentCommitService';

const builderConfig = toWasmBuilderConfig({
  keyWidth: 160,
  valueWidth: 240,
  rowHeight: 28,
  rowPaddingX: 12,
});
const documentJobSettings = {
  parser: { enableNest: true, nestMaxDepth: 8 },
  formatting: {
    indent: 2,
    smart: true,
    formatSourceOnClose: true,
    maxLineLength: 100,
    maxInlineComplexity: 1,
    maxArrayInlineItems: 6,
    alignObjectArrays: true,
  },
};


function createRequest() {
  return {
    documentKey: 'doc-1',
    language: 'json',
    text: '{"name":"Alice"}',
    edits: [{ startByte: 8, oldEndByte: 15, newEndByte: 13, text: 'Bob' }] as any,
    baseSnapshotId: 5 as SnapshotId,
    revision: 7,
    settings: documentJobSettings,
    builderConfig,
  };
}

describe('commitApplyEdits', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('rejects edits without a base snapshot', async () => {
    const result = await commitApplyEdits({
      ...createRequest(),
      baseSnapshotId: null,
    });

    expect(mockGetSharedWasmWorkerClient).not.toHaveBeenCalled();
    expect(result).toEqual({
      status: 'rejected',
      snapshotId: null,
      analysis: null,
      sourceText: null,
      jobHandle: 0,
      batch: {
        requestSeq: 0,
        events: [],
        terminal: {
          type: 'rejected',
          code: 'missing_base_snapshot',
          detail: 'ApplyEdits requires an authoritative base snapshot',
        },
      },
    });
  });

  it('merges start and close batches into one commit result', async () => {
    const analysis: DocumentJobAnalysisPayload = {
      tree: null,
      valueJson: null,
      diagnostics: [],
      semanticTokens: { data: [0, 1, 2, 3], version: 2 },
      sourceByteLength: 16,
      language: '',
    };
    const started: StartDocumentJobResult = {
      jobHandle: 41,
      batch: {
        requestSeq: 1,
        events: [{ type: 'progress', processedBytes: 1 }] as any,
        terminal: null,
      },
    };
    const closed: EventBatch = {
      requestSeq: 2,
      events: [{ type: 'snapshotReady', snapshotId: 9, analysis, mainGraph: null }] as any,
      terminal: { type: 'completed' } as any,
    };
    const call = vi.fn().mockResolvedValueOnce(started).mockResolvedValueOnce(closed);

    mockGetSharedWasmWorkerClient.mockResolvedValue({ call });

    const result = await commitApplyEdits(createRequest());

    expect(call).toHaveBeenNthCalledWith(1, 'startDocumentJob', {
      documentKey: 'doc-1',
      language: 'json',
      settings: documentJobSettings,
      outputAnalysis: true,
      outputGraph: true,
      builderConfig,
      baseSnapshotId: 5,
      edits: [{ startByte: 8, oldEndByte: 15, newEndByte: 13, text: 'Bob' }],
    });
    expect(call).toHaveBeenNthCalledWith(2, 'advanceDocumentJob', {
      jobHandle: 41,
      kind: 'close',
    });
    expect(result.snapshotId).toBe(9);
    expect(result.status).toBe('snapshotReady');
    expect(result.sourceText).toBeNull();
    expect(result.jobHandle).toBe(41);
    expect(result.batch).toEqual({
      requestSeq: 2,
      events: [
        { type: 'progress', processedBytes: 1 },
        { type: 'snapshotReady', snapshotId: 9, analysis, mainGraph: null },
      ],
      terminal: { type: 'completed' },
    });
    expect(result.analysis).toEqual({
      documentKey: 'doc-1',
      tree: null,
      value: null,
      diagnostics: [],
      semanticTokens: expect.any(ArrayBuffer),
      semanticTokenVersion: 2,
      sourceByteLength: 16,
      language: 'json',
    });
    expect(Array.from(new Uint32Array(result.analysis!.semanticTokens))).toEqual([0, 1, 2, 3]);
  });
});
