// 职责：共享的 graph stream 事件构造与解码工具：createBuildGraphDeltaEvent、normalizeGraphDelta、delta transfer codec
import { decodeGraphDeltaPayload } from '@core-wasm/index';
import type {
  BuildGraphDeltaEvent,
  BuildGraphProgressEvent,
  GraphStreamDeltaEvent,
  GraphStreamDeltaTransferEvent,
  GraphStreamProgressEvent,
  NormalizedGraphDelta,
  RawGraphDelta,
  StreamProgressPhase,
} from './protocol';
import { isRawGraphDelta, normalizeRawEdge, normalizeRawNode, normalizeRawTableCellPatch } from './graph-delta-normalize';

type ProgressPayload = {
  processedBytes: number;
  totalBytes: number;
  value: number;
};

export function createEmptyRawGraphDelta(): RawGraphDelta {
  return {
    clear: 0,
    nodesAdded: [],
    nodesUpdated: [],
    nodesRemoved: [],
    edgesAdded: [],
    edgesRemoved: [],
    tableCellPatches: [],
  };
}

export function normalizeGraphDelta(delta: RawGraphDelta): NormalizedGraphDelta {
  return {
    normalized: true,
    clear: delta.clear ?? 0,
    nodesAdded: delta.nodesAdded.map(normalizeRawNode),
    nodesUpdated: delta.nodesUpdated.map(normalizeRawNode),
    nodesRemoved: delta.nodesRemoved,
    edgesAdded: delta.edgesAdded.map(normalizeRawEdge),
    edgesRemoved: delta.edgesRemoved.map(normalizeRawEdge),
    tableCellPatches: (delta.tableCellPatches ?? []).map(normalizeRawTableCellPatch),
    tablePatches: delta.tablePatches ?? [],
    layoutPatches: delta.layoutPatches ?? [],
  };
}

function normalizeOptionalGraphDelta(delta?: RawGraphDelta | null): NormalizedGraphDelta {
  return normalizeGraphDelta(delta ?? createEmptyRawGraphDelta());
}

function normalizeInputByteLength(value: number): number {
  return Math.max(0, Math.round(value || 0));
}

function createGraphStreamDeltaEvent(input: {
  sessionId: string;
  streamKey: string;
  documentKey?: string;
  streamRunId: string;
  eventSeq: number;
  inputByteLength: number;
  delta?: RawGraphDelta | null;
  final: boolean;
}): GraphStreamDeltaEvent {
  return {
    event: 'graphStreamDelta',
    sessionId: input.sessionId,
    streamKey: input.streamKey,
    documentKey: input.documentKey,
    streamRunId: input.streamRunId,
    eventSeq: input.eventSeq,
    inputByteLength: normalizeInputByteLength(input.inputByteLength),
    delta: normalizeOptionalGraphDelta(input.delta),
    final: input.final,
  };
}

function clampProgressPayload(input: ProgressPayload): ProgressPayload {
  const totalBytes = clampProgressBytes(input.totalBytes);
  const processedBytes = clampProgressBytes(input.processedBytes);
  return {
    processedBytes: totalBytes > 0 ? Math.min(processedBytes, totalBytes) : processedBytes,
    totalBytes,
    value: clampProgressValue(input.value),
  };
}

export function createBuildGraphDeltaEvent(input: {
  documentKey: string;
  streamRunId: string;
  eventSeq: number;
  delta?: RawGraphDelta | null;
  final: boolean;
}): BuildGraphDeltaEvent {
  return {
    event: 'graphDelta',
    documentKey: input.documentKey,
    streamRunId: input.streamRunId,
    eventSeq: input.eventSeq,
    delta: normalizeOptionalGraphDelta(input.delta),
    final: input.final,
  };
}

export function createGraphStreamDeltaTransferEvent(input: {
  sessionId: string;
  streamKey: string;
  documentKey?: string;
  streamRunId: string;
  eventSeq: number;
  inputByteLength: number;
  deltaBytes?: Uint8Array | ArrayBuffer | null;
  final: boolean;
}): GraphStreamDeltaTransferEvent {
  const deltaBytes: ArrayBuffer =
    input.deltaBytes instanceof Uint8Array
      ? input.deltaBytes.buffer.slice(input.deltaBytes.byteOffset, input.deltaBytes.byteOffset + input.deltaBytes.byteLength) as ArrayBuffer
      : input.deltaBytes instanceof ArrayBuffer
        ? input.deltaBytes
        : new ArrayBuffer(0);
  return {
    event: 'graphStreamDelta',
    sessionId: input.sessionId,
    streamKey: input.streamKey,
    documentKey: input.documentKey,
    streamRunId: input.streamRunId,
    eventSeq: input.eventSeq,
    inputByteLength: normalizeInputByteLength(input.inputByteLength),
    deltaBytes,
    final: input.final,
  };
}

export function decodeGraphStreamDeltaTransferEvent(event: GraphStreamDeltaTransferEvent): GraphStreamDeltaEvent {
  const payload = event.deltaBytes;
  if (!(payload instanceof ArrayBuffer) || payload.byteLength === 0) {
    return createGraphStreamDeltaEvent({
      sessionId: event.sessionId,
      streamKey: event.streamKey,
      documentKey: event.documentKey,
      streamRunId: event.streamRunId,
      eventSeq: event.eventSeq,
      inputByteLength: event.inputByteLength,
      final: event.final,
    });
  }
  const decoded = decodeGraphDeltaPayload(new Uint8Array(payload));
  if (!isRawGraphDelta(decoded)) {
    throw new Error('graph stream delta payload decode failed');
  }
  return createGraphStreamDeltaEvent({
    sessionId: event.sessionId,
    streamKey: event.streamKey,
    documentKey: event.documentKey,
    streamRunId: event.streamRunId,
    eventSeq: event.eventSeq,
    inputByteLength: event.inputByteLength,
    delta: decoded,
    final: event.final,
  });
}

function clampProgressValue(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(100, Number(value.toFixed(2))));
}

function clampProgressBytes(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.round(value));
}

export function createBuildGraphProgressEvent(input: {
  documentKey: string;
  streamRunId: string;
  eventSeq: number;
  phase: StreamProgressPhase;
  processedBytes: number;
  totalBytes: number;
  value: number;
  final: boolean;
}): BuildGraphProgressEvent {
  const progress = clampProgressPayload(input);
  return {
    event: 'graphProgress',
    documentKey: input.documentKey,
    streamRunId: input.streamRunId,
    eventSeq: input.eventSeq,
    phase: input.phase,
    processedBytes: progress.processedBytes,
    totalBytes: progress.totalBytes,
    value: progress.value,
    final: input.final,
  };
}

export function createGraphStreamProgressEvent(input: {
  sessionId: string;
  streamKey: string;
  streamRunId: string;
  eventSeq: number;
  phase: StreamProgressPhase;
  processedBytes: number;
  totalBytes: number;
  value: number;
  final: boolean;
}): GraphStreamProgressEvent {
  const progress = clampProgressPayload(input);
  return {
    event: 'graphStreamProgress',
    sessionId: input.sessionId,
    streamKey: input.streamKey,
    streamRunId: input.streamRunId,
    eventSeq: input.eventSeq,
    phase: input.phase,
    processedBytes: progress.processedBytes,
    totalBytes: progress.totalBytes,
    value: progress.value,
    final: input.final,
  };
}
