import type { EventBatch } from '@core-wasm/index';
import { getStreamChunkSize } from '../lib/config/constants';

const textEncoder = new TextEncoder();

export type AdvanceDocumentJobRequest =
  | { jobHandle: number; kind: 'textChunk'; text: string }
  | { jobHandle: number; kind: 'binaryChunk'; data: Uint8Array }
  | { jobHandle: number; kind: 'poll' }
  | { jobHandle: number; kind: 'close' };

export type AdvanceDocumentJobFn = (input: AdvanceDocumentJobRequest) => Promise<EventBatch>;
export type DocumentJobBatchListener = (batch: EventBatch) => Promise<void> | void;
export type DocumentJobBinaryChunkListener = (chunk: Uint8Array) => Promise<void> | void;

type DocumentJobTerminalEvent = Extract<
  EventBatch['events'][number],
  { type: 'snapshotReady' | 'parseFailed' }
>;

function createDocumentJobBatchAccumulator() {
  let requestSeq = 0;
  let terminal: EventBatch['terminal'] = null;
  let terminalEvent: DocumentJobTerminalEvent | null = null;
  return {
    append(batch: EventBatch): void {
      requestSeq = batch.requestSeq;
      terminal = batch.terminal ?? terminal;
      for (const event of batch.events) {
        if (event.type === 'snapshotReady' || event.type === 'parseFailed') {
          terminalEvent = event;
        }
      }
    },
    result(): EventBatch {
      return {
        requestSeq,
        events: terminalEvent ? [terminalEvent] : [],
        terminal,
      };
    },
  };
}

export function mergeEventBatches(batches: EventBatch[]): EventBatch {
  const terminal = batches.reduce<EventBatch['terminal']>(
    (current, batch) => batch.terminal ?? current,
    null,
  );
  return {
    requestSeq: batches[batches.length - 1]?.requestSeq ?? 0,
    events: batches.flatMap((batch) => batch.events),
    terminal,
  };
}

function* splitTextIntoUtf8Chunks(text: string, chunkSize: number): Generator<string, void, void> {
  if (!text) return;
  const bytes = textEncoder.encode(text);
  const decoder = new TextDecoder();

  for (let offset = 0; offset < bytes.byteLength; offset += chunkSize) {
    const end = Math.min(bytes.byteLength, offset + chunkSize);
    const chunk = decoder.decode(bytes.subarray(offset, end), { stream: end < bytes.byteLength });
    if (chunk.length > 0) yield chunk;
  }

  const tail = decoder.decode();
  if (tail.length > 0) yield tail;
}

async function emitDocumentJobBatch(
  advance: AdvanceDocumentJobFn,
  batchAccumulator: ReturnType<typeof createDocumentJobBatchAccumulator>,
  onBatch: DocumentJobBatchListener | undefined,
  request: AdvanceDocumentJobRequest,
): Promise<void> {
  const batch = await advance(request);
  batchAccumulator.append(batch);
  await onBatch?.(batch);
}

async function streamDocumentJobTextChunks(input: {
  jobHandle: number;
  chunks: Iterable<string> | AsyncIterable<string>;
  advance: AdvanceDocumentJobFn;
  onBatch?: DocumentJobBatchListener;
}): Promise<EventBatch> {
  const accumulator = createDocumentJobBatchAccumulator();

  for await (const chunk of input.chunks) {
    if (!chunk) continue;
    await emitDocumentJobBatch(input.advance, accumulator, input.onBatch, {
      jobHandle: input.jobHandle,
      kind: 'textChunk',
      text: chunk,
    });
  }

  await emitDocumentJobBatch(input.advance, accumulator, input.onBatch, {
    jobHandle: input.jobHandle,
    kind: 'close',
  });
  return accumulator.result();
}

async function streamDocumentJobBinaryChunks(input: {
  jobHandle: number;
  chunks: Iterable<Uint8Array> | AsyncIterable<Uint8Array>;
  advance: AdvanceDocumentJobFn;
  onBatch?: DocumentJobBatchListener;
  onChunk?: DocumentJobBinaryChunkListener;
}): Promise<EventBatch> {
  const accumulator = createDocumentJobBatchAccumulator();

  for await (const chunk of input.chunks) {
    if (!chunk.byteLength) continue;
    await input.onChunk?.(chunk);
    await emitDocumentJobBatch(input.advance, accumulator, input.onBatch, {
      jobHandle: input.jobHandle,
      kind: 'binaryChunk',
      data: chunk,
    });
  }

  await emitDocumentJobBatch(input.advance, accumulator, input.onBatch, {
    jobHandle: input.jobHandle,
    kind: 'close',
  });
  return accumulator.result();
}

async function* coalescedByteChunks(input: {
  chunks: AsyncIterable<Uint8Array>;
  chunkSize?: number;
  coalesce?: boolean;
}): AsyncGenerator<Uint8Array, void, void> {
  const maxChunkSize =
    typeof input.chunkSize === 'number' && Number.isFinite(input.chunkSize)
      ? Math.max(1, Math.trunc(input.chunkSize))
      : 0;
  const pendingParts: Uint8Array[] = [];
  let pendingBytes = 0;

  const drainPending = (): Uint8Array => {
    if (pendingParts.length === 1) {
      const only = pendingParts[0] ?? new Uint8Array();
      pendingParts.length = 0;
      pendingBytes = 0;
      return only;
    }
    const merged = new Uint8Array(pendingBytes);
    let offset = 0;
    for (const part of pendingParts) {
      merged.set(part, offset);
      offset += part.byteLength;
    }
    pendingParts.length = 0;
    pendingBytes = 0;
    return merged;
  };

  for await (const bytes of input.chunks) {
    if (maxChunkSize <= 0) {
      if (bytes.byteLength > 0) yield bytes;
      continue;
    }
    if (!input.coalesce) {
      if (bytes.byteLength <= maxChunkSize) {
        if (bytes.byteLength > 0) yield bytes;
        continue;
      }
      for await (const slice of slicedByteChunks(bytes, maxChunkSize)) {
        if (slice.byteLength > 0) yield slice;
      }
      continue;
    }

    let offset = 0;
    while (offset < bytes.byteLength) {
      const take = Math.min(maxChunkSize - pendingBytes, bytes.byteLength - offset);
      pendingParts.push(bytes.subarray(offset, offset + take));
      pendingBytes += take;
      offset += take;
      if (pendingBytes >= maxChunkSize) {
        yield drainPending();
      }
    }
  }

  if (pendingBytes > 0) yield drainPending();
}

async function* slicedByteChunks(
  bytes: Uint8Array,
  chunkSize: number,
): AsyncGenerator<Uint8Array, void, void> {
  for (let offset = 0; offset < bytes.byteLength; offset += chunkSize) {
    yield bytes.subarray(offset, Math.min(bytes.byteLength, offset + chunkSize));
  }
}

export async function streamDocumentJobText(input: {
  jobHandle: number;
  text: string;
  advance: AdvanceDocumentJobFn;
  onBatch?: DocumentJobBatchListener;
  chunkSize?: number;
}): Promise<EventBatch> {
  const chunkSize = Math.max(1, input.chunkSize ?? getStreamChunkSize());
  return streamDocumentJobTextChunks({
    jobHandle: input.jobHandle,
    chunks: splitTextIntoUtf8Chunks(input.text, chunkSize),
    advance: input.advance,
    onBatch: input.onBatch,
  });
}

export async function streamDocumentJobBytes(input: {
  jobHandle: number;
  bytes: Uint8Array | ArrayBuffer;
  advance: AdvanceDocumentJobFn;
  onBatch?: DocumentJobBatchListener;
  chunkSize?: number;
}): Promise<EventBatch> {
  const chunkSize = Math.max(1, input.chunkSize ?? getStreamChunkSize());
  const bytes = input.bytes instanceof Uint8Array ? input.bytes : new Uint8Array(input.bytes);
  return streamDocumentJobBinaryChunks({
    jobHandle: input.jobHandle,
    chunks: slicedByteChunks(bytes, chunkSize),
    advance: input.advance,
    onBatch: input.onBatch,
  });
}

async function* readableChunks(
  readable: ReadableStream<Uint8Array>,
): AsyncGenerator<Uint8Array, void, void> {
  const reader = readable.getReader();
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) return;
      if (value) yield value;
    }
  } finally {
    reader.releaseLock();
  }
}

function toAsyncChunks(
  readable: ReadableStream<Uint8Array> | AsyncIterable<Uint8Array>,
): AsyncIterable<Uint8Array> {
  return Symbol.asyncIterator in readable
    ? (readable as AsyncIterable<Uint8Array>)
    : readableChunks(readable as ReadableStream<Uint8Array>);
}

export async function streamDocumentJobReadable(input: {
  jobHandle: number;
  readable: ReadableStream<Uint8Array> | AsyncIterable<Uint8Array>;
  advance: AdvanceDocumentJobFn;
  onBatch?: DocumentJobBatchListener;
  onChunk?: DocumentJobBinaryChunkListener;
  chunkSize?: number;
}): Promise<EventBatch> {
  const chunkSize = Math.max(1, input.chunkSize ?? getStreamChunkSize());
  return streamDocumentJobBinaryChunks({
    jobHandle: input.jobHandle,
    chunks: coalescedByteChunks({
      chunks: toAsyncChunks(input.readable),
      chunkSize,
      coalesce: input.chunkSize != null,
    }),
    advance: input.advance,
    onBatch: input.onBatch,
    onChunk: input.onChunk,
  });
}
