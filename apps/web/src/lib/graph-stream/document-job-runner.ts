import type {
  BuilderConfig,
  DocumentJobSettings,
  DocumentTextEdit,
  EventBatch,
  SnapshotId,
  StartDocumentJobResult,
} from '@core-wasm/index';
import { getSharedWasmWorkerClient } from '../wasm/wasm-worker-singleton';
import {
  mergeEventBatches,
  streamDocumentJobReadable,
  streamDocumentJobText,
  type AdvanceDocumentJobFn,
  type DocumentJobBatchListener,
  type DocumentJobBinaryChunkListener,
} from '../../shared/document-job-stream';
import {
  collectDocumentJobResult,
  normalizeDocumentJobAnalysisPayload,
  type DocumentJobResultStatus,
} from '../../shared/document-job-result';
import type { DocumentAnalysisResult } from '../../shared/worker-protocol/protocol';

export type GraphDocumentJobSettingsInput = {
  enableNest: boolean;
  formatting: {
    indent: number;
    smart: boolean;
    maxLineLength: number;
    maxInlineComplexity: number;
    maxArrayInlineItems: number;
    alignObjectArrays: boolean;
  };
  formatSourceOnClose?: boolean;
};

export function buildDocumentJobSettings(input: GraphDocumentJobSettingsInput): DocumentJobSettings {
  return {
    parser: {
      enableNest: input.enableNest,
      nestMaxDepth: 8,
    },
    formatting: {
      indent: input.formatting.indent,
      smart: input.formatting.smart,
      formatSourceOnClose: input.formatSourceOnClose ?? true,
      maxLineLength: input.formatting.maxLineLength,
      maxInlineComplexity: input.formatting.maxInlineComplexity,
      maxArrayInlineItems: input.formatting.maxArrayInlineItems,
      alignObjectArrays: input.formatting.alignObjectArrays,
    },
  };
}

export type GraphDocumentJobInput = {
  documentKey: string;
  language: string;
  settings: DocumentJobSettings;
  builderConfig?: BuilderConfig;
  outputAnalysis?: boolean;
  outputGraph?: boolean;
  baseSnapshotId?: SnapshotId | null;
  edits?: DocumentTextEdit[];
};

export type TextGraphDocumentJobInput = GraphDocumentJobInput & {
  text: string;
};

export type ReadableGraphDocumentJobInput = GraphDocumentJobInput & {
  readable: ReadableStream<Uint8Array> | AsyncIterable<Uint8Array>;
  onBatch?: DocumentJobBatchListener;
  onChunk?: DocumentJobBinaryChunkListener;
  chunkSize?: number;
};

export type DocumentJobGraphResult = {
  status: DocumentJobResultStatus;
  batch: EventBatch;
  analysis: DocumentAnalysisResult | null;
  jobHandle: number;
  snapshotId: SnapshotId | null;
  sourceText: string | null;
};

export type StreamDocumentJob = (input: {
  jobHandle: number;
  advance: AdvanceDocumentJobFn;
}) => Promise<EventBatch>;

export type GraphDocumentJobLifecycleHooks = {
  onJobHandle?: (jobHandle: number) => void;
  onBatch?: (batch: EventBatch) => void;
};

function chainBatchListener(
  primary?: DocumentJobBatchListener,
  secondary?: DocumentJobBatchListener,
): DocumentJobBatchListener | undefined {
  if (!primary) return secondary;
  if (!secondary) return primary;
  return async (batch) => {
    await primary(batch);
    await secondary(batch);
  };
}

export async function startSharedGraphDocumentJob(input: GraphDocumentJobInput): Promise<{
  started: StartDocumentJobResult;
  advance: AdvanceDocumentJobFn;
}> {
  const client = await getSharedWasmWorkerClient();
  const started = await client.call<StartDocumentJobResult>('startDocumentJob', {
    documentKey: input.documentKey,
    language: input.language,
    settings: input.settings,
    outputAnalysis: input.outputAnalysis ?? true,
    outputGraph: input.outputGraph ?? true,
    builderConfig: input.builderConfig,
    baseSnapshotId: input.baseSnapshotId,
    edits: input.edits,
  });
  return {
    started,
    advance: (request) => client.call<EventBatch>('advanceDocumentJob', request),
  };
}

export function collectGraphDocumentJobResult(params: {
  documentKey: string;
  language: string;
  jobHandle: number;
  batches: EventBatch[];
}): DocumentJobGraphResult {
  const batch = mergeEventBatches(params.batches);
  const result = collectDocumentJobResult(batch);
  const analysis = normalizeDocumentJobAnalysisPayload(params.documentKey, params.language, result.analysis);
  return {
    status: result.status,
    batch,
    analysis,
    jobHandle: params.jobHandle,
    snapshotId: result.snapshotId,
    sourceText: result.sourceText,
  };
}

export async function runSharedGraphDocumentJob(
  input: GraphDocumentJobInput,
  streamDocumentJob: StreamDocumentJob,
  hooks?: GraphDocumentJobLifecycleHooks,
): Promise<DocumentJobGraphResult> {
  const { started, advance } = await startSharedGraphDocumentJob(input);
  hooks?.onJobHandle?.(started.jobHandle);
  hooks?.onBatch?.(started.batch);
  const streamedBatch = await streamDocumentJob({
    jobHandle: started.jobHandle,
    advance,
  });
  return collectGraphDocumentJobResult({
    documentKey: input.documentKey,
    language: input.language,
    jobHandle: started.jobHandle,
    batches: [started.batch, streamedBatch],
  });
}

export async function runTextDocumentJobForGraph(
  input: TextGraphDocumentJobInput,
  hooks?: GraphDocumentJobLifecycleHooks,
): Promise<DocumentJobGraphResult> {
  return runSharedGraphDocumentJob(
    input,
    (streamInput) =>
      streamDocumentJobText({
        ...streamInput,
        text: input.text,
        onBatch: hooks?.onBatch,
      }),
    hooks,
  );
}

export async function runReadableDocumentJobForGraph(
  input: ReadableGraphDocumentJobInput,
  hooks?: GraphDocumentJobLifecycleHooks,
): Promise<DocumentJobGraphResult> {
  return runSharedGraphDocumentJob(
    input,
    (streamInput) =>
      streamDocumentJobReadable({
        ...streamInput,
        readable: input.readable,
        onBatch: chainBatchListener(input.onBatch, hooks?.onBatch),
        onChunk: input.onChunk,
        chunkSize: input.chunkSize,
      }),
    hooks,
  );
}
