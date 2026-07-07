import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createEditorAnalysisController } from './editor-analysis-controller';
import type { JsonBlockSelection } from '../../store/editor-store';
import { getWorkspaceSnapshotId } from '../../store/workspace-snapshot-bindings';
import {
  clearActiveDocumentSemanticState,
  markActiveDocumentSemanticInvalid,
  markActiveDocumentSemanticPending,
  markActiveDocumentSemanticValid,
} from '../../store/active-document-semantic-state';

type ControllerOptions = Parameters<typeof createEditorAnalysisController>[0];
type CursorPathRequestMarker = NonNullable<ControllerOptions['markCursorPathRequested']>;
type CursorPathSettledMarker = NonNullable<ControllerOptions['markCursorPathSettled']>;

const mocked = vi.hoisted(() => ({
  resolveDocumentAnalysis: vi.fn(),
  resolveTreePathResult: vi.fn(async () => ({ status: 'ready', data: [] })),
  resolveEditorPositionTargetResult: vi.fn(async () => ({ status: 'ready', data: null })),
  callSharedWasmWorker: vi.fn(),
  readStoredDiagnosticsResult: vi.fn(),
  runTextDocumentJobForGraph: vi.fn(),
  buildDocumentJobSettings: vi.fn((input: unknown) => input),
}));

vi.mock('../../services/DocumentAnalysisResolver', () => ({
  resolveDocumentAnalysis: mocked.resolveDocumentAnalysis,
}));

vi.mock('../../services/TreePathService', () => ({
  resolveTreePathResult: mocked.resolveTreePathResult,
  toByteColumn: (text: string, column: number) => new TextEncoder().encode(text.slice(0, column)).byteLength,
}));

vi.mock('../../wasm/wasm-worker-singleton', () => ({
  callSharedWasmWorker: mocked.callSharedWasmWorker,
}));

vi.mock('./editor-position-target', () => ({
  resolveEditorPositionTargetResult: mocked.resolveEditorPositionTargetResult,
}));

vi.mock('../../services/EditorDiagnostics', () => ({
  readStoredDiagnosticsResult: mocked.readStoredDiagnosticsResult,
}));
vi.mock('../../graph-stream/document-job-runner', () => ({
  runTextDocumentJobForGraph: mocked.runTextDocumentJobForGraph,
  buildDocumentJobSettings: mocked.buildDocumentJobSettings,
}));
vi.mock('../../store/workspace-snapshot-bindings', () => ({
  getWorkspaceSnapshotId: vi.fn(() => 7),
}));

type TestControllerOptions = {
  language?: 'json' | 'yaml';
  text?: string;
  documentKey?: string;
  revision?: number;
  selection?: JsonBlockSelection | null;
  markCursorPathRequested?: CursorPathRequestMarker;
  markCursorPathSettled?: CursorPathSettledMarker;
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
    markCursorPathRequested: options.markCursorPathRequested,
    markCursorPathSettled: options.markCursorPathSettled,
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
    clearActiveDocumentSemanticState();
    mocked.resolveTreePathResult.mockResolvedValue({ status: 'ready', data: [] });
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
    mocked.runTextDocumentJobForGraph.mockResolvedValue({
      status: 'snapshotReady',
      snapshotId: 9,
      analysis: {
        semanticTokens: new Uint32Array([0, 0, 6, 2, 0]).buffer,
      },
      sourceText: null,
      batch: { requestSeq: 1, events: [], terminal: null },
      jobHandle: 1,
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

  it('marks cursor path readiness around successful tree path resolution', async () => {
    const markCursorPathRequested = vi.fn<CursorPathRequestMarker>();
    const markCursorPathSettled = vi.fn<CursorPathSettledMarker>();
    const { controller } = createController({
      markCursorPathRequested,
      markCursorPathSettled,
    });
    mocked.resolveTreePathResult.mockResolvedValue({ status: 'ready', data: [{ tag: 1, key: 'a', index: 0 }] });

    await controller.updateTreePath({ lineNumber: 2, column: 3 }, { syncGraphHighlight: true });

    expect(markCursorPathRequested).toHaveBeenCalledWith({
      requestId: 1,
      documentKey: 'doc-json',
      revision: 3,
      lineNumber: 2,
      column: 3,
      syncGraphHighlight: true,
    });
    expect(markCursorPathSettled).toHaveBeenCalledWith({
      requestId: 1,
      documentKey: 'doc-json',
      revision: 3,
      lineNumber: 2,
      column: 3,
    });
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
    expect(primeSemanticTokensForDocument).toHaveBeenCalledWith('doc-json', expect.any(ArrayBuffer));
    expect(refreshSemanticTokensForLanguage).toHaveBeenCalledWith('json');
  });

  it('sets JSON block selection without graph sync when no snapshot is available', async () => {
    vi.mocked(getWorkspaceSnapshotId).mockReturnValueOnce(null).mockReturnValueOnce(null);
    const { controller, getSelection, primeSemanticTokensForDocument, refreshSemanticTokensForLanguage } = createController();
    mocked.resolveTreePathResult.mockResolvedValueOnce({ status: 'snapshotNotReady', data: [] });
    mocked.callSharedWasmWorker.mockResolvedValueOnce({
      found: true,
      text: '{"a":1}',
      startByte: 7,
      endByte: 14,
      startLineNumber: 2,
      startColumn: 1,
      endLineNumber: 2,
      endColumn: 8,
    });

    await controller.updateTreePath({ lineNumber: 2, column: 3 }, { syncGraphHighlight: false });

    expect(getSelection()?.blockDocumentKey).toBe('doc-json:json-block:3:7:14');
    expect(primeSemanticTokensForDocument).toHaveBeenCalledWith('doc-json', expect.any(ArrayBuffer));
    expect(refreshSemanticTokensForLanguage).toHaveBeenCalledWith('json');
    expect(mocked.resolveTreePathResult).toHaveBeenCalledWith(
      expect.anything(),
      { lineNumber: 2, column: 3 },
      'doc-json',
      'json',
      false,
      null,
    );
  });

  it('uses the current valid semantic snapshot instead of JSON block fallback', async () => {
    vi.mocked(getWorkspaceSnapshotId).mockReturnValueOnce(null);
    markActiveDocumentSemanticValid({
      documentKey: 'doc-json',
      language: 'json',
      revision: 3,
      snapshotId: 42 as any,
    });
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
    const { controller, setJsonBlockSelection } = createController({ selection: existingSelection });

    await controller.updateTreePath({ lineNumber: 2, column: 3 }, { syncGraphHighlight: false });

    expect(mocked.callSharedWasmWorker).not.toHaveBeenCalled();
    expect(setJsonBlockSelection).toHaveBeenCalledWith(null);
    expect(mocked.resolveTreePathResult).toHaveBeenCalledWith(
      expect.anything(),
      { lineNumber: 2, column: 3 },
      'doc-json',
      'json',
      false,
      42,
    );
  });

  it('does not run JSON block fallback while the current revision is pending', async () => {
    vi.mocked(getWorkspaceSnapshotId).mockReturnValueOnce(null);
    markActiveDocumentSemanticPending({
      documentKey: 'doc-json',
      language: 'json',
      revision: 3,
    });
    const { controller } = createController();

    await controller.updateTreePath({ lineNumber: 2, column: 3 }, { syncGraphHighlight: false });

    expect(mocked.callSharedWasmWorker).not.toHaveBeenCalled();
    expect(mocked.resolveTreePathResult).toHaveBeenCalledWith(
      expect.anything(),
      { lineNumber: 2, column: 3 },
      'doc-json',
      'json',
      false,
      null,
    );
  });

  it('does not downgrade a temporarily invalid whole-document JSON into JSON block fallback', async () => {
    vi.mocked(getWorkspaceSnapshotId).mockReturnValueOnce(null);
    markActiveDocumentSemanticValid({
      documentKey: 'doc-json',
      language: 'json',
      revision: 2,
      snapshotId: 41 as any,
    });
    markActiveDocumentSemanticInvalid({
      documentKey: 'doc-json',
      language: 'json',
      revision: 3,
      snapshotId: 42 as any,
    });
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
    const { controller, setJsonBlockSelection, clearSemanticTokensForDocument, refreshSemanticTokensForLanguage } =
      createController({ selection: existingSelection });

    await controller.updateTreePath({ lineNumber: 2, column: 3 }, { syncGraphHighlight: true });

    expect(mocked.callSharedWasmWorker).not.toHaveBeenCalled();
    expect(setJsonBlockSelection).toHaveBeenCalledWith(null);
    expect(clearSemanticTokensForDocument).toHaveBeenCalledWith('doc-json');
    expect(refreshSemanticTokensForLanguage).toHaveBeenCalledWith('json');
    expect(mocked.resolveTreePathResult).toHaveBeenCalledWith(
      expect.anything(),
      { lineNumber: 2, column: 3 },
      'doc-json',
      'json',
      false,
      null,
    );
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
