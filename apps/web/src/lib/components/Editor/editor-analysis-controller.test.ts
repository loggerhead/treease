import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createEditorAnalysisController } from './editor-analysis-controller';
import type { JsonBlockSelection } from '../../store/editor-store';

const mocked = vi.hoisted(() => ({
  resolveDocumentAnalysis: vi.fn(),
  resolveTreePathSafe: vi.fn(async () => []),
  resolveEditorPositionTarget: vi.fn(async () => null),
  callSharedWasmWorker: vi.fn(),
  analyzeDocumentAndStore: vi.fn(),
  readStoredDiagnosticsResult: vi.fn(),
}));

vi.mock('../../services/DocumentAnalysisResolver', () => ({
  resolveDocumentAnalysis: mocked.resolveDocumentAnalysis,
}));

vi.mock('../../services/TreePathService', () => ({
  resolveTreePathSafe: mocked.resolveTreePathSafe,
  toByteColumn: (text: string, column: number) => new TextEncoder().encode(text.slice(0, column)).byteLength,
}));

vi.mock('../../wasm/wasm-worker-singleton', () => ({
  callSharedWasmWorker: mocked.callSharedWasmWorker,
}));

vi.mock('./editor-position-target', () => ({
  resolveEditorPositionTarget: mocked.resolveEditorPositionTarget,
}));

vi.mock('../../services/EditorDiagnostics', () => ({
  analyzeDocumentAndStore: mocked.analyzeDocumentAndStore,
  readStoredDiagnosticsResult: mocked.readStoredDiagnosticsResult,
}));
vi.mock('../../services/DocumentSessionService', () => ({
  bindActiveDocumentSnapshotIfPresent: vi.fn(),
  getActiveDocumentSnapshotId: vi.fn(() => 7),
}));

type TestControllerOptions = {
  language?: 'json' | 'yaml';
  text?: string;
  documentKey?: string;
  revision?: number;
  selection?: JsonBlockSelection | null;
};

function createModel(text: string) {
  const lines = text.split('\n');
  return {
    getValue: () => text,
    getLineContent: (lineNumber: number) => lines[lineNumber - 1] ?? '',
    getVersionId: () => 1,
  };
}

function createController(options: TestControllerOptions = {}) {
  let language = options.language ?? 'json';
  let selection = options.selection ?? null;
  const text = options.text ?? 'prefix\n{"a":1}\nsuffix';
  const model = createModel(text);
  const setJsonBlockSelection = vi.fn((next: JsonBlockSelection | null) => {
    selection = next;
  });
  const primeSemanticTokensForDocument = vi.fn();
  const clearSemanticTokensForDocument = vi.fn();
  const refreshSemanticTokensForLanguage = vi.fn();
  const setTreeState = vi.fn();
  const applyRootScalarHighlight = vi.fn();
  const updateActiveTempModel = vi.fn((updater: (current: any) => any) => {
    updater({});
  });
  const controller = createEditorAnalysisController({
    getMonaco: () => undefined,
    getEditor: () => ({ getPosition: () => ({ lineNumber: 2, column: 2 }) }) as any,
    getModel: () => model as any,
    getDocumentKey: () => options.documentKey ?? 'doc-json',
    getLanguageId: () => language,
    getNestEnabled: () => false,
    getEditorRevision: () => options.revision ?? 3,
    isImportActive: () => false,
    getSourceText: () => text,
    getJsonBlockSelection: () => selection,
    setJsonBlockSelection,
    updateActiveTempModel,
    setTreeState,
    applyRootScalarHighlight,
    primeSemanticTokensForDocument,
    clearSemanticTokensForDocument,
    refreshSemanticTokensForLanguage,
  });
  return {
    controller,
    model,
    setLanguage: (next: 'json' | 'yaml') => {
      language = next;
    },
    applyRootScalarHighlight,
    getSelection: () => selection,
    setJsonBlockSelection,
    setTreeState,
    primeSemanticTokensForDocument,
    clearSemanticTokensForDocument,
    refreshSemanticTokensForLanguage,
    updateActiveTempModel,
  };
}

function resolvedAnalysis(overrides: Record<string, unknown> = {}) {
  return {
    status: 'resolved',
    analysis: {
      tree: null,
      value: null,
      diagnostics: [{ message: 'parse failed' }],
      semanticTokens: new ArrayBuffer(0),
      ...overrides,
    },
  };
}

describe('editor analysis controller json block selection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.resolveTreePathSafe.mockResolvedValue([]);
    mocked.resolveDocumentAnalysis.mockResolvedValue(resolvedAnalysis());
    mocked.callSharedWasmWorker.mockResolvedValue({
      found: false,
      text: '',
      startByte: 0,
      endByte: 0,
      startLineNumber: 1,
      startColumn: 1,
      endLineNumber: 1,
      endColumn: 1,
    });
  });

  it('clears JSON block selection for valid whole-document JSON', async () => {
    const existingSelection = {
      sourceDocumentKey: 'doc-json',
      blockDocumentKey: 'doc-json:json-block:2:0:7',
      revision: 2,
      language: 'json',
      text: '{"a":1}',
      startByte: 0,
      endByte: 7,
      startLineNumber: 1,
      startColumn: 1,
      endLineNumber: 1,
      endColumn: 8,
    } satisfies JsonBlockSelection;
    const { controller, model, setJsonBlockSelection, clearSemanticTokensForDocument } = createController({
      selection: existingSelection,
    });
    mocked.resolveDocumentAnalysis.mockResolvedValue(
      resolvedAnalysis({ tree: { kind: 1 }, diagnostics: [], value: { a: 1 } }),
    );

    await controller.updateTreePath({ lineNumber: 1, column: 2 }, { syncGraphHighlight: true });

    expect(mocked.callSharedWasmWorker).not.toHaveBeenCalled();
    expect(setJsonBlockSelection).toHaveBeenCalledWith(null);
    expect(clearSemanticTokensForDocument).not.toHaveBeenCalled();
    expect(model.getValue()).toBe('prefix\n{"a":1}\nsuffix');
  });

  it('reuses the latest authoritative analysis for valid whole-document JSON cursor sync', async () => {
    const semanticTokens = new ArrayBuffer(0);
    const analysis = {
      tree: { kind: 1 },
      value: { a: 1 },
      diagnostics: [],
      semanticTokens,
    };
    const { controller, model, getSelection } = createController({ text: '{"a":1}' });
    mocked.analyzeDocumentAndStore.mockResolvedValue(analysis);
    mocked.resolveDocumentAnalysis.mockResolvedValueOnce(resolvedAnalysis(analysis));

    await controller.syncAuthoritativeAnalysis(model as any, 'json', 'doc-json', false);

    mocked.resolveDocumentAnalysis.mockResolvedValue({ status: 'unknown', analysis: null });
    await controller.updateTreePath({ lineNumber: 1, column: 2 }, { syncGraphHighlight: true });

    expect(mocked.callSharedWasmWorker).not.toHaveBeenCalled();
    expect(getSelection()).toBeNull();
  });

  it('routes empty authoritative sources through analysis instead of short-circuiting them', async () => {
    const analysis = {
      tree: null,
      value: null,
      diagnostics: [{ message: 'parse failed' }],
      semanticTokens: new ArrayBuffer(0),
      snapshotId: null,
    };
    const { controller, model, setTreeState } = createController({ text: '' });
    mocked.analyzeDocumentAndStore.mockResolvedValue(analysis);
    mocked.resolveDocumentAnalysis.mockResolvedValue(resolvedAnalysis(analysis));

    await controller.syncAuthoritativeAnalysis(model as any, 'json', 'doc-json', false);

    expect(mocked.analyzeDocumentAndStore).toHaveBeenCalledWith(
      'json',
      '',
      'doc-json',
      false,
      expect.objectContaining({
        onAnalysisDelta: expect.any(Function),
      }),
    );
    expect(setTreeState).not.toHaveBeenCalled();
  });

  it('clears tree state only when authoritative analysis returns null', async () => {
    const { controller, model, setTreeState, updateActiveTempModel } = createController({ text: '' });
    mocked.analyzeDocumentAndStore.mockResolvedValue(null);

    await controller.syncAuthoritativeAnalysis(model as any, 'json', 'doc-json', false);

    expect(mocked.analyzeDocumentAndStore).toHaveBeenCalled();
    expect(setTreeState).toHaveBeenCalledWith({
      tree: null,
      value: null,
      source: 'editor',
      revision: 3,
    });
    expect(updateActiveTempModel).toHaveBeenCalled();
    const currentTempModel: any = {
      treePath: [{ tag: 0, key: 'before', index: 0 }],
      graphHighlight: {
        path: [{ tag: 0, key: 'before', index: 0 }],
        target: 'value',
        revision: 2,
        source: 'search',
      },
    };
    const clearsGraphHighlight = updateActiveTempModel.mock.calls.some(([updater]) => {
      const nextTempModel = (updater as (current: any) => any)(currentTempModel);
      return nextTempModel.treePath.length === 0 && nextTempModel.graphHighlight === null;
    });
    expect(clearsGraphHighlight).toBe(true);
  });

  it('sets JSON block selection when invalid JSON cursor is inside a valid block', async () => {
    const { controller, getSelection, primeSemanticTokensForDocument, refreshSemanticTokensForLanguage } =
      createController();
    mocked.callSharedWasmWorker
      .mockResolvedValueOnce({
        found: true,
        text: '{"a":1}',
        startByte: 7,
        endByte: 14,
        startLineNumber: 2,
        startColumn: 1,
        endLineNumber: 2,
        endColumn: 8,
      })
    mocked.analyzeDocumentAndStore.mockResolvedValueOnce({
      semanticTokens: new Uint32Array([0, 1, 3, 0, 0]).buffer,
    });

    await controller.updateTreePath({ lineNumber: 2, column: 3 }, { syncGraphHighlight: true });

    expect(getSelection()).toEqual({
      sourceDocumentKey: 'doc-json',
      blockDocumentKey: 'doc-json:json-block:3:7:14',
      revision: 3,
      language: 'json',
      text: '{"a":1}',
      startByte: 7,
      endByte: 14,
      startLineNumber: 2,
      startColumn: 1,
      endLineNumber: 2,
      endColumn: 8,
    });
    expect(mocked.analyzeDocumentAndStore).toHaveBeenCalledWith(
      'json',
      '{"a":1}',
      'doc-json:json-block:3:7:14',
      false,
    );
    expect(primeSemanticTokensForDocument).toHaveBeenCalledWith(
      'doc-json',
      new Uint32Array([1, 1, 3, 0, 0]).buffer,
    );
    expect(refreshSemanticTokensForLanguage).toHaveBeenCalledWith('json');
  });

  it('clears JSON block selection when the cursor moves outside blocks', async () => {
    const existingSelection = {
      sourceDocumentKey: 'doc-json',
      blockDocumentKey: 'doc-json:json-block:3:7:14',
      revision: 3,
      language: 'json',
      text: '{"a":1}',
      startByte: 7,
      endByte: 14,
      startLineNumber: 2,
      startColumn: 1,
      endLineNumber: 2,
      endColumn: 8,
    } satisfies JsonBlockSelection;
    const { controller, setJsonBlockSelection, clearSemanticTokensForDocument, refreshSemanticTokensForLanguage } =
      createController({ selection: existingSelection });

    await controller.updateTreePath({ lineNumber: 1, column: 2 }, { syncGraphHighlight: true });

    expect(setJsonBlockSelection).toHaveBeenCalledWith(null);
    expect(clearSemanticTokensForDocument).toHaveBeenCalledWith('doc-json');
    expect(refreshSemanticTokensForLanguage).toHaveBeenCalledWith('json');
  });

  it('clears JSON block selection for non-json languages', async () => {
    const existingSelection = {
      sourceDocumentKey: 'doc-json',
      blockDocumentKey: 'doc-json:json-block:3:7:14',
      revision: 3,
      language: 'json',
      text: '{"a":1}',
      startByte: 7,
      endByte: 14,
      startLineNumber: 2,
      startColumn: 1,
      endLineNumber: 2,
      endColumn: 8,
    } satisfies JsonBlockSelection;
    const { controller, setLanguage, setJsonBlockSelection } = createController({ selection: existingSelection });
    setLanguage('yaml');

    await controller.updateTreePath({ lineNumber: 2, column: 3 }, { syncGraphHighlight: true });

    expect(setJsonBlockSelection).toHaveBeenCalledWith(null);
  });

  it('does not prime whole-document semantic tokens for invalid JSON', async () => {
    const semanticTokens = new Uint32Array([0, 1, 3, 0, 0]).buffer;
    const { controller, model, primeSemanticTokensForDocument, clearSemanticTokensForDocument } = createController();
    const analysis = {
      tree: null,
      value: null,
      diagnostics: [{ message: 'parse failed' }],
      semanticTokens,
    };
    mocked.analyzeDocumentAndStore.mockResolvedValue(analysis);
    mocked.resolveDocumentAnalysis.mockResolvedValue(resolvedAnalysis(analysis));

    await controller.syncAuthoritativeAnalysis(model as any, 'json', 'doc-json', false);

    expect(primeSemanticTokensForDocument).not.toHaveBeenCalledWith('doc-json', semanticTokens);
    expect(clearSemanticTokensForDocument).toHaveBeenCalledWith('doc-json');
  });

  it('keeps priming whole-document semantic tokens for valid JSON', async () => {
    const semanticTokens = new Uint32Array([0, 1, 3, 0, 0]).buffer;
    const { controller, model, primeSemanticTokensForDocument, clearSemanticTokensForDocument } = createController();
    const analysis = {
      tree: { kind: 1 },
      value: { a: 1 },
      diagnostics: [],
      semanticTokens,
    };
    mocked.analyzeDocumentAndStore.mockResolvedValue(analysis);
    mocked.resolveDocumentAnalysis.mockResolvedValue(resolvedAnalysis(analysis));

    await controller.syncAuthoritativeAnalysis(model as any, 'json', 'doc-json', false);

    expect(primeSemanticTokensForDocument).toHaveBeenCalledWith('doc-json', semanticTokens);
    expect(clearSemanticTokensForDocument).not.toHaveBeenCalledWith('doc-json');
  });

  it('does not let stale async results overwrite the latest selection', async () => {
    let resolveFirstBlock: (value: unknown) => void = () => {};
    const firstBlock = new Promise((resolve) => {
      resolveFirstBlock = resolve;
    });
    const { controller, getSelection } = createController();
    mocked.callSharedWasmWorker
      .mockReturnValueOnce(firstBlock)
      .mockResolvedValueOnce({
        found: true,
        text: '{"b":2}',
        startByte: 15,
        endByte: 22,
        startLineNumber: 3,
        startColumn: 1,
        endLineNumber: 3,
        endColumn: 8,
      })
      .mockResolvedValueOnce({
        semanticTokens: new Uint32Array([0, 1, 3, 0, 0]).buffer,
      });

    const firstUpdate = controller.updateTreePath({ lineNumber: 2, column: 3 }, { syncGraphHighlight: true });
    await vi.waitFor(() => {
      expect(mocked.callSharedWasmWorker).toHaveBeenCalledTimes(1);
    });
    const secondUpdate = controller.updateTreePath({ lineNumber: 3, column: 3 }, { syncGraphHighlight: true });
    await secondUpdate;
    resolveFirstBlock({
      found: true,
      text: '{"a":1}',
      startByte: 7,
      endByte: 14,
      startLineNumber: 2,
      startColumn: 1,
      endLineNumber: 2,
      endColumn: 8,
    });
    await firstUpdate;

    expect(getSelection()?.blockDocumentKey).toBe('doc-json:json-block:3:15:22');
    expect(getSelection()?.text).toBe('{"b":2}');
  });
});
