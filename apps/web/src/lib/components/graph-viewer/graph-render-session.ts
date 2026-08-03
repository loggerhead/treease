// Responsibility: manage GraphViewer rendering from Document Job streaming events and SnapshotReady.mainGraph.
// Self-started text jobs and external full-edit document-job sessions share one projection-apply pipeline; do not fall back to buildProjection('mainGraph').
import { type BuilderConfig, type EventBatch, type SnapshotId, type TreeNode } from '@core-wasm/index';
import { selectGraphStreamChunkSize } from '../../graph-stream/chunk-size-policy';
import type { GraphViewerConfig } from '../../settings/ui-settings';
import type { GraphEdge, GraphNode } from '@treease/graph-viewer-runtime';
import type { JsonBlockSelection } from '../../store/full-edit-ui-store';
import type {
  DocumentAnalysisResult,
  RawGraphDelta,
} from '../../../shared/worker-protocol/protocol';
import { isRawGraphDelta } from '../../../shared/worker-protocol/graph-delta-normalize';
import { buildGraphStreamBuilderConfig } from '../../graph-stream/graph-stream-builder-config';
import { processGraphBatchEvents, projectionToRawGraphDelta } from '../../../shared/document-job-graph-stream';
import { bindWorkspaceSnapshot, clearWorkspaceSnapshotBinding } from '../../store/workspace-store';
import { createViewRuntimeOperation } from '../../guards/view-runtime-operation';
import { streamDocumentJobText, type AdvanceDocumentJobRequest } from '../../../shared/document-job-stream';
import type { FullEditDocumentJobSession } from '../../graph-stream/full-edit-document-job-session';
import { buildDocumentJobSettings, collectGraphDocumentJobResult, startSharedGraphDocumentJob } from '../../graph-stream/document-job-runner';
import { normalizeDocumentJobAnalysisPayload } from '../../../shared/document-job-result';
import { markGraphApplied, markGraphFlushed } from '../../test-bridge/runtime-readiness';

const defaultGraphDocumentFormatting = {
  indent: 2,
  smart: true,
  maxLineLength: 100,
  maxInlineComplexity: 1,
  maxArrayInlineItems: 6,
  alignObjectArrays: true,
};

function toLightweightDocumentAnalysis(
  documentKey: string,
  language: string,
  analysis: DocumentAnalysisResult | null,
): DocumentAnalysisResult | null {
  if (!analysis) return null;
  return {
    ...analysis,
    documentKey,
    tree: null,
    value: null,
    language: analysis.language || language,
  };
}

export type ParsedTreeData = {
  tree: TreeNode;
  value: unknown;
};

type GraphRenderResult = {
  nodes: GraphNode[];
  edges: GraphEdge[];
};

type GraphRenderSourceRef = {
  kind: 'incremental' | 'full-edit';
  documentKey: string;
  language: string;
  text: string;
  revision: number;
};

type GraphReadinessRef = {
  documentKey: string;
  revision: number;
  mode: 'committed' | 'streaming' | 'json-block';
};

type GraphRenderSceneBridge = {
  applyGraphDelta: (delta: RawGraphDelta, version?: { baseGraphVersion: number; graphVersion: number }) => Promise<void>;
  flushPendingRenderWork: () => Promise<void>;
  cancelActiveRenderWork: () => void;
  replaceRenderedGraph: (value: GraphRenderResult) => GraphRenderResult;
  getLastRenderedGraph: () => GraphRenderResult | null;
};

export type GraphRenderGuard = {
  documentKey: string;
  revision: number;
  snapshotId: SnapshotId | null;
  renderToken: number;
  mode: 'committed' | 'streaming' | 'json-block';
};

type GraphRenderSessionDeps = {
  getDocumentKey: () => string;
  getEnableNest: () => boolean;
  getRenderConfig: () => GraphViewerConfig;
  getJsonBlockSelection: () => JsonBlockSelection | null;
  hasRenderTarget: () => boolean;
  shouldAttachGraphViewerTestHooks: () => boolean;
  getGraphStreamState: () => Record<string, any> | null;
  replaceGraphStreamState: (state: Record<string, any>) => void;
  nextTreeToken: () => number;
  publishTreeState: (
    requestId: number,
    tree: TreeNode | null,
    value: unknown,
    source: 'editor' | 'graph',
    revision: number,
    snapshotId?: SnapshotId | null,
  ) => boolean;
  clearTreeState: (requestId: number, source: 'editor' | 'graph', revision: number, snapshotId?: SnapshotId | null) => boolean;
  resetJsonBlockViewport: () => void;
  callWorker: <T>(method: string, input: unknown) => Promise<T>;
  onStreamFinalAnalysis: (
    documentKey: string,
    language: string,
    revision: number,
    analysis: DocumentAnalysisResult | null,
    snapshotId: SnapshotId | null,
  ) => void;
  onStreamFinalRedraw: (
    mode: 'committed' | 'streaming' | 'json-block',
    revision: number,
    guard: GraphRenderGuard,
  ) => void;
  onTopologyRendered?: (topologyBytes: Uint8Array) => Promise<void>;
  updateStreamProgress: (event: { event: string; phase: string; processedBytes: number; totalBytes: number; final: boolean }) => void;
  resetStreamProgress: () => void;
  completeStreamProgress: () => void;
  setErrorMessage: (message: string) => void;
  clearErrorMessage: () => void;
  handleError: (
    error: unknown,
    context: { component: string; operation: string; metadata?: Record<string, unknown> },
  ) => void;
};
type DocumentJobFinalEvent =
  | Extract<EventBatch['events'][number], { type: 'snapshotReady' }>
  | Extract<EventBatch['events'][number], { type: 'parseFailed' }>;

const textEncoder = new TextEncoder();

function isExpectedRenderTermination(error: unknown, isCurrent: () => boolean): boolean {
  if (!isCurrent()) return true;
  const message = error instanceof Error ? error.message : String(error);
  return /stale|cancel|disposed|dispose|no longer fresh/i.test(message);
}

function buildGraphProgressEvent(streamRunId: string, processedBytes: number, totalBytes: number) {
  return {
    event: 'graphProgress',
    phase: 'streaming',
    processedBytes,
    totalBytes,
    final: false,
    streamRunId,
    eventSeq: processedBytes,
    value: totalBytes > 0 ? Math.min(99, Math.round((processedBytes / totalBytes) * 100)) : 0,
  };
}

function buildGraphLifecycleProgressEvent(
  streamRunId: string,
  totalBytes: number,
  phase: 'flushing' | 'finishing',
) {
  return {
    event: 'graphProgress',
    phase,
    processedBytes: totalBytes,
    totalBytes,
    final: false,
    streamRunId,
    eventSeq: totalBytes + (phase === 'flushing' ? 1 : 2),
    value: 99,
  };
}


function finalDocumentEvent(batch: EventBatch): DocumentJobFinalEvent | null {
  for (let index = batch.events.length - 1; index >= 0; index -= 1) {
    const event = batch.events[index];
    if (event.type === 'snapshotReady' || event.type === 'parseFailed') return event;
  }
  return null;
}

export function createGraphRenderSession(deps: GraphRenderSessionDeps) {
  let sceneBridge: GraphRenderSceneBridge | null = null;
  let renderedDocumentKey: string | null = null;
  let renderedRevision: number | null = null;
  let renderedLanguage: string | null = null;
  let renderedText: string | null = null;
  let activeSnapshotId: SnapshotId | null = null;
  let streamRunSeq = 0;
  let activeRenderToken = 0;
  let activeExternalSessionId: string | null = null;

  function nextStreamRunId(documentKey: string, revision: number): string {
    streamRunSeq += 1;
    return `${documentKey}:${revision}:${streamRunSeq}`;
  }

  type OwnedRenderOperation = {
    lifecycle: ReturnType<typeof createViewRuntimeOperation>;
    registerJobHandle: (jobHandle: number) => void;
    releaseJobHandle: () => void;
    cancel: () => Promise<void>;
  };

  function createOwnedRenderOperation(options: {
    captured: Parameters<typeof createViewRuntimeOperation>[0]['captured'];
    getCurrent: Parameters<typeof createViewRuntimeOperation>[0]['getCurrent'];
    cancelResource?: () => Promise<void> | void;
  }): OwnedRenderOperation {
    let ownedJobHandle: number | null = null;
    let cancelRequested = false;
    let cancelPromise: Promise<void> | null = null;

    const cancel = (): Promise<void> => {
      cancelRequested = true;
      if (cancelPromise) return cancelPromise;
      const jobHandle = ownedJobHandle;
      if (jobHandle == null && !options.cancelResource) return Promise.resolve();
      cancelPromise = (jobHandle == null
        ? Promise.resolve().then(() => options.cancelResource?.())
        : deps.callWorker('cancelDocumentJob', { jobHandle }))
        .then(() => undefined)
        .catch(() => undefined)
        .finally(() => {
          if (ownedJobHandle === jobHandle) ownedJobHandle = null;
        });
      return cancelPromise;
    };

    const lifecycle = createViewRuntimeOperation({ ...options, onStale: cancel });
    return {
      lifecycle,
      registerJobHandle: (jobHandle) => {
        ownedJobHandle = jobHandle;
        if (cancelRequested) void cancel();
      },
      releaseJobHandle: () => {
        ownedJobHandle = null;
      },
      cancel,
    };
  }

  let activeOperation: OwnedRenderOperation | null = null;

  function emptyRenderResult(): GraphRenderResult {
    return { nodes: [], edges: [] };
  }

  function getBuilderConfig(): BuilderConfig {
    return buildGraphStreamBuilderConfig(deps.getRenderConfig());
  }

  function attachSceneBridge(bridge: GraphRenderSceneBridge): void {
    sceneBridge = bridge;
  }

  function getSceneBridge(): GraphRenderSceneBridge {
    if (!sceneBridge) {
      throw new Error('Graph render scene bridge is not attached');
    }
    return sceneBridge;
  }

  async function flushSceneAndRedraw(
    mode: 'committed' | 'streaming' | 'json-block',
    revision: number,
    guard: GraphRenderGuard,
    readiness: GraphReadinessRef,
    freshness?: { isCurrent: () => boolean },
  ): Promise<void> {
    await getSceneBridge().flushPendingRenderWork();
    markGraphFlushed(readiness);
    if (freshness && !freshness.isCurrent()) return;
    deps.onStreamFinalRedraw(mode, revision, guard);
  }

  function mutateGraphStreamState(mutator: (state: Record<string, any>) => void): void {
    if (!deps.shouldAttachGraphViewerTestHooks()) return;
    const current = deps.getGraphStreamState() ?? { partialSeen: false, finalSeen: false };
    const next = { ...current };
    mutator(next);
    deps.replaceGraphStreamState(next);
  }

  function startGraphStreamMeasurement(
    documentKey: string,
    language: string,
    revision: number,
    textLength: number,
    totalBytes: number,
    chunkSize: number,
  ): void {
    if (!deps.shouldAttachGraphViewerTestHooks()) return;
    const startedAtMs = performance.now();
    const previous = deps.getGraphStreamState() ?? {};
    deps.replaceGraphStreamState({
      ...previous,
      partialSeen: false,
      finalSeen: false,
      requested: true,
      documentKey,
      language,
      revision,
      totalBytes,
      chunkSize,
      chunkCount: 0,
      progressEventCount: 0,
      startedAtMs,
      firstPartialAtMs: null,
      finalSeenAtMs: null,
      doneAtMs: null,
      failedAtMs: null,
      errorMessage: '',
      applyDeltaCount: 0,
      maxApplyDeltaMs: 0,
      receivedEvents: 0,
      acceptedEvents: 0,
      renderCalls: (previous.renderCalls ?? 0) + 1,
      lastRenderTextLength: textLength,
      lastRenderLanguage: language,
      lastUseStream: true,
      lastPhase: 'requested',
    });
  }

  function recordAdvanceRequest(kind: AdvanceDocumentJobRequest['kind']): void {
    if (kind !== 'textChunk') return;
    mutateGraphStreamState((state) => {
      state.chunkCount = (state.chunkCount ?? 0) + 1;
      state.lastPhase = 'chunk-sent';
    });
  }

  function recordAdvanceBatch(batch: EventBatch): void {
    mutateGraphStreamState((state) => {
      state.receivedEvents = (state.receivedEvents ?? 0) + batch.events.length;
      for (const event of batch.events) {
        if (event.type === 'progress') {
          state.progressEventCount = (state.progressEventCount ?? 0) + 1;
          state.acceptedEvents = (state.acceptedEvents ?? 0) + 1;
          state.lastPhase = 'progress';
        }
      }
    });
  }


  async function applyTrackedProjectionDelta(
    delta: RawGraphDelta,
    readiness: GraphReadinessRef,
    version?: { baseGraphVersion: number; graphVersion: number },
  ): Promise<void> {
    const startedAtMs = performance.now();
    await getSceneBridge().applyGraphDelta(delta, version);
    markGraphApplied(readiness);
    const finishedAtMs = performance.now();
    const durationMs = finishedAtMs - startedAtMs;

    mutateGraphStreamState((state) => {
      state.applyDeltaCount = (state.applyDeltaCount ?? 0) + 1;
      state.acceptedEvents = (state.acceptedEvents ?? 0) + 1;
      state.maxApplyDeltaMs = Math.max(state.maxApplyDeltaMs ?? 0, durationMs);
      if (!state.partialSeen) {
        state.partialSeen = true;
        state.firstPartialAtMs = finishedAtMs;
      }
      state.lastPhase = 'projection-applied';
    });
  }

  function markGraphStreamFinal(
    phase: 'snapshot-ready' | 'parse-failed' | 'exception',
    options: { errorMessage?: string; failed?: boolean } = {},
  ): void {
    const now = performance.now();
    mutateGraphStreamState((state) => {
      state.finalSeen = true;
      state.finalSeenAtMs = now;
      state.lastPhase = phase;
      if (options.errorMessage) {
        state.errorMessage = options.errorMessage;
      }
      if (options.failed) {
        state.failedAtMs = now;
      }
    });
  }

  function clearGraphStreamFailure(): void {
    mutateGraphStreamState((state) => {
      state.errorMessage = '';
      state.failedAtMs = null;
    });
  }

  function markGraphStreamDone(): void {
    const now = performance.now();
    mutateGraphStreamState((state) => {
      state.doneAtMs = now;
      state.lastPhase = 'done';
    });
  }

  function createProjectionEventApplier(
    totalBytes: number,
    streamRunId: string,
    readiness: GraphReadinessRef,
  ) {
    return async (batch: EventBatch): Promise<void> => {
      await processGraphBatchEvents(batch, totalBytes, {
        onProgress: (processedBytes: number, total: number) => {
          deps.updateStreamProgress(buildGraphProgressEvent(streamRunId, processedBytes, total));
        },
        onProjection: async (delta, version) => {
          await applyTrackedProjectionDelta(delta, readiness, version);
        },
      });
    };
  }

  async function consumeGraphBatchStream(params: {
    batches: AsyncIterable<EventBatch>;
    freshness: { isCurrent: () => boolean };
    applyProjectionEvents: (batch: EventBatch) => Promise<void>;
    onBatch?: (batch: EventBatch) => void;
  }): Promise<void> {
    for await (const batch of params.batches) {
      if (!params.freshness.isCurrent()) return;
      params.onBatch?.(batch);
      await params.applyProjectionEvents(batch);
    }
  }

  async function finalizeGraphDocumentJob(params: {
    documentKey: string;
    language: string;
    revision: number;
    renderedText: string | null;
    redrawMode: 'committed' | 'streaming';
    totalBytes: number;
    streamRunId: string;
    batch: EventBatch;
    analysis: DocumentAnalysisResult | null;
    snapshotId: SnapshotId | null;
    renderToken: number;
    freshness: { isCurrent: () => boolean };
  }): Promise<GraphRenderResult | null> {
    const finalEvent = finalDocumentEvent(params.batch);
    if (finalEvent == null) {
      console.error('[graph] document analysis did not produce a snapshot', {
        documentKey: params.documentKey,
        revision: params.revision,
      });
      deps.clearErrorMessage();
      deps.completeStreamProgress();
      return null;
    }

    if (finalEvent.type === 'snapshotReady') {
      const snapshotId = (params.snapshotId ?? finalEvent.snapshotId) as SnapshotId;
      if (!finalEvent.mainGraph) {
        console.error('[graph] document analysis did not produce requested main graph', {
          documentKey: params.documentKey,
          revision: params.revision,
        });
        deps.clearErrorMessage();
        deps.completeStreamProgress();
        return null;
      }
      const finalDelta = projectionToRawGraphDelta(finalEvent.mainGraph);
      if (!finalDelta || !isRawGraphDelta(finalDelta)) {
        throw new Error('document graph projection decode failed');
      }
      deps.updateStreamProgress(
        buildGraphLifecycleProgressEvent(params.streamRunId, params.totalBytes, 'finishing'),
      );
      // SnapshotReady.mainGraph is the authoritative final graph. Always apply
      // it so streaming-time approximation or stale edge geometry cannot leak
      // into the settled scene state.
      await applyTrackedProjectionDelta(finalDelta, {
        documentKey: params.documentKey,
        revision: params.revision,
        mode: params.redrawMode,
      });
      if (!params.freshness.isCurrent()) return null;
      deps.clearErrorMessage();
      clearGraphStreamFailure();
      markGraphStreamFinal('snapshot-ready');

      activeSnapshotId = snapshotId;
      renderedDocumentKey = params.documentKey;
      renderedRevision = params.revision;
      renderedLanguage = params.language;
      renderedText = params.renderedText;
      bindWorkspaceSnapshot({
        documentKey: params.documentKey,
        revision: params.revision,
        snapshotId,
      });

      deps.onStreamFinalAnalysis(
        params.documentKey,
        params.language,
        params.revision,
        toLightweightDocumentAnalysis(
          params.documentKey,
          params.language,
          params.analysis ?? normalizeDocumentJobAnalysisPayload(params.documentKey, params.language, finalEvent.analysis),
        ),
        snapshotId,
      );

      deps.completeStreamProgress();
      await flushSceneAndRedraw(
        params.redrawMode,
        params.revision,
        {
          documentKey: params.documentKey,
          revision: params.revision,
          snapshotId,
          renderToken: params.renderToken,
          mode: params.redrawMode,
        },
        {
          documentKey: params.documentKey,
          revision: params.revision,
          mode: params.redrawMode,
        },
      params.freshness,
      );
      if (params.freshness.isCurrent() && finalEvent.mainGraph.topologyBytes?.byteLength) {
        await deps.onTopologyRendered?.(finalEvent.mainGraph.topologyBytes);
      }
      markGraphStreamDone();

      return getSceneBridge().getLastRenderedGraph() ?? emptyRenderResult();
    }

    console.error('[graph] document analysis failed', {
      documentKey: params.documentKey,
      revision: params.revision,
      diagnostics: params.analysis?.diagnostics,
    });
    deps.clearErrorMessage();
    markGraphStreamFinal('parse-failed', {
      failed: true,
    });
    deps.onStreamFinalAnalysis(
      params.documentKey,
      params.language,
      params.revision,
      toLightweightDocumentAnalysis(
        params.documentKey,
        params.language,
        params.analysis ?? normalizeDocumentJobAnalysisPayload(params.documentKey, params.language, finalEvent.analysis),
      ),
      null,
    );
    deps.completeStreamProgress();
    const requestId = deps.nextTreeToken();
    deps.clearTreeState(requestId, 'graph', params.revision, null);
    await flushSceneAndRedraw(
      params.redrawMode,
      params.revision,
      {
        documentKey: params.documentKey,
        revision: params.revision,
        snapshotId: null,
        renderToken: params.renderToken,
        mode: params.redrawMode,
      },
      {
        documentKey: params.documentKey,
        revision: params.revision,
        mode: params.redrawMode,
      },
      params.freshness,
    );
    markGraphStreamDone();
    return getSceneBridge().getLastRenderedGraph() ?? emptyRenderResult();
  }

  async function dispose(): Promise<void> {
    activeRenderToken += 1;
    const operation = activeOperation;
    activeOperation = null;
    await operation?.cancel();
    if (renderedDocumentKey) {
      clearWorkspaceSnapshotBinding(renderedDocumentKey, activeSnapshotId);
    }
    renderedDocumentKey = null;
    renderedRevision = null;
    renderedLanguage = null;
    renderedText = null;
    activeSnapshotId = null;
    activeExternalSessionId = null;
    sceneBridge?.cancelActiveRenderWork();
  }


  function ensureDocumentKey(): string | null {
    const documentKey = deps.getDocumentKey();
    if (!documentKey) {
      deps.clearErrorMessage();
      return null;
    }
    return documentKey;
  }

  async function renderDocumentGraph(request: GraphRenderSourceRef): Promise<GraphRenderResult | null> {
    if (!deps.hasRenderTarget()) return null;
    if (!ensureDocumentKey()) return null;

    deps.clearErrorMessage();
    if (
      renderedDocumentKey === request.documentKey &&
      renderedRevision === request.revision &&
      renderedLanguage === request.language &&
      renderedText === request.text
    ) {
      return getSceneBridge().getLastRenderedGraph() ?? emptyRenderResult();
    }

    await dispose();
    const renderToken = ++activeRenderToken;
    deps.resetStreamProgress();

    const operation = createOwnedRenderOperation({
      captured: {
        documentKey: request.documentKey,
        revision: request.revision,
        languageId: request.language,
        token: renderToken,
      },
      getCurrent: () => ({
        documentKey: deps.getDocumentKey(),
        revision: request.revision,
        languageId: request.language,
        token: activeRenderToken,
      }),
    });
    activeOperation = operation;

    try {
      const totalBytes = textEncoder.encode(request.text).length;
      const chunkSize = selectGraphStreamChunkSize(totalBytes);
      const streamRunId = nextStreamRunId(request.documentKey, request.revision);
      startGraphStreamMeasurement(
        request.documentKey,
        request.language,
        request.revision,
        request.text.length,
        totalBytes,
        chunkSize,
      );
      const applyProjectionEvents = createProjectionEventApplier(totalBytes, streamRunId, {
        documentKey: request.documentKey,
        revision: request.revision,
        mode: request.kind === 'incremental' ? 'committed' : 'streaming',
      });

      const builderConfig = getBuilderConfig();
      const { started, advance } = await startSharedGraphDocumentJob({
        documentKey: request.documentKey,
        language: request.language,
        settings: buildDocumentJobSettings({
          enableNest: deps.getEnableNest(),
          formatting: defaultGraphDocumentFormatting,
          formatSourceOnClose: false,
        }),
        outputAnalysis: true,
        outputGraph: true,
        builderConfig,
      });
      operation.registerJobHandle(started.jobHandle);
      if (!operation.lifecycle.isCurrent()) return null;

      const streamedBatch = await streamDocumentJobText({
        jobHandle: started.jobHandle,
        text: request.text,
        chunkSize,
        advance: async (input) => {
          recordAdvanceRequest(input.kind);
          const batch = await advance(input);
          recordAdvanceBatch(batch);
          return batch;
        },
        onBatch: async (batch) => {
          if (!operation.lifecycle.isCurrent()) return;
          await applyProjectionEvents(batch);
        },
      });
      if (!operation.lifecycle.isCurrent()) return null;
      operation.releaseJobHandle();

      const result = collectGraphDocumentJobResult({
        documentKey: request.documentKey,
        language: request.language,
        jobHandle: started.jobHandle,
        batches: [started.batch, streamedBatch],
      });
      deps.updateStreamProgress(
        buildGraphLifecycleProgressEvent(streamRunId, totalBytes, 'flushing'),
      );
      return await finalizeGraphDocumentJob({
        documentKey: request.documentKey,
        language: request.language,
        revision: request.revision,
        renderedText: request.text,
        redrawMode: request.kind === 'incremental' ? 'committed' : 'streaming',
        totalBytes,
        streamRunId,
        batch: result.batch,
        analysis: result.analysis,
        snapshotId: result.snapshotId,
        renderToken,
        freshness: operation.lifecycle,
      });
    } catch (error) {
      if (isExpectedRenderTermination(error, operation.lifecycle.isCurrent)) {
        console.debug('[graph] document render ended before landing', error);
        deps.completeStreamProgress();
        markGraphStreamDone();
        return null;
      }
      deps.handleError(error, {
        component: 'GraphViewer',
        operation: 'renderDocumentGraph',
        metadata: { documentKey: request.documentKey, revision: request.revision },
      });
      console.error('[graph] document render failed', error);
      deps.clearErrorMessage();
      markGraphStreamFinal('exception', {
        failed: true,
      });
      deps.completeStreamProgress();
      markGraphStreamDone();
      return null;
    }
  }

  async function attachExternalDocumentJobSession(
    session: FullEditDocumentJobSession,
  ): Promise<GraphRenderResult | null> {
    if (!deps.hasRenderTarget()) return null;
    if (!ensureDocumentKey()) return null;
    if (activeExternalSessionId === session.sessionId) {
      return getSceneBridge().getLastRenderedGraph() ?? emptyRenderResult();
    }

    await dispose();
    const renderToken = ++activeRenderToken;
    activeExternalSessionId = session.sessionId;
    deps.resetStreamProgress();
    deps.clearErrorMessage();

    const operation = createOwnedRenderOperation({
      captured: {
        documentKey: session.documentKey,
        revision: session.revision,
        sessionId: session.sessionId,
        token: renderToken,
      },
      getCurrent: () => ({
        documentKey: deps.getDocumentKey(),
        revision: session.revision,
        sessionId: activeExternalSessionId,
        token: activeRenderToken,
      }),
    });
    activeOperation = operation;

    const totalBytes = session.totalBytes;
    const streamRunId = session.streamRunId || nextStreamRunId(session.documentKey, session.revision);
    startGraphStreamMeasurement(
      session.documentKey,
      session.language,
      session.revision,
      totalBytes,
      totalBytes,
      session.chunkSize ?? 0,
    );
    const applyProjectionEvents = createProjectionEventApplier(totalBytes, streamRunId, {
      documentKey: session.documentKey,
      revision: session.revision,
      mode: 'streaming',
    });
    try {
      // The full-edit controller owns the document job. A graph attachment may
      // detach or become stale as tabs change, but it must never abort that job.
      // If the bounded replay no longer contains the beginning, wait for the
      // terminal canonical render rather than projecting a partial stream.
      if (session.hasCompleteReplay()) {
        await consumeGraphBatchStream({
          batches: session.batches(),
          freshness: operation.lifecycle,
          applyProjectionEvents,
          onBatch: (batch) => {
            recordAdvanceBatch(batch);
          },
        });
      }
      if (!operation.lifecycle.isCurrent()) return null;
      operation.releaseJobHandle();

      const result = await session.result;
      if (!operation.lifecycle.isCurrent()) return null;

      deps.updateStreamProgress(
        buildGraphLifecycleProgressEvent(streamRunId, totalBytes, 'flushing'),
      );
      return await finalizeGraphDocumentJob({
        documentKey: session.documentKey,
        language: session.language,
        revision: session.revision,
        renderedText: result.sourceText,
        redrawMode: 'streaming',
        totalBytes,
        streamRunId,
        batch: result.batch,
        analysis: result.analysis,
        snapshotId: result.snapshotId,
        renderToken,
        freshness: operation.lifecycle,
      });
    } catch (error) {
      if (isExpectedRenderTermination(error, operation.lifecycle.isCurrent)) {
        console.debug('[graph] external render ended before landing', error);
        deps.completeStreamProgress();
        markGraphStreamDone();
        return null;
      }
      deps.handleError(error, {
        component: 'GraphViewer',
        operation: 'attachExternalDocumentJobSession',
        metadata: { documentKey: session.documentKey, revision: session.revision, sessionId: session.sessionId },
      });
      console.error('[graph] external document render failed', error);
      deps.clearErrorMessage();
      markGraphStreamFinal('exception', {
        failed: true,
      });
      deps.completeStreamProgress();
      markGraphStreamDone();
      return null;
    }
  }


  async function renderJsonBlockSelection(selection: JsonBlockSelection): Promise<GraphRenderResult | null> {
    if (!deps.hasRenderTarget()) return null;

    await dispose();
    const renderToken = ++activeRenderToken;
    clearWorkspaceSnapshotBinding(selection.sourceDocumentKey, activeSnapshotId);
    activeSnapshotId = null;
    deps.clearErrorMessage();
    deps.resetStreamProgress();

    const operation = createOwnedRenderOperation({
      captured: {
        documentKey: selection.blockDocumentKey,
        revision: selection.revision,
        sessionId: `${selection.blockDocumentKey}|${selection.revision}|${selection.startByte}|${selection.endByte}`,
        token: renderToken,
      },
      getCurrent: () => {
        const current = deps.getJsonBlockSelection();
        return {
          documentKey: current?.blockDocumentKey ?? null,
          revision: current?.revision,
          sessionId:
            current != null
              ? `${current.blockDocumentKey}|${current.revision}|${current.startByte}|${current.endByte}`
              : null,
          token: activeRenderToken,
        };
      },
    });
    activeOperation = operation;
    const totalBytes = textEncoder.encode(selection.text).length;
    const chunkSize = selectGraphStreamChunkSize(totalBytes);
    const streamRunId = nextStreamRunId(selection.blockDocumentKey, selection.revision);
    startGraphStreamMeasurement(
      selection.blockDocumentKey,
      selection.language,
      selection.revision,
      selection.text.length,
      totalBytes,
      chunkSize,
    );
    const applyProjectionEvents = createProjectionEventApplier(totalBytes, streamRunId, {
      documentKey: selection.blockDocumentKey,
      revision: selection.revision,
      mode: 'json-block',
    });

    try {
      const { started, advance } = await startSharedGraphDocumentJob({
        documentKey: selection.blockDocumentKey,
        language: selection.language,
        settings: buildDocumentJobSettings({
          enableNest: deps.getEnableNest(),
          formatting: defaultGraphDocumentFormatting,
          formatSourceOnClose: false,
        }),
        outputAnalysis: true,
        outputGraph: true,
        builderConfig: getBuilderConfig(),
      });
      operation.registerJobHandle(started.jobHandle);
      if (!operation.lifecycle.isCurrent()) return null;

      const streamedBatch = await streamDocumentJobText({
        jobHandle: started.jobHandle,
        text: selection.text,
        chunkSize,
        advance: async (input) => {
          recordAdvanceRequest(input.kind);
          const batch = await advance(input);
          recordAdvanceBatch(batch);
          return batch;
        },
        onBatch: async (batch) => {
          if (!operation.lifecycle.isCurrent()) return;
          await applyProjectionEvents(batch);
        },
      });
      if (!operation.lifecycle.isCurrent()) return null;

      const result = collectGraphDocumentJobResult({
        documentKey: selection.blockDocumentKey,
        language: selection.language,
        jobHandle: started.jobHandle,
        batches: [started.batch, streamedBatch],
      });
      deps.updateStreamProgress(
        buildGraphLifecycleProgressEvent(streamRunId, totalBytes, 'flushing'),
      );
      const finalEvent = finalDocumentEvent(result.batch);
      let finalSnapshotId: SnapshotId | null = null;
      renderedDocumentKey = selection.blockDocumentKey;
      if (finalEvent?.type === 'snapshotReady') {
        const snapshotId = (result.snapshotId ?? finalEvent.snapshotId) as SnapshotId;
        finalSnapshotId = snapshotId;
        activeSnapshotId = snapshotId;
        bindWorkspaceSnapshot({
          documentKey: selection.blockDocumentKey,
          revision: selection.revision,
          snapshotId,
        });
        const delta = projectionToRawGraphDelta(finalEvent.mainGraph);
        if (delta) {
          if (!isRawGraphDelta(delta)) {
            throw new Error('json block graph projection decode failed');
          }
          deps.updateStreamProgress(
            buildGraphLifecycleProgressEvent(streamRunId, totalBytes, 'finishing'),
          );
          await applyTrackedProjectionDelta(delta, {
            documentKey: selection.blockDocumentKey,
            revision: selection.revision,
            mode: 'json-block',
          });
        }
        markGraphStreamFinal('snapshot-ready');
      } else if (finalEvent?.type === 'parseFailed') {
        console.error('[graph] JSON block graph analysis failed', {
          documentKey: selection.blockDocumentKey,
          revision: selection.revision,
          diagnostics: result.analysis?.diagnostics,
        });
        deps.clearErrorMessage();
        markGraphStreamFinal('parse-failed', {
          failed: true,
        });
      }
      if (!operation.lifecycle.isCurrent()) return null;
      const requestId = deps.nextTreeToken();
      deps.clearTreeState(requestId, 'graph', selection.revision, finalSnapshotId);
      deps.completeStreamProgress();
      await flushSceneAndRedraw(
        'json-block',
        selection.revision,
        {
          documentKey: selection.blockDocumentKey,
          revision: selection.revision,
          snapshotId: finalSnapshotId,
          renderToken,
          mode: 'json-block',
        },
        {
          documentKey: selection.blockDocumentKey,
          revision: selection.revision,
          mode: 'json-block',
        },
        operation.lifecycle,
      );
      markGraphStreamDone();
      return getSceneBridge().getLastRenderedGraph() ?? emptyRenderResult();
    } catch (error) {
      if (isExpectedRenderTermination(error, operation.lifecycle.isCurrent)) {
        console.debug('[graph] JSON block render ended before landing', error);
        deps.completeStreamProgress();
        markGraphStreamDone();
        return null;
      }
      deps.handleError(error, {
        component: 'GraphViewer',
        operation: 'renderJsonBlockSelection',
        metadata: { blockDocumentKey: selection.blockDocumentKey, revision: selection.revision },
      });
      console.error('[graph] JSON block render failed', error);
      deps.clearErrorMessage();
      markGraphStreamFinal('exception', {
        failed: true,
      });
      deps.completeStreamProgress();
      markGraphStreamDone();
      return null;
    }
  }

  return {
    attachSceneBridge,
    emptyRenderResult,
    ensureDocumentKey,
    renderDocumentGraph,
    attachExternalDocumentJobSession,
    renderJsonBlockSelection,
    dispose,
    getActiveSnapshotId: () => activeSnapshotId,
  };
}
