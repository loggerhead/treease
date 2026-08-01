import { beforeEach, describe, expect, it, vi } from 'vitest';

const mockedCallWorker = vi.hoisted(() => vi.fn());

vi.mock('@core-wasm/index', () => ({
  SemType: { MAP: 1, SEQ: 2, STR: 3, INT: 4, FLOAT: 5, BOOLEAN: 6, NIL: 7 },
  TreeKind: { MAPPING: 10, SEQUENCE: 11 },
  GraphKind: { SCALAR: 0, TABLE: 1, OBJECT: 2 },
}));


vi.mock('../../store/workspace-store', () => ({
  bindWorkspaceSnapshot: vi.fn(),
  clearWorkspaceSnapshotBinding: vi.fn(),
}));

vi.mock('../../wasm/wasm-worker-singleton', () => ({
  getSharedWasmWorkerClient: vi.fn(async () => ({ call: mockedCallWorker })),
  getWorkerChunkSizeConfig: () => ({
    defaultChunkSize: 128 * 1024,
    largeFileThreshold: 4 * 1024 * 1024,
    largeFileChunkSize: 256 * 1024,
  }),
}));

import { createGraphRenderSession } from './graph-render-session';
import { bindWorkspaceSnapshot } from '../../store/workspace-store';
import { readRuntimeReadiness, resetRuntimeReadiness, syncRuntimeReadinessFromEditorState } from '../../test-bridge/runtime-readiness';

function startJobResult(jobHandle = 1, requestSeq = 1) {
  return {
    jobHandle,
    batch: {
      requestSeq,
      events: [],
      terminal: null,
    },
  };
}

function analysisPayload(overrides: Record<string, unknown> & { value?: unknown; valueJson?: string | null } = {}) {
  const hasValue = Object.prototype.hasOwnProperty.call(overrides, 'value');
  const valueJson =
    (typeof overrides.valueJson === 'string' || overrides.valueJson === null)
      ? overrides.valueJson
      : hasValue
        ? JSON.stringify(overrides.value ?? null)
        : JSON.stringify({ a: 1 });
  const { value: _value, ...rest } = overrides;
  return {
    tree: { kind: 10, semType: 1, tag: '', value: '', children: [] },
    valueJson,
    diagnostics: [],
    semanticTokens: { data: [], version: 1 },
    sourceByteLength: 9,
    language: 'json',
    ...rest,
  };
}

function snapshotReadyEvent(snapshotId = 1, overrides: Record<string, unknown> = {}) {
  const hasAnalysis = Object.prototype.hasOwnProperty.call(overrides, 'analysis');
  const hasMainGraph = Object.prototype.hasOwnProperty.call(overrides, 'mainGraph');
  return {
    type: 'snapshotReady' as const,
    snapshotId,
    analysis: hasAnalysis ? overrides.analysis : analysisPayload(),
    mainGraph: hasMainGraph ? overrides.mainGraph : projectionDelta(true),
  };
}

function projectionEvent(clear = true, graphData = graphDataWithNode()) {
  return {
    type: 'projectionDelta' as const,
    clear,
    graphData,
    patchSeq: 0,
    baseGraphVersion: 0,
    graphVersion: 0,
  };
}

function textChunkBatch(events: Array<Record<string, unknown>> = [], requestSeq = 1) {
  return {
    requestSeq,
    events,
    terminal: null,
  };
}

function closeCompletedBatch(snapshotId = 1, overrides: Record<string, unknown> = {}) {
  return {
    requestSeq: snapshotId,
    events: [snapshotReadyEvent(snapshotId, overrides)],
    terminal: { type: 'completed' as const },
  };
}

function closeParseFailedBatch(snapshotId = 1, analysisOverrides: Record<string, unknown> = {}) {
  return {
    requestSeq: snapshotId,
    events: [
      {
        type: 'parseFailed' as const,
        snapshotId,
        analysis: analysisPayload({ tree: null, value: null, ...analysisOverrides }),
      },
      projectionEvent(true, null as any),
    ],
    terminal: { type: 'parseFailed' as const },
  };
}

function graphDataWithNode() {
  return {
    nodesAdded: [
      {
        renderHandle: 1,
        kind: 0,
        depth: 0,
        boxArgs: { x: 0, y: 0, width: 100, height: 20, cornerRadius: 4 },
        path: [],
        meta: null,
        rows: [],
        table: null,
      },
    ],
    nodesUpdated: [],
    nodesRemoved: [],
    edgesAdded: [],
    edgesRemoved: [],
  };
}

function graphDataWithParentChildEdge(toX: number) {
  return {
    nodesAdded: [
      {
        renderHandle: 1,
        kind: 2,
        depth: 0,
        boxArgs: { x: 486, y: 0, width: 306, height: 156, cornerRadius: 8 },
        path: [{ tag: 0, key: 'root_step', index: 0 }],
        meta: null,
        rows: [
          {
            boxArgs: { x: 486, y: 106, width: 306, height: 32, cornerRadius: 0 },
            cellBoxArgs: { x: 486, y: 106, width: 306, height: 32, cornerRadius: 0 },
            cells: [],
          },
        ],
        table: null,
      },
      {
        renderHandle: 8,
        kind: 2,
        depth: 1,
        boxArgs: { x: 1572, y: 2230, width: 338, height: 156, cornerRadius: 8 },
        path: [
          { tag: 0, key: 'root_step', index: 0 },
          { tag: 0, key: 'metrics_info', index: 0 },
        ],
        meta: null,
        rows: [
          {
            boxArgs: { x: 1572, y: 2230, width: 338, height: 24, cornerRadius: 0 },
            cellBoxArgs: { x: 1572, y: 2230, width: 338, height: 24, cornerRadius: 0 },
            cells: [],
          },
        ],
        table: null,
      },
    ],
    nodesUpdated: [],
    nodesRemoved: [],
    edgesAdded: [
      {
        fromRenderHandle: 1,
        from: { kind: 2, path: [{ tag: 0, key: 'root_step', index: 0 }] },
        fromRow: 0,
        toRenderHandle: 8,
        to: {
          kind: 2,
          path: [
            { tag: 0, key: 'root_step', index: 0 },
            { tag: 0, key: 'metrics_info', index: 0 },
          ],
        },
        toRow: 0,
        bezierArgs: { fromX: 792, fromY: 122, c1x: 954, c1y: 122, c2x: 954, c2y: 2242, toX, toY: 2242 },
      },
    ],
    edgesRemoved: [],
  };
}


function projectionDelta(clear = true, graphData = graphDataWithNode()) {
  return {
    clear,
    graphData,
    patchSeq: 0,
    baseGraphVersion: 0,
    graphVersion: 0,
  };
}

function createDeps(overrides: Record<string, unknown> = {}) {
  let graphStreamState: Record<string, unknown> | null = null;
  return {
    getDocumentKey: () => 'test-key',
    getEnableNest: () => false,
    getRenderConfig: () => ({
      columns: { keyColumnMaxWidth: 100, valueColumnMaxWidth: 140 },
      layout: {
        rowHeight: 20, rowPaddingInline: 8, rowPaddingBlock: 4,
        nodeBorderWidth: 1, layerGapY: 16, layerGapX: 16,
        tableMaxHeight: 240, tableRowHeight: 20, tableHeaderHeight: 24,
        averageCharWidth: 8, baseFontSize: 14,
      },
      truncation: { metaPathMinSegments: 1, metaPathMinChars: 1, metaPathKeepTailSegments: 1 },
    }),
    getJsonBlockSelection: () => null,
    hasRenderTarget: () => true,
    shouldAttachGraphViewerTestHooks: () => false,
    getGraphStreamState: () => graphStreamState,
    replaceGraphStreamState: vi.fn((state: Record<string, unknown>) => {
      graphStreamState = state;
    }),
    nextTreeToken: vi.fn(() => 1),
    publishTreeState: vi.fn(() => true),
    clearTreeState: vi.fn(() => true),
    resetJsonBlockViewport: vi.fn(),
    callWorker: mockedCallWorker,
    onStreamFinalRedraw: vi.fn(),
    onStreamFinalAnalysis: vi.fn(),
    updateStreamProgress: vi.fn(),
    resetStreamProgress: vi.fn(),
    completeStreamProgress: vi.fn(),
    setErrorMessage: vi.fn(),
    clearErrorMessage: vi.fn(),
    handleError: vi.fn(),
    ...overrides,
  };
}

function createSceneBridge(overrides: Record<string, unknown> = {}) {
  return {
    applyGraphDelta: vi.fn(async () => {}),
    flushPendingRenderWork: vi.fn(async () => {}),
    cancelActiveRenderWork: vi.fn(),
    replaceRenderedGraph: vi.fn((value: any) => value),
    getLastRenderedGraph: () => ({ nodes: [], edges: [] }),
    ...overrides,
  };
}

describe('graph-render-session coordinator', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetRuntimeReadiness();
    syncRuntimeReadinessFromEditorState({
      documentKey: 'test-key',
      editorRevision: 5,
      fullEditUiState: {
        active: false,
        sessionId: null,
        ownerKey: null,
        documentKey: null,
        revision: 0,
        streamSeq: 0,
        inputByteLength: 0,
        modelVersionId: null,
        byteLength: 0,
        language: '',
        phase: 'idle',
        sessionKind: null,
        transportKind: null,
        reason: null,
      },
    });
  });

  it('renderDocumentGraph consumes SnapshotReady.mainGraph from the close batch', async () => {
    mockedCallWorker
      .mockResolvedValueOnce(startJobResult(1))
      .mockResolvedValueOnce(textChunkBatch())
      .mockResolvedValueOnce(closeCompletedBatch(1, { mainGraph: projectionDelta(true) }));

    const deps = createDeps();
    const bridge = createSceneBridge({ getLastRenderedGraph: () => ({ nodes: [{ id: 1 }], edges: [] }) });
    const coordinator = createGraphRenderSession(deps as any);
    coordinator.attachSceneBridge(bridge);

    const result = await coordinator.renderDocumentGraph({
      kind: 'incremental',
      documentKey: 'test-key',
      language: 'json',
      text: '{"a":1}',
      revision: 5,
    });

    expect(mockedCallWorker).toHaveBeenNthCalledWith(1, 'startDocumentJob', expect.objectContaining({
      documentKey: 'test-key',
      language: 'json',
      settings: expect.objectContaining({
        parser: expect.objectContaining({ enableNest: false }),
        formatting: expect.objectContaining({ smart: true }),
      }),
      outputAnalysis: true,
      outputGraph: true,
    }));
    expect(mockedCallWorker).toHaveBeenNthCalledWith(2, 'advanceDocumentJob', {
      jobHandle: 1,
      kind: 'textChunk',
      text: '{"a":1}',
    });
    expect(mockedCallWorker).toHaveBeenNthCalledWith(3, 'advanceDocumentJob', {
      jobHandle: 1,
      kind: 'close',
    });
    expect(bridge.applyGraphDelta).toHaveBeenCalledWith(expect.objectContaining({ clear: 1 }), undefined);
    expect(result).toEqual({ nodes: [{ id: 1 }], edges: [] });
  });

  it('renderDocumentGraph publishes flushing and finishing progress around the close snapshot', async () => {
    mockedCallWorker
      .mockResolvedValueOnce(startJobResult(11))
      .mockResolvedValueOnce(
        textChunkBatch([
          {
            type: 'progress',
            processedBytes: 7,
          },
        ]),
      )
      .mockResolvedValueOnce(closeCompletedBatch(11, { mainGraph: projectionDelta(true) }));

    const deps = createDeps();
    const coordinator = createGraphRenderSession(deps as any);
    coordinator.attachSceneBridge(createSceneBridge());

    await coordinator.renderDocumentGraph({
      kind: 'incremental',
      documentKey: 'test-key',
      language: 'json',
      text: '{"a":1}',
      revision: 5,
    });

    expect(deps.updateStreamProgress).toHaveBeenCalledWith(
      expect.objectContaining({
        phase: 'flushing',
        final: false,
        value: 99,
      }),
    );
    expect(deps.updateStreamProgress).toHaveBeenCalledWith(
      expect.objectContaining({
        phase: 'finishing',
        final: false,
        value: 99,
      }),
    );
  });

  it('renderDocumentGraph applies streamed graph deltas through the scene bridge', async () => {
    mockedCallWorker
      .mockResolvedValueOnce(startJobResult(2))
      .mockResolvedValueOnce(textChunkBatch([projectionEvent(false)]))
      .mockResolvedValueOnce(closeCompletedBatch(2, { mainGraph: projectionDelta(false) }));

    const deps = createDeps();
    const bridge = createSceneBridge({ getLastRenderedGraph: () => ({ nodes: [{ id: 1 }], edges: [] }) });
    const coordinator = createGraphRenderSession(deps as any);
    coordinator.attachSceneBridge(bridge);

    const renderResult = await coordinator.renderDocumentGraph({
      kind: 'incremental',
      documentKey: 'test-key',
      language: 'json',
      text: '{"a":1}',
      revision: 5,
    });

    expect(bridge.applyGraphDelta).toHaveBeenCalledWith(
      expect.objectContaining({ clear: 0 }),
      expect.objectContaining({ baseGraphVersion: 0, graphVersion: 0 }),
    );
    expect(renderResult).toEqual({ nodes: [{ id: 1 }], edges: [] });
    expect(deps.onStreamFinalRedraw).toHaveBeenCalledWith(
      'committed',
      5,
      expect.objectContaining({ documentKey: 'test-key', revision: 5, snapshotId: 2, mode: 'committed' }),
    );
    expect(readRuntimeReadiness().graph).toMatchObject({
      appliedRevision: 5,
      flushedRevision: 5,
    });
    expect(bindWorkspaceSnapshot).toHaveBeenCalledWith(
      expect.objectContaining({ documentKey: 'test-key', revision: 5 }),
    );
  });

  it('waits for pending scene render work before publishing final redraw', async () => {
    mockedCallWorker
      .mockResolvedValueOnce(startJobResult(23))
      .mockResolvedValueOnce(textChunkBatch([projectionEvent(false)]))
      .mockResolvedValueOnce(closeCompletedBatch(23, { mainGraph: projectionDelta(false) }));

    const deps = createDeps();
    let releaseFlush: (() => void) | null = null;
    const flushPendingRenderWork = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          releaseFlush = resolve;
        }),
    );
    const bridge = createSceneBridge({
      flushPendingRenderWork,
      getLastRenderedGraph: () => ({ nodes: [{ id: 1 }], edges: [] }),
    });
    const coordinator = createGraphRenderSession(deps as any);
    coordinator.attachSceneBridge(bridge);

    const renderPromise = coordinator.renderDocumentGraph({
      kind: 'incremental',
      documentKey: 'test-key',
      language: 'json',
      text: '{"a":1}',
      revision: 5,
    });

    await vi.waitFor(() => {
      expect(flushPendingRenderWork).toHaveBeenCalled();
    });
    expect(deps.onStreamFinalRedraw).not.toHaveBeenCalled();

    releaseFlush?.();
    await renderPromise;

    expect(deps.onStreamFinalRedraw).toHaveBeenCalledWith(
      'committed',
      5,
      expect.objectContaining({ documentKey: 'test-key', revision: 5, snapshotId: 23, mode: 'committed' }),
    );
  });

  it('applies the authoritative snapshot main graph after streamed projections', async () => {
    mockedCallWorker
      .mockResolvedValueOnce(startJobResult(22))
      .mockResolvedValueOnce(
        textChunkBatch([projectionEvent(false, graphDataWithParentChildEdge(1116))]),
      )
      .mockResolvedValueOnce(
        closeCompletedBatch(22, {
          mainGraph: projectionDelta(true, graphDataWithParentChildEdge(1572)),
        }),
      );

    const deps = createDeps();
    let renderedGraph: any = { nodes: [], edges: [] };
    const bridge = createSceneBridge({
      applyGraphDelta: vi.fn(async (delta: any) => {
        if (delta.clear === 1) {
          renderedGraph = {
            nodes: [...(delta.nodesAdded ?? [])],
            edges: [...(delta.edgesAdded ?? [])],
          };
          return;
        }
        renderedGraph = {
          nodes: [...(delta.nodesAdded ?? delta.nodesUpdated ?? [])],
          edges: [...(delta.edgesAdded ?? [])],
        };
      }),
      getLastRenderedGraph: () => renderedGraph,
    });
    // Test helper uses a structurally complete dependency object.
    const coordinator = createGraphRenderSession(deps as unknown as Parameters<typeof createGraphRenderSession>[0]);
    coordinator.attachSceneBridge(bridge);

    const result = await coordinator.renderDocumentGraph({
      kind: 'incremental',
      documentKey: 'test-key',
      language: 'json',
      text: '{"a":1}',
      revision: 5,
    });

    expect(result?.edges).toHaveLength(1);
    expect(result?.edges[0]?.bezierArgs?.toX).toBe(1572);
  });


  it('attachExternalDocumentJobSession consumes an existing document job without starting a text job', async () => {
    const streamedBatch = textChunkBatch(
      [projectionEvent(false, graphDataWithParentChildEdge(1116))],
      1,
    );
    const finalBatch = closeCompletedBatch(12, {
      mainGraph: projectionDelta(true, graphDataWithParentChildEdge(1572)),
    });
    const mergedBatch = {
      requestSeq: 12,
      events: [...streamedBatch.events, ...finalBatch.events],
      terminal: finalBatch.terminal,
    };
    const session = {
      sessionId: 'session-1',
      documentKey: 'test-key',
      language: 'json',
      revision: 5,
      totalBytes: 64,
      chunkSize: 32,
      streamRunId: 'session-1',
      jobHandle: null,
      hasCompleteReplay: () => true,
      result: Promise.resolve({
        batch: mergedBatch,
        analysis: { documentKey: 'test-key', language: 'json', tree: { kind: 10, semType: 1, tag: '', value: '', children: [] }, value: { a: 1 }, diagnostics: [], semanticTokens: new ArrayBuffer(0), semanticTokenVersion: 1, sourceByteLength: 9 },
        jobHandle: 12,
        snapshotId: 12,
      }),
      batches: async function* () {
        yield streamedBatch;
        yield finalBatch;
      },
      cancel: vi.fn(async () => {}),
    };

    const deps = createDeps();
    let renderedGraph: any = { nodes: [], edges: [] };
    const bridge = createSceneBridge({
      applyGraphDelta: vi.fn(async (delta: any) => {
        if (delta.clear === 1) {
          renderedGraph = {
            nodes: [...(delta.nodesAdded ?? [])],
            edges: [...(delta.edgesAdded ?? [])],
          };
          return;
        }
        renderedGraph = {
          nodes: [...(delta.nodesAdded ?? delta.nodesUpdated ?? [])],
          edges: [...(delta.edgesAdded ?? [])],
        };
      }),
      getLastRenderedGraph: () => renderedGraph,
    });
    // Test helper uses a structurally complete dependency object.
    const coordinator = createGraphRenderSession(deps as unknown as Parameters<typeof createGraphRenderSession>[0]);
    coordinator.attachSceneBridge(bridge);

    const renderResult = await coordinator.attachExternalDocumentJobSession(
      session as unknown as Parameters<typeof coordinator.attachExternalDocumentJobSession>[0],
    );

    expect(mockedCallWorker).not.toHaveBeenCalled();
    expect(deps.onStreamFinalAnalysis).toHaveBeenCalledWith(
      'test-key',
      'json',
      5,
      expect.objectContaining({ value: null, tree: null, language: 'json' }),
      12,
    );
    expect(deps.onStreamFinalRedraw).toHaveBeenCalledWith(
      'streaming',
      5,
      expect.objectContaining({ documentKey: 'test-key', revision: 5, snapshotId: 12, mode: 'streaming' }),
    );
    expect(renderResult?.edges).toHaveLength(1);
    expect(renderResult?.edges[0]?.bezierArgs?.toX).toBe(1572);
    await coordinator.dispose();
    expect(session.cancel).not.toHaveBeenCalled();
  });

  it('waits for the terminal render when an external session replay is incomplete', async () => {
    const finalBatch = closeCompletedBatch(13, {
      mainGraph: projectionDelta(true, graphDataWithParentChildEdge(1600)),
    });
    const batches = vi.fn(async function* () {
      throw new Error('a partial replay must not be projected');
    });
    const session = {
      sessionId: 'session-partial',
      documentKey: 'test-key',
      language: 'json',
      revision: 5,
      totalBytes: 64,
      streamRunId: 'session-partial',
      jobHandle: null,
      hasCompleteReplay: () => false,
      batches,
      result: Promise.resolve({
        batch: finalBatch,
        analysis: { documentKey: 'test-key', language: 'json', tree: { kind: 10, semType: 1, tag: '', value: '', children: [] }, value: { a: 1 }, diagnostics: [], semanticTokens: new ArrayBuffer(0), semanticTokenVersion: 1, sourceByteLength: 9 },
        jobHandle: 13,
        snapshotId: 13,
      }),
      cancel: vi.fn(async () => {}),
    };
    const deps = createDeps();
    const coordinator = createGraphRenderSession(deps as any);
    coordinator.attachSceneBridge(createSceneBridge());

    await coordinator.attachExternalDocumentJobSession(session as any);

    expect(batches).not.toHaveBeenCalled();
    expect(deps.onStreamFinalRedraw).toHaveBeenCalledWith(
      'streaming',
      5,
      expect.objectContaining({ snapshotId: 13 }),
    );
  });

  it('renderDocumentGraph publishes final analysis from snapshot events', async () => {
    const finalAnalysis = analysisPayload({ value: { a: 1 } });
    mockedCallWorker
      .mockResolvedValueOnce(startJobResult(3))
      .mockResolvedValueOnce(textChunkBatch())
      .mockResolvedValueOnce(closeCompletedBatch(3, { analysis: finalAnalysis, mainGraph: projectionDelta(true) }));

    const deps = createDeps();
    const coordinator = createGraphRenderSession(deps as any);
    coordinator.attachSceneBridge(createSceneBridge());

    await coordinator.renderDocumentGraph({
      kind: 'incremental',
      documentKey: 'test-key',
      language: 'json',
      text: '{"a":1}',
      revision: 5,
    });

    expect(deps.onStreamFinalAnalysis).toHaveBeenCalledWith(
      'test-key',
      'json',
      5,
      expect.objectContaining({ value: null, tree: null, language: 'json' }),
      3,
    );
  });

  it('renderDocumentGraph skips duplicate render for same documentKey and revision', async () => {
    mockedCallWorker
      .mockResolvedValueOnce(startJobResult(4))
      .mockResolvedValueOnce(textChunkBatch())
      .mockResolvedValueOnce(closeCompletedBatch(4, { mainGraph: projectionDelta(true) }));

    const deps = createDeps();
    const bridge = createSceneBridge({ getLastRenderedGraph: () => ({ nodes: [{ id: 1 }], edges: [] }) });
    const coordinator = createGraphRenderSession(deps as any);
    coordinator.attachSceneBridge(bridge);

    await coordinator.renderDocumentGraph({ kind: 'incremental', documentKey: 'test-key', language: 'json', text: '{"a":1}', revision: 5 });
    expect(mockedCallWorker).toHaveBeenCalledTimes(3);

    mockedCallWorker.mockClear();
    const second = await coordinator.renderDocumentGraph({ kind: 'incremental', documentKey: 'test-key', language: 'json', text: '{"a":1}', revision: 5 });

    expect(mockedCallWorker).not.toHaveBeenCalled();
    expect(second).toEqual({ nodes: [{ id: 1 }], edges: [] });
  });

  it('renderDocumentGraph reruns when text changes within the same revision', async () => {
    mockedCallWorker
      .mockResolvedValueOnce(startJobResult(40))
      .mockResolvedValueOnce(textChunkBatch())
      .mockResolvedValueOnce(closeCompletedBatch(40, { mainGraph: projectionDelta(true) }))
      .mockResolvedValueOnce(startJobResult(41))
      .mockResolvedValueOnce(textChunkBatch())
      .mockResolvedValueOnce(closeCompletedBatch(41, { mainGraph: projectionDelta(true) }));

    const deps = createDeps();
    const bridge = createSceneBridge({ getLastRenderedGraph: () => ({ nodes: [{ id: 1 }], edges: [] }) });
    const coordinator = createGraphRenderSession(deps as any);
    coordinator.attachSceneBridge(bridge);

    await coordinator.renderDocumentGraph({ kind: 'full-edit', documentKey: 'test-key', language: 'json', text: '{"a":', revision: 5 });
    mockedCallWorker.mockClear();

    await coordinator.renderDocumentGraph({ kind: 'full-edit', documentKey: 'test-key', language: 'json', text: '{"a":1}', revision: 5 });

    expect(mockedCallWorker).toHaveBeenCalledTimes(3);
  });
  it('renderDocumentGraph returns null when hasRenderTarget is false', async () => {
    const deps = createDeps({ hasRenderTarget: () => false });
    const coordinator = createGraphRenderSession(deps as any);

    const result = await coordinator.renderDocumentGraph({
      kind: 'incremental',
      documentKey: 'test-key',
      language: 'json',
      text: '{"a":1}',
      revision: 5,
    });

    expect(result).toBeNull();
    expect(mockedCallWorker).not.toHaveBeenCalled();
  });

  it('renderDocumentGraph sets error message when snapshotId is missing', async () => {
    mockedCallWorker
      .mockResolvedValueOnce(startJobResult(1))
      .mockResolvedValueOnce(textChunkBatch())
      .mockResolvedValueOnce({
        requestSeq: 1,
        events: [],
        terminal: { type: 'completed' as const },
      });

    const deps = createDeps();
    const coordinator = createGraphRenderSession(deps as any);
    coordinator.attachSceneBridge(createSceneBridge());

    await coordinator.renderDocumentGraph({
      kind: 'incremental',
      documentKey: 'test-key',
      language: 'json',
      text: '{"a":1}',
      revision: 5,
    });

    expect(deps.setErrorMessage).toHaveBeenCalledWith('Document analysis did not produce a snapshot');
  });

  it('ignores cancelled batches from a stale render', async () => {
    let resolveTextChunk: ((batch: ReturnType<typeof textChunkBatch>) => void) | null = null;
    mockedCallWorker.mockImplementation((method: string, input: any) => {
      if (method === 'startDocumentJob') return Promise.resolve(startJobResult(42));
      if (method === 'cancelDocumentJob') {
        return Promise.resolve({
          requestSeq: 42,
          events: [],
          terminal: { type: 'cancelled' as const },
        });
      }
      if (method === 'advanceDocumentJob' && input.kind === 'textChunk') {
        return new Promise((resolve) => {
          resolveTextChunk = resolve;
        });
      }
      if (method === 'advanceDocumentJob' && input.kind === 'close') {
        return Promise.resolve({
          requestSeq: 42,
          events: [],
          terminal: { type: 'cancelled' as const },
        });
      }
      return Promise.resolve(null);
    });

    const deps = createDeps();
    const coordinator = createGraphRenderSession(deps as any);
    coordinator.attachSceneBridge(createSceneBridge());

    const renderPromise = coordinator.renderDocumentGraph({
      kind: 'incremental',
      documentKey: 'test-key',
      language: 'json',
      text: '{"a":1}',
      revision: 5,
    });
    await vi.waitFor(() => expect(resolveTextChunk).not.toBeNull());

    await coordinator.dispose();
    resolveTextChunk?.(textChunkBatch());

    await expect(renderPromise).resolves.toBeNull();
    expect(mockedCallWorker.mock.calls.filter(([method]) => method === 'cancelDocumentJob')).toHaveLength(1);
    expect(deps.setErrorMessage).not.toHaveBeenCalled();
  });

  it('stale cleanup only cancels the job owned by the stale operation', async () => {
    let resolveFirstChunk: (() => void) | null = null;
    mockedCallWorker.mockImplementation((method: string, input: any) => {
      if (method === 'startDocumentJob') {
        const jobHandle = mockedCallWorker.mock.calls.filter(([name]) => name === 'startDocumentJob').length === 1 ? 42 : 43;
        return Promise.resolve(startJobResult(jobHandle));
      }
      if (method === 'cancelDocumentJob') return Promise.resolve({ terminal: { type: 'cancelled' } });
      if (method === 'advanceDocumentJob' && input.kind === 'textChunk' && input.jobHandle === 42) {
        return new Promise<void>((resolve) => {
          resolveFirstChunk = resolve;
        }).then(() => textChunkBatch());
      }
      if (method === 'advanceDocumentJob' && input.kind === 'textChunk') return Promise.resolve(textChunkBatch());
      if (method === 'advanceDocumentJob' && input.kind === 'close') {
        return Promise.resolve(closeCompletedBatch(input.jobHandle, { mainGraph: projectionDelta(true) }));
      }
      return Promise.resolve(null);
    });

    const coordinator = createGraphRenderSession(createDeps() as any);
    coordinator.attachSceneBridge(createSceneBridge());
    const staleRender = coordinator.renderDocumentGraph({
      kind: 'incremental',
      documentKey: 'test-key',
      language: 'json',
      text: '{"a":1}',
      revision: 5,
    });
    await vi.waitFor(() => expect(resolveFirstChunk).not.toBeNull());

    const currentRender = coordinator.renderDocumentGraph({
      kind: 'incremental',
      documentKey: 'test-key',
      language: 'json',
      text: '{"a":2}',
      revision: 6,
    });
    await currentRender;
    resolveFirstChunk?.();
    await staleRender;

    expect(mockedCallWorker).toHaveBeenCalledWith('cancelDocumentJob', { jobHandle: 42 });
    expect(mockedCallWorker).not.toHaveBeenCalledWith('cancelDocumentJob', { jobHandle: 43 });
  });

  it('cancels a handle that arrives after the operation became stale', async () => {
    let resolveStart: ((value: ReturnType<typeof startJobResult>) => void) | null = null;
    mockedCallWorker.mockImplementation((method: string) => {
      if (method === 'startDocumentJob') {
        return new Promise((resolve) => {
          resolveStart = resolve;
        });
      }
      if (method === 'cancelDocumentJob') return Promise.resolve({ terminal: { type: 'cancelled' } });
      return Promise.resolve(null);
    });

    const coordinator = createGraphRenderSession(createDeps() as any);
    const renderPromise = coordinator.renderDocumentGraph({
      kind: 'incremental',
      documentKey: 'test-key',
      language: 'json',
      text: '{"a":1}',
      revision: 5,
    });
    await vi.waitFor(() => expect(mockedCallWorker).toHaveBeenCalledWith('startDocumentJob', expect.anything()));

    const disposePromise = coordinator.dispose();
    resolveStart?.(startJobResult(44));
    await disposePromise;
    await renderPromise;
    await vi.waitFor(() => {
      expect(mockedCallWorker).toHaveBeenCalledWith('cancelDocumentJob', { jobHandle: 44 });
    });
  });

  it('renderDocumentGraph catches errors and delegates to handleError', async () => {
    const error = new Error('worker crash');
    mockedCallWorker.mockRejectedValueOnce(error);

    const deps = createDeps();
    const coordinator = createGraphRenderSession(deps as any);

    const result = await coordinator.renderDocumentGraph({
      kind: 'incremental',
      documentKey: 'test-key',
      language: 'json',
      text: '{"a":1}',
      revision: 5,
    });

    expect(result).toBeNull();
    expect(deps.handleError).toHaveBeenCalledWith(error, expect.objectContaining({
      component: 'GraphViewer',
      operation: 'renderDocumentGraph',
    }));
  });
  it('renderDocumentGraph fails when SnapshotReady.mainGraph is missing', async () => {
    const error = new Error('Document analysis did not produce requested main graph');
    mockedCallWorker
      .mockResolvedValueOnce(startJobResult(5))
      .mockResolvedValueOnce(textChunkBatch())
      .mockResolvedValueOnce(closeCompletedBatch(5, { mainGraph: null }));

    const deps = createDeps();
    const bridge = createSceneBridge();
    const coordinator = createGraphRenderSession(deps as any);
    coordinator.attachSceneBridge(bridge);

    const result = await coordinator.renderDocumentGraph({
      kind: 'incremental',
      documentKey: 'test-key',
      language: 'json',
      text: '{"a":1}',
      revision: 5,
    });

    expect(result).toBeNull();
    expect(bridge.replaceRenderedGraph).not.toHaveBeenCalled();
    expect(deps.handleError).toHaveBeenCalledWith(error, expect.objectContaining({
      component: 'GraphViewer',
      operation: 'renderDocumentGraph',
    }));
  });

  it('renderDocumentGraph clears graph on parse-failed snapshots', async () => {
    mockedCallWorker
      .mockResolvedValueOnce(startJobResult(5))
      .mockResolvedValueOnce(textChunkBatch())
      .mockResolvedValueOnce(
        closeParseFailedBatch(5, {
          diagnostics: [{ startLineNumber: 1, startColumn: 1, endLineNumber: 1, endColumn: 2, kind: 1 }],
        }),
      );

    const deps = createDeps();
    const bridge = createSceneBridge({ getLastRenderedGraph: () => ({ nodes: [], edges: [] }) });
    const coordinator = createGraphRenderSession(deps as any);
    coordinator.attachSceneBridge(bridge);

    const result = await coordinator.renderDocumentGraph({
      kind: 'incremental',
      documentKey: 'test-key',
      language: 'json',
      text: '{"a":1,',
      revision: 5,
    });

    expect(bridge.applyGraphDelta).toHaveBeenCalledWith(
      expect.objectContaining({
        clear: 1,
        nodesAdded: [],
        nodesUpdated: [],
        nodesRemoved: [],
        edgesAdded: [],
        edgesRemoved: [],
      }),
      expect.objectContaining({ baseGraphVersion: 0, graphVersion: 0 }),
    );
    expect(result).toEqual({ nodes: [], edges: [] });
  });


  it('dispose cancels active job and clears snapshot binding', async () => {
    mockedCallWorker
      .mockResolvedValueOnce(startJobResult(6))
      .mockResolvedValueOnce(textChunkBatch())
      .mockResolvedValueOnce(closeCompletedBatch(6, { mainGraph: projectionDelta(true) }));

    const deps = createDeps();
    const coordinator = createGraphRenderSession(deps as any);
    coordinator.attachSceneBridge(createSceneBridge());

    await coordinator.renderDocumentGraph({
      kind: 'incremental',
      documentKey: 'test-key',
      language: 'json',
      text: '{"a":1}',
      revision: 5,
    });

    await coordinator.dispose();

    expect(mockedCallWorker).not.toHaveBeenCalledWith('cancelDocumentJob', { jobHandle: 6 });
  });

  it('renderJsonBlockSelection streams block text through the job API', async () => {
    mockedCallWorker
      .mockResolvedValueOnce(startJobResult(7))
      .mockResolvedValueOnce(textChunkBatch())
      .mockResolvedValueOnce(closeCompletedBatch(7, { mainGraph: projectionDelta(true) }));

    const deps = createDeps({
      getJsonBlockSelection: () => ({ blockDocumentKey: 'block-key', revision: 8, startByte: 5, endByte: 25 }),
    });
    const coordinator = createGraphRenderSession(deps as any);
    coordinator.attachSceneBridge(createSceneBridge());

    await coordinator.renderJsonBlockSelection({
      sourceDocumentKey: 'source-key',
      blockDocumentKey: 'block-key',
      language: 'json',
      text: '{"nested":true}',
      revision: 8,
      startByte: 5,
      endByte: 25,
      startLineNumber: 0,
      startColumn: 5,
      endLineNumber: 0,
      endColumn: 25,
    });

    expect(mockedCallWorker).toHaveBeenNthCalledWith(1, 'startDocumentJob', expect.objectContaining({
      documentKey: 'block-key',
      language: 'json',
      settings: expect.objectContaining({
        parser: expect.objectContaining({ enableNest: false }),
        formatting: expect.objectContaining({ smart: true }),
      }),
      outputAnalysis: true,
      outputGraph: true,
    }));
    expect(mockedCallWorker).toHaveBeenNthCalledWith(2, 'advanceDocumentJob', {
      jobHandle: 7,
      kind: 'textChunk',
      text: '{"nested":true}',
    });
    expect(deps.onStreamFinalRedraw).toHaveBeenCalledWith(
      'json-block',
      8,
      expect.objectContaining({ documentKey: 'block-key', revision: 8, snapshotId: 7, mode: 'json-block' }),
    );
  });

  it('renderJsonBlockSelection resets stream progress before starting a new transient block render', async () => {
    mockedCallWorker
      .mockResolvedValueOnce(startJobResult(70))
      .mockResolvedValueOnce(textChunkBatch())
      .mockResolvedValueOnce(closeCompletedBatch(70, { mainGraph: projectionDelta(true) }));

    const deps = createDeps({
      getJsonBlockSelection: () => ({ blockDocumentKey: 'block-key', revision: 8, startByte: 5, endByte: 25 }),
    });
    const coordinator = createGraphRenderSession(deps as any);
    coordinator.attachSceneBridge(createSceneBridge());

    await coordinator.renderJsonBlockSelection({
      sourceDocumentKey: 'source-key',
      blockDocumentKey: 'block-key',
      language: 'json',
      text: '{"nested":true}',
      revision: 8,
      startByte: 5,
      endByte: 25,
      startLineNumber: 0,
      startColumn: 5,
      endLineNumber: 0,
      endColumn: 25,
    });

    expect(deps.resetStreamProgress).toHaveBeenCalledTimes(1);
  });

  it('renderJsonBlockSelection completes stream progress when the block job reaches a final snapshot', async () => {
    mockedCallWorker
      .mockResolvedValueOnce(startJobResult(7))
      .mockResolvedValueOnce(
        textChunkBatch([
          {
            type: 'progress',
            processedBytes: 13,
          },
        ]),
      )
      .mockResolvedValueOnce(closeCompletedBatch(7, { mainGraph: projectionDelta(true) }));

    const deps = createDeps({
      getJsonBlockSelection: () => ({ blockDocumentKey: 'block-key', revision: 8, startByte: 5, endByte: 25 }),
    });
    const coordinator = createGraphRenderSession(deps as any);
    coordinator.attachSceneBridge(createSceneBridge());

    await coordinator.renderJsonBlockSelection({
      sourceDocumentKey: 'source-key',
      blockDocumentKey: 'block-key',
      language: 'json',
      text: '{"nested":true}',
      revision: 8,
      startByte: 5,
      endByte: 25,
      startLineNumber: 0,
      startColumn: 5,
      endLineNumber: 0,
      endColumn: 25,
    });

    expect(deps.updateStreamProgress).toHaveBeenCalledWith(
      expect.objectContaining({
        phase: 'streaming',
        processedBytes: 13,
        totalBytes: 15,
        final: false,
      }),
    );
    expect(deps.completeStreamProgress).toHaveBeenCalledTimes(1);
    expect(deps.onStreamFinalRedraw).toHaveBeenCalledWith(
      'json-block',
      8,
      expect.objectContaining({ documentKey: 'block-key', revision: 8, snapshotId: 7, mode: 'json-block' }),
    );
  });

  it('renderJsonBlockSelection publishes tree state when analysis is available', async () => {
    const tree = { kind: 10, semType: 1, tag: '', value: '', children: [] };
    mockedCallWorker
      .mockResolvedValueOnce(startJobResult(8))
      .mockResolvedValueOnce(textChunkBatch())
      .mockResolvedValueOnce(
        closeCompletedBatch(8, {
          analysis: analysisPayload({ tree, value: { nested: true } }),
          mainGraph: projectionDelta(true),
        }),
      );

    const deps = createDeps({
      getJsonBlockSelection: () => ({ blockDocumentKey: 'block-key', revision: 8, startByte: 5, endByte: 25 }),
    });
    const coordinator = createGraphRenderSession(deps as any);
    coordinator.attachSceneBridge(createSceneBridge());

    await coordinator.renderJsonBlockSelection({
      sourceDocumentKey: 'source-key',
      blockDocumentKey: 'block-key',
      language: 'json',
      text: '{"nested":true}',
      revision: 8,
      startByte: 5,
      endByte: 25,
      startLineNumber: 0,
      startColumn: 5,
      endLineNumber: 0,
      endColumn: 25,
    });

    expect(deps.publishTreeState).not.toHaveBeenCalled();
    expect(deps.clearTreeState).toHaveBeenCalledWith(1, 'graph', 8, 8);
  });
  it('renderDocumentGraph keeps parse-failed snapshots in diagnostics flow without raw UI error', async () => {
    const diagnostics = [{ startLineNumber: 1, startColumn: 1, endLineNumber: 1, endColumn: 2, kind: 1 }];
    mockedCallWorker
      .mockResolvedValueOnce(startJobResult(10))
      .mockResolvedValueOnce(textChunkBatch())
      .mockResolvedValueOnce(closeParseFailedBatch(10, {
        diagnostics,
      }));

    const deps = createDeps();
    const coordinator = createGraphRenderSession(deps as any);
    coordinator.attachSceneBridge(createSceneBridge());

    await coordinator.renderDocumentGraph({
      kind: 'incremental',
      documentKey: 'test-key',
      language: 'json',
      text: '{"a":1,',
      revision: 6,
    });

    expect(deps.setErrorMessage).not.toHaveBeenCalled();
    expect(deps.onStreamFinalAnalysis).toHaveBeenCalledWith(
      'test-key',
      'json',
      6,
      expect.objectContaining({
        diagnostics,
      }),
      null,
    );
  });
  it('renderDocumentGraph does not leak parse-failed raw UI error over a later success for the same revision', async () => {
    let resolveFirstDelta: (() => void) | null = null;
    const firstDeltaApplied = new Promise<void>((resolve) => {
      resolveFirstDelta = resolve;
    });
    const bridge = createSceneBridge({
      applyGraphDelta: vi
        .fn()
        .mockImplementationOnce(() => firstDeltaApplied)
        .mockImplementation(async () => {}),
    });
    mockedCallWorker
      .mockResolvedValueOnce(startJobResult(20))
      .mockResolvedValueOnce(textChunkBatch())
      .mockResolvedValueOnce(closeCompletedBatch(20, { mainGraph: projectionDelta(true) }))
      .mockResolvedValueOnce(startJobResult(21))
      .mockResolvedValueOnce(textChunkBatch())
      .mockResolvedValueOnce(closeParseFailedBatch(21, {
        diagnostics: [{ startLineNumber: 1, startColumn: 1, endLineNumber: 1, endColumn: 2, kind: 1 }],
      }));

    const deps = createDeps();
    const coordinator = createGraphRenderSession(deps as any);
    coordinator.attachSceneBridge(bridge);

    const firstRender = coordinator.renderDocumentGraph({
      kind: 'full-edit',
      documentKey: 'test-key',
      language: 'json',
      text: '{"a":1}',
      revision: 7,
    });
    await vi.waitFor(() => {
      expect(bridge.applyGraphDelta).toHaveBeenCalledTimes(1);
    });

    await coordinator.renderDocumentGraph({
      kind: 'incremental',
      documentKey: 'test-key',
      language: 'json',
      text: '{"a":1}',
      revision: 7,
    });

    expect(deps.setErrorMessage).not.toHaveBeenCalled();

    resolveFirstDelta?.();
    await firstRender;
  });

  it('renderJsonBlockSelection binds the block snapshot for downstream queries', async () => {
    mockedCallWorker
      .mockResolvedValueOnce(startJobResult(11))
      .mockResolvedValueOnce(textChunkBatch())
      .mockResolvedValueOnce(closeCompletedBatch(11, {
        analysis: analysisPayload({ tree: { kind: 10, semType: 1, tag: '', value: '', children: [] }, value: { nested: true } }),
        mainGraph: projectionDelta(true),
      }));

    const deps = createDeps({
      getJsonBlockSelection: () => ({ blockDocumentKey: 'block-key', revision: 8, startByte: 5, endByte: 25 }),
    });
    const coordinator = createGraphRenderSession(deps as any);
    coordinator.attachSceneBridge(createSceneBridge());

    await coordinator.renderJsonBlockSelection({
      sourceDocumentKey: 'source-key',
      blockDocumentKey: 'block-key',
      language: 'json',
      text: '{"nested":true}',
      revision: 8,
      startByte: 5,
      endByte: 25,
      startLineNumber: 0,
      startColumn: 5,
      endLineNumber: 0,
      endColumn: 25,
    });

    expect(bindWorkspaceSnapshot).toHaveBeenCalledWith(
      expect.objectContaining({ documentKey: 'block-key', revision: 8, snapshotId: 11 }),
    );
    expect(coordinator.getActiveSnapshotId()).toBe(11);
  });

  it('getActiveSnapshotId reflects the latest rendered snapshot', async () => {
    mockedCallWorker
      .mockResolvedValueOnce(startJobResult(9))
      .mockResolvedValueOnce(textChunkBatch())
      .mockResolvedValueOnce(closeCompletedBatch(9, { mainGraph: projectionDelta(true) }));

    const deps = createDeps();
    const coordinator = createGraphRenderSession(deps as any);
    coordinator.attachSceneBridge(createSceneBridge());

    expect(coordinator.getActiveSnapshotId()).toBeNull();

    await coordinator.renderDocumentGraph({
      kind: 'incremental',
      documentKey: 'test-key',
      language: 'json',
      text: '{"a":1}',
      revision: 5,
    });

    expect(coordinator.getActiveSnapshotId()).toBe(9);
  });
});
