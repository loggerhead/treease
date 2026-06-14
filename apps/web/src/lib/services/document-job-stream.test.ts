import type { EventBatch } from '@core-wasm/index';
import { describe, expect, it, vi } from 'vitest';
import { mergeEventBatches, streamDocumentJobBytes, streamDocumentJobReadable, streamDocumentJobText, type AdvanceDocumentJobRequest } from '../../shared/document-job-stream';

function normalizeAdvanceCalls(advance: { mock: { calls: unknown[][] } }) {
  return advance.mock.calls.map(([input]) => {
    const request = input as { jobHandle: number; kind: string; text?: string; data?: Uint8Array };
    if (request.kind !== 'binaryChunk') return request;
    return {
      jobHandle: request.jobHandle,
      kind: request.kind,
      data: Array.from(request.data ?? []),
    };
  });
}

describe('document-job-stream', () => {
  it('merges event batches while keeping the latest terminal', () => {
    expect(
      mergeEventBatches([
        { requestSeq: 1, events: [{ type: 'progress', processedBytes: 1 }], terminal: null } as any,
        { requestSeq: 1, events: [{ type: 'snapshotReady', snapshotId: 9, analysis: null, mainGraph: null }], terminal: { type: 'completed' } } as any,
      ]),
    ).toEqual({
      requestSeq: 1,
      events: [
        { type: 'progress', processedBytes: 1 },
        { type: 'snapshotReady', snapshotId: 9, analysis: null, mainGraph: null },
      ],
      terminal: { type: 'completed' },
    });
  });

  it('streams utf8 text chunks without corrupting multibyte characters', async () => {
    const advance = vi.fn(async (input: AdvanceDocumentJobRequest) =>
      ({
        requestSeq: 1,
        events:
          input.kind === 'textChunk'
            ? [{ type: 'progress' as const, processedBytes: input.text?.length ?? 0 }]
            : [],
        terminal: input.kind === 'close' ? ({ type: 'completed' as const }) : null,
      }) satisfies EventBatch,
    );

    const batches = await streamDocumentJobText({
      jobHandle: 7,
      text: 'a你b',
      chunkSize: 2,
      advance,
    });

    expect(advance.mock.calls).toEqual([
      [{ jobHandle: 7, kind: 'textChunk', text: 'a' }],
      [{ jobHandle: 7, kind: 'textChunk', text: '你' }],
      [{ jobHandle: 7, kind: 'textChunk', text: 'b' }],
      [{ jobHandle: 7, kind: 'close' }],
    ]);
    expect(batches).toHaveLength(4);
    expect(batches.at(-1)?.terminal).toEqual({ type: 'completed' });
  });

  it('streams byte chunks as binary chunks', async () => {
    const advance = vi.fn(async (input: AdvanceDocumentJobRequest) =>
      ({
        requestSeq: 1,
        events:
          input.kind === 'binaryChunk'
            ? [{ type: 'progress' as const, processedBytes: input.data?.byteLength ?? 0 }]
            : [],
        terminal: input.kind === 'close' ? ({ type: 'completed' as const }) : null,
      }) satisfies EventBatch,
    );

    const encoder = new TextEncoder();
    const bytes = encoder.encode('a你b');
    const batches = await streamDocumentJobBytes({
      jobHandle: 8,
      bytes,
      chunkSize: 2,
      advance,
    });

    expect(normalizeAdvanceCalls(advance)).toEqual([
      { jobHandle: 8, kind: 'binaryChunk', data: Array.from(bytes.subarray(0, 2)) },
      { jobHandle: 8, kind: 'binaryChunk', data: Array.from(bytes.subarray(2, 4)) },
      { jobHandle: 8, kind: 'binaryChunk', data: Array.from(bytes.subarray(4, 5)) },
      { jobHandle: 8, kind: 'close' },
    ]);
    expect(batches).toHaveLength(4);
  });

  it('streams readable chunks as binary chunks and emits close once', async () => {
    const advance = vi.fn(async (input: AdvanceDocumentJobRequest) =>
      ({
        requestSeq: 2,
        events:
          input.kind === 'binaryChunk'
            ? [{ type: 'progress' as const, processedBytes: input.data?.byteLength ?? 0 }]
            : [],
        terminal: input.kind === 'close' ? ({ type: 'completed' as const }) : null,
      }) satisfies EventBatch,
    );
    const encoder = new TextEncoder();
    const readable = new ReadableStream<Uint8Array>({
      start(controller) {
        controller.enqueue(encoder.encode('a'));
        controller.enqueue(encoder.encode('你'));
        controller.enqueue(encoder.encode('b'));
        controller.close();
      },
    });

    const batches = await streamDocumentJobReadable({
      jobHandle: 9,
      readable,
      advance,
    });

    expect(normalizeAdvanceCalls(advance)).toEqual([
      { jobHandle: 9, kind: 'binaryChunk', data: Array.from(encoder.encode('a')) },
      { jobHandle: 9, kind: 'binaryChunk', data: Array.from(encoder.encode('你')) },
      { jobHandle: 9, kind: 'binaryChunk', data: Array.from(encoder.encode('b')) },
      { jobHandle: 9, kind: 'close' },
    ]);
    expect(batches.at(-1)?.terminal).toEqual({ type: 'completed' });
  });

  it('slices oversized readable chunks before binary advance', async () => {
    const advance = vi.fn(async (input: AdvanceDocumentJobRequest) =>
      ({
        requestSeq: 3,
        events:
          input.kind === 'binaryChunk'
            ? [{ type: 'progress' as const, processedBytes: input.data?.byteLength ?? 0 }]
            : [],
        terminal: input.kind === 'close' ? ({ type: 'completed' as const }) : null,
      }) satisfies EventBatch,
    );
    const encoder = new TextEncoder();
    const readable = new ReadableStream<Uint8Array>({
      start(controller) {
        controller.enqueue(encoder.encode('abcdefg'));
        controller.close();
      },
    });

    const batches = await streamDocumentJobReadable({
      jobHandle: 10,
      readable,
      advance,
      chunkSize: 3,
    });

    expect(normalizeAdvanceCalls(advance)).toEqual([
      { jobHandle: 10, kind: 'binaryChunk', data: Array.from(encoder.encode('abc')) },
      { jobHandle: 10, kind: 'binaryChunk', data: Array.from(encoder.encode('def')) },
      { jobHandle: 10, kind: 'binaryChunk', data: Array.from(encoder.encode('g')) },
      { jobHandle: 10, kind: 'close' },
    ]);
    expect(batches.at(-1)?.terminal).toEqual({ type: 'completed' });
  });

  it('coalesces readable bytes up to the requested binary chunk size', async () => {
    const advance = vi.fn(async (input: AdvanceDocumentJobRequest) =>
      ({
        requestSeq: 30,
        events:
          input.kind === 'binaryChunk'
            ? [{ type: 'progress' as const, processedBytes: input.data?.byteLength ?? 0 }]
            : [],
        terminal: input.kind === 'close' ? ({ type: 'completed' as const }) : null,
      }) satisfies EventBatch,
    );
    const encoder = new TextEncoder();
    const readable = new ReadableStream<Uint8Array>({
      start(controller) {
        controller.enqueue(encoder.encode('ab'));
        controller.enqueue(encoder.encode('cd'));
        controller.enqueue(encoder.encode('ef'));
        controller.close();
      },
    });

    await streamDocumentJobReadable({
      jobHandle: 30,
      readable,
      advance,
      chunkSize: 4,
    });

    expect(normalizeAdvanceCalls(advance)).toEqual([
      { jobHandle: 30, kind: 'binaryChunk', data: Array.from(encoder.encode('abcd')) },
      { jobHandle: 30, kind: 'binaryChunk', data: Array.from(encoder.encode('ef')) },
      { jobHandle: 30, kind: 'close' },
    ]);
  });

  it('streams async iterable chunks as binary chunks and emits close once', async () => {
    const advance = vi.fn(async (input: AdvanceDocumentJobRequest) =>
      ({
        requestSeq: 3,
        events:
          input.kind === 'binaryChunk'
            ? [{ type: 'progress' as const, processedBytes: input.data?.byteLength ?? 0 }]
            : [],
        terminal: input.kind === 'close' ? ({ type: 'completed' as const }) : null,
      }) satisfies EventBatch,
    );
    const encoder = new TextEncoder();
    async function* chunks() {
      yield encoder.encode('a');
      yield encoder.encode('你');
      yield encoder.encode('b');
    }

    const batches = await streamDocumentJobReadable({
      jobHandle: 10,
      readable: chunks(),
      advance,
    });

    expect(normalizeAdvanceCalls(advance)).toEqual([
      { jobHandle: 10, kind: 'binaryChunk', data: Array.from(encoder.encode('a')) },
      { jobHandle: 10, kind: 'binaryChunk', data: Array.from(encoder.encode('你')) },
      { jobHandle: 10, kind: 'binaryChunk', data: Array.from(encoder.encode('b')) },
      { jobHandle: 10, kind: 'close' },
    ]);
    expect(batches.at(-1)?.terminal).toEqual({ type: 'completed' });
  });

  it('closes empty text streams without emitting text chunks', async () => {
    const advance = vi.fn(async (input: AdvanceDocumentJobRequest) =>
      ({
        requestSeq: 4,
        events: [],
        terminal: input.kind === 'close' ? ({ type: 'completed' as const }) : null,
      }) satisfies EventBatch,
    );

    const batches = await streamDocumentJobText({
      jobHandle: 11,
      text: '',
      advance,
    });

    expect(advance.mock.calls).toEqual([[{ jobHandle: 11, kind: 'close' }]]);
    expect(batches).toEqual([
      {
        requestSeq: 4,
        events: [],
        terminal: { type: 'completed' },
      },
    ]);
  });
});
