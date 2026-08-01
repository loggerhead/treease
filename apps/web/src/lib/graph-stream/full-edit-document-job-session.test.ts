import { beforeEach, describe, expect, it, vi } from 'vitest';

const mockWorkerCall = vi.hoisted(() => vi.fn());

vi.mock('../wasm/wasm-worker-singleton', () => ({
  getSharedWasmWorkerClient: vi.fn(async () => ({ call: mockWorkerCall })),
}));

import {
  clearFullEditDocumentJobSession,
  getFullEditDocumentJobSession,
  startFullEditDocumentJobSessionForGraph,
} from './full-edit-document-job-session';
const documentJobSettings = {
  parser: { enableNest: false, nestMaxDepth: 8 },
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


describe('full-edit-document-job-session', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    clearFullEditDocumentJobSession('session-1');
  });

  it('registers a readable document job session and replays its batches to late graph consumers', async () => {
    mockWorkerCall.mockImplementation(async (method: string, input: any) => {
      if (method === 'startDocumentJob') {
        return {
          jobHandle: 7,
          batch: { requestSeq: 0, events: [], terminal: null },
        };
      }
      if (method === 'advanceDocumentJob' && input.kind === 'binaryChunk') {
        return {
          requestSeq: 1,
          events: [{ type: 'projectionDelta', clear: true, graphData: null }],
          terminal: null,
        };
      }
      if (method === 'advanceDocumentJob' && input.kind === 'close') {
        return {
          requestSeq: 2,
          events: [{ type: 'snapshotReady', snapshotId: 9, analysis: null, mainGraph: null }],
          terminal: { type: 'completed' },
        };
      }
      throw new Error(`unexpected worker call: ${method}`);
    });

    const encoder = new TextEncoder();
    const session = startFullEditDocumentJobSessionForGraph({
      sessionId: 'session-1',
      documentKey: 'doc-1',
      revision: 3,
      language: 'json',
      readable: (async function* () {
        yield encoder.encode('{"a":1}');
      })(),
      settings: documentJobSettings,
      totalBytes: 7,
      chunkSize: 64,
    });

    expect(getFullEditDocumentJobSession('session-1')).toBe(session);

    const result = await session.result;
    const replayed = [];
    for await (const batch of session.batches()) {
      replayed.push(batch);
    }

    expect(result.snapshotId).toBe(9);
    expect(session.hasCompleteReplay()).toBe(true);
    expect(replayed).toHaveLength(3);
    expect(replayed[1]?.events[0]).toMatchObject({ type: 'projectionDelta' });
    expect(mockWorkerCall).toHaveBeenNthCalledWith(1, 'startDocumentJob', expect.objectContaining({
      documentKey: 'doc-1',
      language: 'json',
      outputGraph: true,
      outputAnalysis: true,
      settings: documentJobSettings,
    }));
    expect(mockWorkerCall).toHaveBeenNthCalledWith(2, 'advanceDocumentJob', {
      jobHandle: 7,
      kind: 'binaryChunk',
      data: encoder.encode('{"a":1}'),
    });
    expect(mockWorkerCall).toHaveBeenNthCalledWith(3, 'advanceDocumentJob', {
      jobHandle: 7,
      kind: 'close',
    });
  });

  it('keeps only a bounded replay window for late graph consumers', async () => {
    let requestSeq = 0;
    mockWorkerCall.mockImplementation(async (method: string, input: any) => {
      if (method === 'startDocumentJob') {
        return {
          jobHandle: 8,
          batch: { requestSeq: requestSeq++, events: [], terminal: null },
        };
      }
      if (method === 'advanceDocumentJob' && input.kind === 'binaryChunk') {
        return {
          requestSeq: requestSeq++,
          events: [{ type: 'progress', processedBytes: requestSeq }],
          terminal: null,
        };
      }
      if (method === 'advanceDocumentJob' && input.kind === 'close') {
        return {
          requestSeq: requestSeq++,
          events: [{ type: 'snapshotReady', snapshotId: 10, analysis: null, mainGraph: null }],
          terminal: { type: 'completed' },
        };
      }
      throw new Error(`unexpected worker call: ${method}`);
    });

    const session = startFullEditDocumentJobSessionForGraph({
      sessionId: 'session-1',
      documentKey: 'doc-1',
      revision: 4,
      language: 'json',
      readable: (async function* () {
        for (let index = 0; index < 20; index += 1) {
          yield new Uint8Array([index]);
        }
      })(),
      settings: documentJobSettings,
      totalBytes: 20,
      chunkSize: 1,
    });

    await session.result;
    const replayed = [];
    for await (const batch of session.batches()) replayed.push(batch);

    expect(replayed.length).toBeLessThanOrEqual(8);
    expect(session.hasCompleteReplay()).toBe(false);
    expect(replayed.at(-1)?.events.at(-1)).toMatchObject({ type: 'snapshotReady', snapshotId: 10 });
  });

  it('runs memory full-edit text through the same session path', async () => {
    mockWorkerCall.mockImplementation(async (method: string, input: any) => {
      if (method === 'startDocumentJob') {
        return { jobHandle: 11, batch: { requestSeq: 0, events: [], terminal: null } };
      }
      if (method === 'advanceDocumentJob' && input.kind === 'textChunk') {
        return { requestSeq: 1, events: [], terminal: null };
      }
      if (method === 'advanceDocumentJob' && input.kind === 'close') {
        return {
          requestSeq: 2,
          events: [{ type: 'snapshotReady', snapshotId: 12, analysis: null, mainGraph: null }],
          terminal: { type: 'completed' },
        };
      }
      throw new Error(`unexpected worker call: ${method}`);
    });

    const session = startFullEditDocumentJobSessionForGraph({
      sessionId: 'session-1',
      documentKey: 'doc-1',
      revision: 5,
      language: 'json',
      text: '{"a":1}',
      settings: documentJobSettings,
      totalBytes: 7,
    });

    const result = await session.result;
    expect(result.snapshotId).toBe(12);
    expect(mockWorkerCall).toHaveBeenNthCalledWith(2, 'advanceDocumentJob', expect.objectContaining({
      jobHandle: 11,
      kind: 'textChunk',
      text: '{"a":1}',
    }));
  });
});
