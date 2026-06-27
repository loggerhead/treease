// 职责：GraphViewer 渲染管理：直接消费 Document Job streaming events 与 SnapshotReady.mainGraph
// 自启动 text job 与外部 full-edit document job session 共用同一 projection apply 管线；不使用 buildProjection('mainGraph') fallback。
import { type BuilderConfig, type EventBatch, type SnapshotId, type TreeNode } from '@core-wasm/index';
import { selectGraphStreamChunkSize } from '../../graph-stream/chunk-size-policy';
import type { GraphViewerConfig } from '../../settings/ui-settings';
import type { GraphEdge, GraphNode } from '../../graph/graph-viewer-render';
import type { JsonBlockSelection } from '../../store/editor-store';
import type {
  DocumentAnalysisResult,
  RawGraphDelta,
} from '../../../shared/worker-protocol/protocol';
import { isRawGraphDelta } from '../../../shared/worker-protocol/graph-delta-normalize';
import { buildGraphStreamBuilderConfig } from '../../graph-stream/graph-stream-builder-config';
import { processGraphBatchEvents, projectionToRawGraphDelta } from '../../../shared/document-job-graph-stream';
import { bindActiveDocumentSnapshotIfPresent, clearActiveDocumentSnapshot } from '../../services/DocumentSessionService';
import { createFreshnessScope } from '../../guards/freshness-scope';
import { streamDocumentJobText, type AdvanceDocumentJobRequest } from '../../../shared/document-job-stream';
import type { FullEditDocumentJobSession } from '../../graph-stream/full-edit-document-job-session';
import { buildDocumentJobSettings, collectGraphDocumentJobResult, startSharedGraphDocumentJob } from '../../graph-stream/document-job-runner';
import { normalizeDocumentJobAnalysisPayload } from '../../../shared/document-job-result';

const defaultGraphDocumentFormatting = {
  indent: 2,
  smart: true,
  maxLineLength: 100,
  maxInlineComplexity: 1,
  maxArrayInlineItems: 6,
  alignObjectArrays: true,
};

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

  let activeJobHandle: number | null = null;

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
    freshness?: { isCurrent: () => boolean },
  ): Promise<void> {
    await getSceneBridge().flushPendingRenderWork();
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
    version?: { baseGraphVersion: number; graphVersion: number },
  ): Promise<void> {
    const startedAtMs = performance.now();
    performance.mark('pipeline:apply-graph-delta:start');
    await getSceneBridge().applyGraphDelta(delta, version);
    performance.mark('pipeline:apply-graph-delta:end');
    performance.measure('pipeline:apply-graph-delta', 'pipeline:apply-graph-delta:start', 'pipeline:apply-graph-delta:end');
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
  ) {
    return async (batch: EventBatch): Promise<void> => {
      await processGraphBatchEvents(batch, totalBytes, {
        onProgress: (processedBytes: number, total: number) => {
          deps.updateStreamProgress(buildGraphProgressEvent(streamRunId, processedBytes, total));
        },
        onProjection: async (delta, version) => {
          await applyTrackedProjectionDelta(delta, version);
        },
      });
    };
  }

  async function consumeGraphBatchStream(params: {
    batches: AsyncIterable<EventBatch>;
    freshness: ReturnType<typeof createFreshnessScope>;
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
    batch: EventBatch;
    analysis: DocumentAnalysisResult | null;
    snapshotId: SnapshotId | null;
    renderToken: number;
    freshness: ReturnType<typeof createFreshnessScope>;
  }): Promise<GraphRenderResult | null> {
    const finalEvent = finalDocumentEvent(params.batch);
    const snapshotId = params.snapshotId ?? extractSnapshotIdFromBatch(params.batch);
    if (snapshotId == null || finalEvent == null) {
      deps.setErrorMessage('Document analysis did not produce a snapshot');
      deps.completeStreamProgress();
      return null;
    }

    if (finalEvent.type === 'snapshotReady') {
      if (!finalEvent.mainGraph) {
        throw new Error('Document analysis did not produce requested main graph');
      }
      const finalDelta = projectionToRawGraphDelta(finalEvent.mainGraph);
      if (!finalDelta || !isRawGraphDelta(finalDelta)) {
        throw new Error('document graph projection decode failed');
      }
      // SnapshotReady.mainGraph is the authoritative final graph. Always apply
      // it so streaming-time approximation or stale edge geometry cannot leak
      // into the settled scene state.
      await applyTrackedProjectionDelta(finalDelta);
      if (!params.freshness.isCurrent()) return null;
      deps.clearErrorMessage();
      clearGraphStreamFailure();
      markGraphStreamFinal('snapshot-ready');
    } else {
      markGraphStreamFinal('parse-failed', {
        failed: true,
      });
    }

    activeSnapshotId = snapshotId;
    renderedDocumentKey = params.documentKey;
    renderedRevision = params.revision;
    renderedLanguage = params.language;
    renderedText = params.renderedText;
    bindActiveDocumentSnapshotIfPresent({
      documentKey: params.documentKey,
      revision: params.revision,
      snapshotId,
    });

    deps.onStreamFinalAnalysis(
      params.documentKey,
      params.language,
      params.revision,
      params.analysis ?? normalizeDocumentJobAnalysisPayload(params.documentKey, params.language, finalEvent.analysis),
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
      params.freshness,
    );
    performance.mark('pipeline:render-document-graph:end');
    performance.measure('pipeline:render-document-graph', 'pipeline:render-document-graph:start', 'pipeline:render-document-graph:end');
    markGraphStreamDone();

    return getSceneBridge().getLastRenderedGraph() ?? emptyRenderResult();
  }

  async function cancelStartedJob(jobHandle: number | null): Promise<null> {
    if (jobHandle == null) return null;
    try {
      await deps.callWorker('cancelDocumentJob', { jobHandle });
    } catch {
      // Best-effort cancellation for stale starts.
    } finally {
      if (activeJobHandle === jobHandle) activeJobHandle = null;
    }
    return null;
  }

  async function dispose(): Promise<void> {
    activeRenderToken += 1;
    if (activeJobHandle != null) {
      const jobHandle = activeJobHandle;
      try {
        await deps.callWorker('cancelDocumentJob', { jobHandle });
      } catch {
        // Best-effort cleanup.
      } finally {
        if (activeJobHandle === jobHandle) activeJobHandle = null;
      }
    }
    if (renderedDocumentKey) {
      clearActiveDocumentSnapshot(renderedDocumentKey, activeSnapshotId);
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
    performance.mark('pipeline:render-document-graph:start');

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

    const freshness = createFreshnessScope(
      {
        documentKey: request.documentKey,
        token: renderToken,
      },
      () => ({
        documentKey: deps.getDocumentKey(),
        token: activeRenderToken,
      }),
    );

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
      const applyProjectionEvents = createProjectionEventApplier(totalBytes, streamRunId);

      const builderConfig = getBuilderConfig();
      performance.mark('pipeline:wasm-start-job:start');
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
      performance.mark('pipeline:wasm-start-job:end');
      performance.measure('pipeline:wasm-start-job', 'pipeline:wasm-start-job:start', 'pipeline:wasm-start-job:end');
      if (!freshness.isCurrent()) return cancelStartedJob(started.jobHandle);
      activeJobHandle = started.jobHandle;

      const streamedBatches = await streamDocumentJobText({
        jobHandle: started.jobHandle,
        text: request.text,
        chunkSize,
        advance: async (input) => {
          recordAdvanceRequest(input.kind);
          if (input.kind === 'close') {
            performance.mark('pipeline:close-advance:start');
          } else {
            performance.mark('pipeline:wasm-advance:start');
          }
          const batch = await advance(input);
          if (input.kind === 'close') {
            performance.mark('pipeline:close-advance:end');
            performance.measure('pipeline:close-advance', 'pipeline:close-advance:start', 'pipeline:close-advance:end');
          } else {
            performance.mark('pipeline:wasm-advance:end');
            performance.measure('pipeline:wasm-advance', 'pipeline:wasm-advance:start', 'pipeline:wasm-advance:end');
          }
          recordAdvanceBatch(batch);
          return batch;
        },
        onBatch: async (batch) => {
          if (!freshness.isCurrent()) return;
          await applyProjectionEvents(batch);
        },
      });
      if (!freshness.isCurrent()) return cancelStartedJob(started.jobHandle);
      if (activeJobHandle === started.jobHandle) activeJobHandle = null;

      const result = collectGraphDocumentJobResult({
        documentKey: request.documentKey,
        language: request.language,
        jobHandle: started.jobHandle,
        batches: [started.batch, ...streamedBatches],
      });
      return await finalizeGraphDocumentJob({
        documentKey: request.documentKey,
        language: request.language,
        revision: request.revision,
        renderedText: request.text,
        redrawMode: request.kind === 'incremental' ? 'committed' : 'streaming',
        batch: result.batch,
        analysis: result.analysis,
        snapshotId: result.snapshotId,
        renderToken,
        freshness,
      });
    } catch (error) {
      deps.handleError(error, {
        component: 'GraphViewer',
        operation: 'renderDocumentGraph',
        metadata: { documentKey: request.documentKey, revision: request.revision },
      });
      markGraphStreamFinal('exception', {
        errorMessage: error instanceof Error ? error.message : String(error),
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
    performance.mark('pipeline:render-document-graph:start');

    const freshness = createFreshnessScope(
      {
        documentKey: session.documentKey,
        revision: session.revision,
        sessionId: session.sessionId,
        token: renderToken,
      },
      () => ({
        documentKey: deps.getDocumentKey(),
        revision: session.revision,
        sessionId: activeExternalSessionId,
        token: activeRenderToken,
      }),
    );

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
    const applyProjectionEvents = createProjectionEventApplier(totalBytes, streamRunId);

    try {
      await consumeGraphBatchStream({
        batches: session.batches(),
        freshness,
        applyProjectionEvents,
        onBatch: (batch) => {
          recordAdvanceBatch(batch);
        },
      });
      if (!freshness.isCurrent()) return null;

      const result = await session.result;
      if (!freshness.isCurrent()) return null;

      return await finalizeGraphDocumentJob({
        documentKey: session.documentKey,
        language: session.language,
        revision: session.revision,
        renderedText: null,
        redrawMode: 'streaming',
        batch: result.batch,
        analysis: result.analysis,
        snapshotId: result.snapshotId,
        renderToken,
        freshness,
      });
    } catch (error) {
      deps.handleError(error, {
        component: 'GraphViewer',
        operation: 'attachExternalDocumentJobSession',
        metadata: { documentKey: session.documentKey, revision: session.revision, sessionId: session.sessionId },
      });
      markGraphStreamFinal('exception', {
        errorMessage: error instanceof Error ? error.message : String(error),
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
    clearActiveDocumentSnapshot(selection.sourceDocumentKey, activeSnapshotId);
    activeSnapshotId = null;
    deps.clearErrorMessage();
    deps.resetStreamProgress();

    const freshness = createFreshnessScope(
      {
        documentKey: selection.blockDocumentKey,
        revision: selection.revision,
        sessionId: `${selection.blockDocumentKey}|${selection.revision}|${selection.startByte}|${selection.endByte}`,
        token: renderToken,
      },
      () => {
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
    );
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
    const applyProjectionEvents = createProjectionEventApplier(totalBytes, streamRunId);

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
      if (!freshness.isCurrent()) return cancelStartedJob(started.jobHandle);
      activeJobHandle = started.jobHandle;

      const streamedBatches = await streamDocumentJobText({
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
          if (!freshness.isCurrent()) return;
          await applyProjectionEvents(batch);
        },
      });
      if (!freshness.isCurrent()) return cancelStartedJob(started.jobHandle);
      if (activeJobHandle === started.jobHandle) activeJobHandle = null;

      const result = collectGraphDocumentJobResult({
        documentKey: selection.blockDocumentKey,
        language: selection.language,
        jobHandle: started.jobHandle,
        batches: [started.batch, ...streamedBatches],
      });
      const finalEvent = finalDocumentEvent(result.batch);
      const snapshotId = result.snapshotId ?? extractSnapshotIdFromBatch(result.batch);
      renderedDocumentKey = selection.blockDocumentKey;
      if (snapshotId != null) {
        activeSnapshotId = snapshotId;
        bindActiveDocumentSnapshotIfPresent({
          documentKey: selection.blockDocumentKey,
          revision: selection.revision,
          snapshotId,
        });
      }
      if (finalEvent?.type === 'snapshotReady') {
        const delta = projectionToRawGraphDelta(finalEvent.mainGraph);
        if (delta) {
          if (!isRawGraphDelta(delta)) {
            throw new Error('json block graph projection decode failed');
          }
          await applyTrackedProjectionDelta(delta);
        }
        markGraphStreamFinal('snapshot-ready');
      } else if (finalEvent?.type === 'parseFailed') {
        deps.setErrorMessage('JSON block graph analysis failed');
        markGraphStreamFinal('parse-failed', {
          errorMessage: 'JSON block graph analysis failed',
          failed: true,
        });
      }
      const analysis = result.analysis ?? normalizeDocumentJobAnalysisPayload(
        selection.blockDocumentKey,
        selection.language,
        finalEvent?.analysis,
      );
      if (!freshness.isCurrent()) return null;
      const requestId = deps.nextTreeToken();
      if (analysis?.tree) {
        deps.publishTreeState(requestId, analysis.tree as TreeNode, analysis.value, 'graph', selection.revision, snapshotId);
      } else {
        deps.clearTreeState(requestId, 'graph', selection.revision, snapshotId);
        deps.setErrorMessage('JSON block graph analysis unavailable');
      }
      deps.completeStreamProgress();
      await flushSceneAndRedraw(
        'json-block',
        selection.revision,
        {
          documentKey: selection.blockDocumentKey,
          revision: selection.revision,
          snapshotId: snapshotId ?? null,
          renderToken,
          mode: 'json-block',
        },
        freshness,
      );
      markGraphStreamDone();
      return getSceneBridge().getLastRenderedGraph() ?? emptyRenderResult();
    } catch (error) {
      deps.handleError(error, {
        component: 'GraphViewer',
        operation: 'renderJsonBlockSelection',
        metadata: { blockDocumentKey: selection.blockDocumentKey, revision: selection.revision },
      });
      markGraphStreamFinal('exception', {
        errorMessage: error instanceof Error ? error.message : String(error),
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

function extractSnapshotIdFromBatch(batch: EventBatch): SnapshotId | null {
  for (let index = batch.events.length - 1; index >= 0; index -= 1) {
    const event = batch.events[index];
    if (event.type === 'snapshotReady') return event.snapshotId as SnapshotId;
    if (event.type === 'parseFailed' && event.snapshotId != null) return event.snapshotId as SnapshotId;
  }
  return null;
}
