// 职责：Worker document job transport handler — 透传请求到 Rust runtime，并同步最小本地可见层缓存
import type { WorkerContext, WorkerRequest } from './protocol';
import { advanceDocumentJob, buildHoverSubgraphProjection, cancelDocumentJob, querySnapshot, startDocumentJob, type DocumentJobSettings } from '@core-wasm/index';
import { postOk, postError } from './logging';

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
  ctx: WorkerContext,
  message: Extract<WorkerRequest, { type: 'startDocumentJob' }>,
): Promise<void> {
  try {
    const result = await startDocumentJob({
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
    postOk(ctx, message.id, result);
  } catch (error) {
    postError(ctx, message.id, error instanceof Error ? error.message : String(error));
  }
}

export async function handleCancelDocumentJob(
  ctx: WorkerContext,
  message: Extract<WorkerRequest, { type: 'cancelDocumentJob' }>,
): Promise<void> {
  try {
    await cancelDocumentJob({ jobHandle: message.jobHandle });
    postOk(ctx, message.id, true);
  } catch (error) {
    postError(ctx, message.id, error instanceof Error ? error.message : String(error));
  }
}

export async function handleAdvanceDocumentJob(
  ctx: WorkerContext,
  message: Extract<WorkerRequest, { type: 'advanceDocumentJob' }>,
): Promise<void> {
  try {
    const batch = await advanceDocumentJob({
      jobHandle: message.jobHandle,
      kind: message.kind,
      text: message.text,
      data: message.data,
    });
    postOk(ctx, message.id, batch);
  } catch (error) {
    postError(ctx, message.id, error instanceof Error ? error.message : String(error));
  }
}

export async function handleQuerySnapshot(
  ctx: WorkerContext,
  message: Extract<WorkerRequest, { type: 'querySnapshot' }>,
): Promise<void> {
  try {
    const result = await querySnapshot({
      documentKey: message.documentKey,
      queryKind: message.queryKind,
      snapshotId: message.snapshotId,
      pathPattern: message.pathPattern,
      spanStart: message.spanStart,
      spanEnd: message.spanEnd,
      target: message.target,
    });
    postOk(ctx, message.id, result);
  } catch (error) {
    postError(ctx, message.id, error instanceof Error ? error.message : String(error));
  }
}

export async function handleBuildHoverSubgraphProjection(
  ctx: WorkerContext,
  message: Extract<WorkerRequest, { type: 'buildHoverSubgraphProjection' }>,
): Promise<void> {
  try {
    const result = await buildHoverSubgraphProjection({
      documentKey: message.documentKey,
      snapshotId: message.snapshotId,
      path: message.path,
    });
    postOk(ctx, message.id, result);
  } catch (error) {
    postError(ctx, message.id, error instanceof Error ? error.message : String(error));
  }
}
