import { readFileSync } from 'node:fs';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const mockStartDocumentJobForGraph = vi.hoisted(() =>
  vi.fn().mockResolvedValue({ snapshotId: 1, analysis: null, batch: { requestSeq: 1, events: [], terminal: null }, jobHandle: 1 }),
);
const mockStartReadableDocumentJobSessionForGraph = vi.hoisted(() =>
  vi.fn((input: any) => ({
    sessionId: input.sessionId,
    documentKey: input.documentKey,
    language: input.language,
    revision: input.revision,
    totalBytes: input.totalBytes ?? 0,
    chunkSize: input.chunkSize,
    streamRunId: input.sessionId,
    jobHandle: 2,
    result: Promise.resolve({
      snapshotId: 1,
      analysis: null,
      batch: { requestSeq: 1, events: [], terminal: null },
      jobHandle: 2,
    }),
    batches: async function* () {},
    cancel: vi.fn(async () => {}),
  })),
);
const mockClearFullEditDocumentJobSession = vi.hoisted(() => vi.fn());
const mockApplyGraphAnalysis = vi.hoisted(() => vi.fn(async () => {}));

vi.mock('../../graph-stream/document-job-runner', () => ({
  runTextDocumentJobForGraph: (input: any) => mockStartDocumentJobForGraph(input),
  buildDocumentJobSettings: vi.fn((input: any) => ({
    parser: { enableNest: input.enableNest, nestMaxDepth: 8 },
    formatting: {
      indent: input.formatting.indent,
      smart: input.formatting.smart,
      formatSourceOnClose: input.formatSourceOnClose ?? true,
      maxLineLength: input.formatting.maxLineLength,
      maxInlineComplexity: input.formatting.maxInlineComplexity,
      maxArrayInlineItems: input.formatting.maxArrayInlineItems,
      alignObjectArrays: input.formatting.alignObjectArrays,
    },
  })),
}));

vi.mock('../../graph-stream/full-edit-document-job-session', () => ({
  startReadableDocumentJobSessionForGraph: (input: any) => mockStartReadableDocumentJobSessionForGraph(input),
  clearFullEditDocumentJobSession: (sessionId: any, expected?: any) =>
    mockClearFullEditDocumentJobSession(sessionId, expected),
}));

import {
  clearActiveDocumentSnapshot,
  getActiveDocumentSnapshotId,
} from '../../services/DocumentSessionService';
import { editorStore, type FullEditUiState } from '../../store/editor-store';
import { createEditorFullEditController } from './editor-full-edit-controller';
import type { FullEditSink } from './editor-full-edit-sink';

describe('editor-full-edit-controller', () => {
  const originalRequestAnimationFrame = globalThis.requestAnimationFrame;
  const originalCancelAnimationFrame = globalThis.cancelAnimationFrame;

  beforeEach(() => {
    vi.clearAllMocks();
    editorStore.reset();
    clearActiveDocumentSnapshot('doc-test');
    clearActiveDocumentSnapshot('sidecar:tab-sidecar:0');
    globalThis.requestAnimationFrame = ((callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    }) as typeof requestAnimationFrame;
    globalThis.cancelAnimationFrame = vi.fn() as typeof cancelAnimationFrame;
  });

  afterEach(() => {
    globalThis.requestAnimationFrame = originalRequestAnimationFrame;
    globalThis.cancelAnimationFrame = originalCancelAnimationFrame;
  });

  function createOptions(overrides: Record<string, unknown> = {}) {
    let modelValue = '{"a":1}';
    const model = {
      uri: { toString: () => 'model://test' },
      getLineCount: () => 3,
      getLineMaxColumn: (_line: number) => 10,
      pushEditOperations: vi.fn((_before: unknown, edits: Array<{ text: string }>) => {
        modelValue += edits.map((edit) => edit.text).join('');
      }),
      getVersionId: () => 1,
      getValue: () => modelValue,
    };
    return {
      getModel: () => model,
      getEditor: () => ({ updateOptions: vi.fn(), setValue: vi.fn() }) as any,
      getMonaco: () => ({ Range: class { constructor(..._args: unknown[]) {} } }) as any,
      getLanguageId: () => 'json' as any,
      getNestEnabled: () => false,
      getGraphBuilderConfig: () => ({}),
      getFullEditUiState: () => editorStore.get().fullEditUiState,
      rotateActiveDocumentKey: () => 'doc-test',
      setModelDocumentKey: vi.fn(),
      clearSemanticTokensForDocument: vi.fn(),
      setEditorValue: vi.fn((value: string) => {
        modelValue = value;
        return true;
      }),
      setSourceText: vi.fn(),
      setDocumentKey: vi.fn(),
      applyImportLanguage: vi.fn(),
      updateActiveTempModel: vi.fn(),
      commitEditorState: vi.fn(() => 5),
      callWasmWorker: vi.fn(async () => 'converted text'),
      applyGraphAnalysis: mockApplyGraphAnalysis,
      setActiveTabDocumentKey: vi.fn(),
      ...overrides,
    };
  }

  function createReadableFile(chunks: string[], name = 'data.json', sizeOverride?: number) {
    const encoder = new TextEncoder();
    const encodedChunks = chunks.map((chunk) => encoder.encode(chunk));
    const byteLength = encodedChunks.reduce((total, chunk) => total + chunk.byteLength, 0);
    return {
      name,
      size: sizeOverride ?? byteLength,
      stream: () =>
        new ReadableStream<Uint8Array>({
          start(controller) {
            for (const chunk of encodedChunks) controller.enqueue(chunk);
            controller.close();
          },
      }),
    };
  }

  function createIdleFullEditUiState(): FullEditUiState {
    return {
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
    };
  }

  function spyOnLegacyFullEditActions() {
    return [
      vi.spyOn(editorStore.actions, 'beginFullEditStream'),
      vi.spyOn(editorStore.actions, 'appendFullEditStreamChunkMeta'),
      vi.spyOn(editorStore.actions, 'markFullEditStreamFinalizing'),
      vi.spyOn(editorStore.actions, 'finishFullEditStream'),
      vi.spyOn(editorStore.actions, 'cancelFullEditStream'),
    ];
  }

  function expectLegacyFullEditActionsNotCalled(spies: ReturnType<typeof spyOnLegacyFullEditActions>) {
    for (const spy of spies) {
      expect(spy).not.toHaveBeenCalled();
    }
  }

  it('startFullEditSession sets editor value and calls startDocumentJobForGraph', async () => {
    const options = createOptions();
    const controller = createEditorFullEditController(options as any);

    const revision = await controller.startFullEditSession({
      language: 'json' as any,
      text: '{"x":1}',
      reason: 'whole-document-replacement',
    });

    expect(revision).toBe(5);
    expect(options.setEditorValue).toHaveBeenCalledWith('{"x":1}');
    expect(mockStartDocumentJobForGraph).toHaveBeenCalledWith(
      expect.objectContaining({
        documentKey: 'doc-test',
        language: 'json',
        text: '{"x":1}',
      }),
    );
  });

  it('tab reactivation rebuilds with the correct reason', async () => {
    const options = createOptions();
    const controller = createEditorFullEditController(options as any);

    const revision = await controller.startFullEditSession({
      language: 'json' as any,
      text: 'foo: bar\n',
      reason: 'tab-reactivate',
    });

    expect(revision).toBe(5);
    expect(options.setEditorValue).toHaveBeenCalledWith('foo: bar\n');
    expect(editorStore.get().fullEditUiState.reason).toBe('tab-reactivate');
  });

  it('whole-document replacement records the correct reason', async () => {
    const options = createOptions();
    const controller = createEditorFullEditController(options as any);

    const revision = await controller.startFullEditSession({
      language: 'json' as any,
      text: '{}',
      reason: 'whole-document-replacement',
    });

    expect(revision).toBe(5);
    expect(mockStartDocumentJobForGraph).toHaveBeenCalledWith(
      expect.objectContaining({ language: 'json' }),

    );
  });

  it('startFullEditSession returns 0 when model is missing', async () => {
    const options = createOptions({ getModel: () => null });
    const controller = createEditorFullEditController(options as any);

    const revision = await controller.startFullEditSession({
      language: 'json' as any,
      text: '{}',
      reason: 'initial-example',
    });

    expect(revision).toBe(0);
    expect(mockStartDocumentJobForGraph).not.toHaveBeenCalled();
  });

  it('calls applyGraphAnalysis when graph analysis result is available', async () => {
    mockStartDocumentJobForGraph.mockResolvedValueOnce({
      snapshotId: 10,
      analysis: { documentKey: 'doc-test', language: 'json', tree: {}, value: {} },
    });

    const options = createOptions();
    const controller = createEditorFullEditController(options as any);

    await controller.startFullEditSession({
      language: 'json' as any,
      text: '{"a":1}',
      reason: 'whole-document-replacement',
    });

    await vi.waitFor(() => {
      expect(mockApplyGraphAnalysis).toHaveBeenCalled();
    });
  });
  it('applies authoritative source text after whole-document replacement intake by default', async () => {
    mockStartDocumentJobForGraph.mockResolvedValueOnce({
      snapshotId: 10,
      analysis: null,
      sourceText: '{\n  "a": 1\n}',
    });

    const options = createOptions();
    const controller = createEditorFullEditController(options as any);

    await controller.startFullEditSession({
      language: 'json' as any,
      text: '"{\\"a\\":1}"',
      reason: 'whole-document-replacement',
    });

    await vi.waitFor(() => {
      expect(options.setEditorValue).toHaveBeenLastCalledWith('{\n  "a": 1\n}');
    });
  });
  it('preserves submitted source text when whole-document replacement opts out of intake writeback', async () => {
    mockStartDocumentJobForGraph.mockResolvedValueOnce({
      snapshotId: 10,
      analysis: null,
      sourceText: '{\n  "a": 1\n}',
    });

    const options = createOptions();
    const controller = createEditorFullEditController(options as any);

    await controller.startFullEditSession({
      language: 'json' as any,
      text: '"{\\"a\\":1}"',
      reason: 'whole-document-replacement',
      sourceWritebackPolicy: 'submitted',
    });

    await vi.waitFor(() => {
      expect(options.setEditorValue).toHaveBeenCalledTimes(1);
    });
    expect(options.setEditorValue).toHaveBeenLastCalledWith('"{\\"a\\":1}"');
  });

  it('publishes sidecar full-edit state through the injected sink only', async () => {
    const sinkEvents: Array<{ kind: string; payload: unknown }> = [];
    const legacyFullEditActionSpies = spyOnLegacyFullEditActions();
    const sink: FullEditSink = {
      getState: createIdleFullEditUiState,
      begin: (payload) => sinkEvents.push({ kind: 'begin', payload }),
      appendChunkMeta: (payload) => sinkEvents.push({ kind: 'appendChunkMeta', payload }),
      markFinalizing: (payload) => sinkEvents.push({ kind: 'markFinalizing', payload }),
      finish: (payload) => sinkEvents.push({ kind: 'finish', payload }),
      cancel: (payload) => sinkEvents.push({ kind: 'cancel', payload }),
      bindSnapshot: (payload) => sinkEvents.push({ kind: 'bindSnapshot', payload }),
    };
    mockStartDocumentJobForGraph.mockResolvedValueOnce({
      snapshotId: 42,
      analysis: null,
      sourceText: '{\n  "right": true\n}',
    });
    const options = createOptions({
      fullEditSink: sink,
      rotateActiveDocumentKey: () => 'sidecar:tab-sidecar:0',
      commitEditorState: vi.fn(() => 8),
    });
    const controller = createEditorFullEditController(options as any);

    await controller.startFullEditSession({
      language: 'json' as any,
      text: '{"right":true}',
      reason: 'whole-document-replacement',
    });

    await vi.waitFor(() => {
      expect(sinkEvents.some((event) => event.kind === 'finish')).toBe(true);
    });
    expect(sinkEvents.map((event) => event.kind)).toContain('begin');
    expect(sinkEvents.map((event) => event.kind)).toContain('appendChunkMeta');
    expect(sinkEvents.map((event) => event.kind)).toContain('markFinalizing');
    expect(sinkEvents).toContainEqual({
      kind: 'bindSnapshot',
      payload: {
        documentKey: 'sidecar:tab-sidecar:0',
        revision: 8,
        snapshotId: 42,
      },
    });
    expect(editorStore.get().fullEditUiState).toMatchObject({
      active: false,
      sessionId: null,
      revision: 0,
    });
    expectLegacyFullEditActionsNotCalled(legacyFullEditActionSpies);
    expect(getActiveDocumentSnapshotId('sidecar:tab-sidecar:0')).toBeNull();
  });
  it('updates source text progressively and finishes with the full streamed import text', async () => {
    const options = createOptions();
    const controller = createEditorFullEditController(options as any);
    const file = createReadableFile(['{"a":', '1}']);

    await controller.importStream(file as any, 'json' as any, 'import-file');

    expect(options.setSourceText).toHaveBeenNthCalledWith(1, '');
    const sourceTextCalls = (options.setSourceText as ReturnType<typeof vi.fn>).mock.calls.map(([value]) => value);
    expect(sourceTextCalls).toContain('{"a":');
    expect(sourceTextCalls.at(-1)).toBe('{"a":1}');
  });

  it('applies graph analysis after a streamed file import completes', async () => {
    mockStartReadableDocumentJobSessionForGraph.mockImplementationOnce((input: any) => ({
      sessionId: input.sessionId,
      documentKey: input.documentKey,
      language: input.language,
      revision: input.revision,
      totalBytes: input.totalBytes ?? 0,
      chunkSize: input.chunkSize,
      streamRunId: input.sessionId,
      jobHandle: 2,
      result: Promise.resolve({
        snapshotId: 11,
        analysis: { documentKey: 'doc-test', language: 'json', tree: {}, value: { a: 1 } },
        batch: { requestSeq: 1, events: [], terminal: null },
        jobHandle: 2,
      }),
      batches: async function* () {},
      cancel: vi.fn(async () => {}),
    }));

    const options = createOptions();
    const controller = createEditorFullEditController(options as any);
    const file = createReadableFile(['{"a":', '1}']);

    await controller.importStream(file as any, 'json' as any, 'import-file');

    await vi.waitFor(() => {
      expect(mockApplyGraphAnalysis).toHaveBeenCalledWith(
        expect.anything(),
        'json',
        'doc-test',
        5,
        expect.objectContaining({ value: { a: 1 } }),
      );
    });
  });

  it('keeps graph analysis enabled for large streamed json imports so semantic tokens can be applied', async () => {
    const analysis = {
      documentKey: 'doc-test',
      language: 'json',
      tree: {},
      value: { a: 1 },
      semanticTokens: new ArrayBuffer(8),
    };
    mockStartReadableDocumentJobSessionForGraph.mockImplementationOnce((input: unknown) => {
      const request = input as {
        sessionId: string;
        documentKey: string;
        language: string;
        revision: number;
        totalBytes?: number;
        chunkSize?: number;
        outputAnalysis?: boolean;
      };
      return {
        sessionId: request.sessionId,
        documentKey: request.documentKey,
        language: request.language,
        revision: request.revision,
        totalBytes: request.totalBytes ?? 0,
        chunkSize: request.chunkSize,
        streamRunId: request.sessionId,
        jobHandle: 2,
        result: Promise.resolve({
          snapshotId: 11,
          analysis: request.outputAnalysis === false ? null : analysis,
          batch: { requestSeq: 1, events: [], terminal: null },
          jobHandle: 2,
        }),
        batches: async function* () {},
        cancel: vi.fn(async () => {}),
      };
    });

    const options = createOptions();
    const controller = createEditorFullEditController(
      options as unknown as Parameters<typeof createEditorFullEditController>[0],
    );
    const file = createReadableFile(['{"a":1}'], '5MB-min.json', 5 * 1024 * 1024);

    await controller.importStream(file as unknown as File, 'json', 'drop-file');

    await vi.waitFor(() => {
      expect(mockApplyGraphAnalysis).toHaveBeenCalledWith(
        expect.anything(),
        'json',
        'doc-test',
        5,
        expect.objectContaining({ semanticTokens: analysis.semanticTokens }),
      );
    });
  });

  it('imports dropped files as drop-file full-edit sessions', async () => {
    const options = createOptions();
    const controller = createEditorFullEditController(options as any);

    const file = createReadableFile(['{"a":', '1}']);
    await controller.importStream(file as any, 'json' as any, 'drop-file');

    expect(mockStartReadableDocumentJobSessionForGraph).toHaveBeenCalledWith(
      expect.objectContaining({ language: 'json', documentKey: 'doc-test', sessionId: 'doc-test:5', revision: 5 }),
    );
  });

  it('converts CSV import to the active editor language', async () => {
    const options = createOptions();
    const controller = createEditorFullEditController(options as any);
    const csvText = readFileSync(new URL('../../../../../../test/fixtures/csv/region_and_currency.csv', import.meta.url), 'utf8');
    const file = {
      name: 'region_and_currency.csv',
      size: new TextEncoder().encode(csvText).byteLength,
      text: vi.fn(async () => csvText),
    };
    await controller.importStream(file as any, 'csv' as any, 'import-file');
    expect(file.text).toHaveBeenCalledTimes(1);
    expect(options.callWasmWorker).toHaveBeenCalledWith('convert', {
      sourceLanguage: 'csv',
      targetFormat: 'json',
      text: csvText,
      options: undefined,
    });
    expect(options.applyImportLanguage).toHaveBeenCalledWith('json');
  });

  it('handles graph rebuild errors from startDocumentJobForGraph', async () => {
    mockStartDocumentJobForGraph.mockRejectedValueOnce(new Error('graph rebuild failed'));

    const options = createOptions();
    const controller = createEditorFullEditController(options as any);

    const revision = await controller.startFullEditSession({
      language: 'json' as any,
      text: '{"a":1}',
      reason: 'whole-document-replacement',
    });

    expect(revision).toBe(5);
    await vi.waitFor(() => {
      expect(options.updateActiveTempModel).toHaveBeenCalled();
    });
  });
});
