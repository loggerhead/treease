// Responsibility: map Worker DocumentJob requests to the Rust runtime; transport responses and error conversion stay elsewhere.
import type { WorkerRequest } from './protocol';
import { advanceDocumentJob, buildHoverSubgraphProjection, cancelDocumentJob, querySnapshot, startDocumentJob, type DocumentJobSettings } from '@core-wasm/index';

function defaultDocumentJobSettings(nest: boolean): DocumentJobSettings {
  return {
    parser: { enableNest: nest, nestMaxDepth: 8 },
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
}


export async function handleStartDocumentJob(
  message: Extract<WorkerRequest, { type: 'startDocumentJob' }>,
): Promise<Awaited<ReturnType<typeof startDocumentJob>>> {
  return startDocumentJob({
    documentKey: message.documentKey,
    language: message.language,
    nest: message.nest ?? message.settings?.parser.enableNest ?? false,
    settings: message.settings ?? defaultDocumentJobSettings(message.nest ?? false),
    outputGraph: message.outputGraph,
    outputAnalysis: message.outputAnalysis,
    builderConfig: message.builderConfig,
    baseSnapshotId: message.baseSnapshotId,
    edits: message.edits,
  });
}

export async function handleCancelDocumentJob(
  message: Extract<WorkerRequest, { type: 'cancelDocumentJob' }>,
): Promise<true> {
  await cancelDocumentJob({ jobHandle: message.jobHandle });
  return true;
}

export async function handleAdvanceDocumentJob(
  message: Extract<WorkerRequest, { type: 'advanceDocumentJob' }>,
): Promise<Awaited<ReturnType<typeof advanceDocumentJob>>> {
  return advanceDocumentJob({
    jobHandle: message.jobHandle,
    kind: message.kind,
    text: message.text,
    data: message.data,
  });
}

export async function handleQuerySnapshot(
  message: Extract<WorkerRequest, { type: 'querySnapshot' }>,
): Promise<Awaited<ReturnType<typeof querySnapshot>>> {
  return querySnapshot({
    documentKey: message.documentKey,
    queryKind: message.queryKind,
    snapshotId: message.snapshotId,
    pathPattern: message.pathPattern,
    spanStart: message.spanStart,
    spanEnd: message.spanEnd,
    target: message.target,
  });
}

export async function handleBuildHoverSubgraphProjection(
  message: Extract<WorkerRequest, { type: 'buildHoverSubgraphProjection' }>,
): Promise<Awaited<ReturnType<typeof buildHoverSubgraphProjection>>> {
  return buildHoverSubgraphProjection({
    documentKey: message.documentKey,
    snapshotId: message.snapshotId,
    path: message.path,
  });
}
