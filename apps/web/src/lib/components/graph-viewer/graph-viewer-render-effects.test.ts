import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createGraphViewerRenderEffects } from './graph-viewer-render-effects';
import type { JsonBlockSelection } from '../../store/editor-store';

const originalRequestAnimationFrame = globalThis.requestAnimationFrame;
const originalCancelAnimationFrame = globalThis.cancelAnimationFrame;

function createSelection(): JsonBlockSelection {
  return {
    sourceDocumentKey: 'doc-json',
    blockDocumentKey: 'doc-json:json-block:3:8:19',
    revision: 3,
    language: 'json',
    text: '{"b":[2,3]}',
    startByte: 8,
    endByte: 19,
    startLineNumber: 2,
    startColumn: 1,
    endLineNumber: 2,
    endColumn: 12,
  };
}

function createDeps() {
  return {
    shouldAttachGraphViewerTestHooks: () => false,
    getGraphStreamState: () => null,
    replaceGraphStreamState: vi.fn(),
    renderDocumentGraph: vi.fn(async () => ({})),
    attachFullEditDocumentJobSession: vi.fn(async () => ({})),
    renderJsonBlockSelection: vi.fn(async () => ({ nodes: [], edges: [] })),
    markGraphRequested: vi.fn(),
    resetStreamProgress: vi.fn(),
    onStreamingRenderError: vi.fn(),
  };
}

describe('graph-viewer render effects JSON block scheduling', () => {
  beforeEach(() => {
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

  it('renders active JSON block selection and blocks whole-document incremental render', () => {
    const deps = createDeps();
    const effects = createGraphViewerRenderEffects(deps);
    const selection = createSelection();

    effects.maybeRenderJsonBlock(selection, true);
    effects.maybeRenderIncremental({
      hasRenderRuntime: true,
      isBlocked: true,
      documentKey: 'doc-json',
      language: 'json',
      sourceText: '{"whole":true}',
      editorRevision: 3,
      graphAppliedRevision: 2,
    });

    expect(deps.renderJsonBlockSelection).toHaveBeenCalledWith(selection);
    expect(deps.markGraphRequested).toHaveBeenCalledWith({
      documentKey: selection.blockDocumentKey,
      revision: selection.revision,
      mode: 'json-block',
    });
    expect(deps.renderDocumentGraph).not.toHaveBeenCalled();

  });

  it('allows whole-document incremental render after JSON block selection clears', () => {
    const deps = createDeps();
    const effects = createGraphViewerRenderEffects(deps);

    effects.maybeRenderJsonBlock(createSelection(), true);
    effects.maybeRenderJsonBlock(null, true);
    effects.maybeRenderIncremental({
      hasRenderRuntime: true,
      isBlocked: false,
      documentKey: 'doc-json',
      language: 'json',
      sourceText: 'not-json',
      editorRevision: 4,
      graphAppliedRevision: 3,
    });

    expect(deps.renderDocumentGraph).toHaveBeenCalledWith({

      kind: 'incremental',
      documentKey: 'doc-json',
      language: 'json',
      text: 'not-json',
      revision: 4,
    });
    expect(deps.markGraphRequested).toHaveBeenCalledWith({
      documentKey: 'doc-json',
      revision: 4,
      mode: 'committed',
    });
  });

  it('resets transient graph progress when JSON block selection clears', () => {
    const deps = createDeps();
    const effects = createGraphViewerRenderEffects(deps);

    effects.maybeRenderJsonBlock(createSelection(), true);
    effects.maybeRenderJsonBlock(null, true);

    expect(deps.resetStreamProgress).toHaveBeenCalledTimes(1);
  });

  it('attaches a full-edit session even when it has already reached finalizing', () => {
    const deps = createDeps();
    const effects = createGraphViewerRenderEffects(deps);

    const ownership = effects.maybeAttachFullEditSession(
      {
        active: true,
        sessionId: 'session-1',
        ownerKey: 'owner-1',
        documentKey: 'doc-1',
        revision: 2,
        streamSeq: 1,
        inputByteLength: 12,
        modelVersionId: 3,
        byteLength: 12,
        language: 'json',
        phase: 'finalizing',
        sessionKind: 'full-edit',
        transportKind: 'memory',
        reason: 'whole-document-replacement',
      },
      {
        hasRenderRuntime: true,
        documentKey: 'doc-1',
        language: 'json',
        sourceText: '{"hello":"world"}',
      },
    );

    expect(deps.renderDocumentGraph).toHaveBeenCalledWith({
      kind: 'full-edit',
      documentKey: 'doc-1',
      language: 'json',
      text: '{"hello":"world"}',
      revision: 2,
    });
    expect(ownership).toEqual({ kind: 'started', documentKey: 'doc-1', revision: 2 });
  });

  it('does not claim Full Edit graph ownership before the render runtime is ready', () => {
    const deps = createDeps();
    const effects = createGraphViewerRenderEffects(deps);

    const ownership = effects.maybeAttachFullEditSession(
      {
        active: true,
        sessionId: 'session-1',
        ownerKey: 'owner-1',
        documentKey: 'doc-1',
        revision: 2,
        streamSeq: 1,
        inputByteLength: 12,
        modelVersionId: 3,
        byteLength: 12,
        language: 'json',
        phase: 'finalizing',
        sessionKind: 'full-edit',
        transportKind: 'memory',
        reason: 'initial-example',
      },
      {
        hasRenderRuntime: false,
        documentKey: 'doc-1',
        language: 'json',
        sourceText: '{"hello":"world"}',
      },
    );

    expect(ownership).toEqual({ kind: 'not-started' });
    expect(deps.renderDocumentGraph).not.toHaveBeenCalled();
  });

  it('does not reattach full-edit render when only session transport identity changes', () => {
    const deps = createDeps();
    const effects = createGraphViewerRenderEffects(deps);

    effects.maybeAttachFullEditSession(
      {
        active: true,
        sessionId: 'session-1',
        ownerKey: 'owner-1',
        documentKey: 'doc-1',
        revision: 2,
        streamSeq: 1,
        inputByteLength: 12,
        modelVersionId: 3,
        byteLength: 12,
        language: 'json',
        phase: 'streaming',
        sessionKind: 'full-edit',
        transportKind: 'memory',
        reason: 'whole-document-replacement',
      },
      {
        hasRenderRuntime: true,
        documentKey: 'doc-1',
        language: 'json',
        sourceText: '{"hello":"world"}',
      },
    );
    effects.maybeAttachFullEditSession(
      {
        active: true,
        sessionId: 'session-2',
        ownerKey: 'owner-2',
        documentKey: 'doc-1',
        revision: 2,
        streamSeq: 2,
        inputByteLength: 12,
        modelVersionId: 3,
        byteLength: 12,
        language: 'json',
        phase: 'finalizing',
        sessionKind: 'full-edit',
        transportKind: 'memory',
        reason: 'whole-document-replacement',
      },
      {
        hasRenderRuntime: true,
        documentKey: 'doc-1',
        language: 'json',
        sourceText: '{"hello":"world"}',
      },
    );

    expect(deps.renderDocumentGraph).toHaveBeenCalledTimes(1);
  });

  it('reattaches a full-edit render when the source text arrives after session creation', () => {
    const deps = createDeps();
    const effects = createGraphViewerRenderEffects(deps);
    const fullEditUiState = {
      active: true,
      sessionId: 'session-1',
      ownerKey: 'owner-1',
      documentKey: 'doc-1',
      revision: 2,
      streamSeq: 1,
      inputByteLength: 0,
      modelVersionId: 3,
      byteLength: 0,
      language: 'json' as const,
      phase: 'streaming' as const,
      sessionKind: 'full-edit' as const,
      transportKind: 'memory' as const,
      reason: 'initial-example' as const,
    };

    effects.maybeAttachFullEditSession(fullEditUiState, {
      hasRenderRuntime: true,
      documentKey: 'doc-1',
      language: 'json',
      sourceText: '',
    });
    const ownership = effects.maybeAttachFullEditSession(fullEditUiState, {
      hasRenderRuntime: true,
      documentKey: 'doc-1',
      language: 'json',
      sourceText: '{"object":{},"table_without_header":["a","b","c"]}',
    });

    expect(deps.renderDocumentGraph).toHaveBeenCalledTimes(2);
    expect(deps.renderDocumentGraph).toHaveBeenLastCalledWith({
      kind: 'full-edit',
      documentKey: 'doc-1',
      language: 'json',
      text: '{"object":{},"table_without_header":["a","b","c"]}',
      revision: 2,
    });
    expect(ownership).toEqual({ kind: 'started', documentKey: 'doc-1', revision: 2 });
  });

  it('attaches file-import streaming to the external document job session without starting a text job', () => {
    const deps = createDeps();
    const effects = createGraphViewerRenderEffects(deps);
    const fullEditUiState = {
      active: true,
      sessionId: 'session-file',
      ownerKey: 'owner-file',
      documentKey: 'doc-file',
      revision: 5,
      streamSeq: 0,
      inputByteLength: 0,
      modelVersionId: 7,
      byteLength: 0,
      language: 'json' as const,
      phase: 'streaming' as const,
      sessionKind: 'full-edit' as const,
      transportKind: 'file' as const,
      reason: 'drop-file' as const,
    };

    const ownership = effects.maybeAttachFullEditSession(fullEditUiState, {
      hasRenderRuntime: true,
      documentKey: 'doc-file',
      language: 'json',
      sourceText: '{"a":1',
    });
    effects.maybeAttachFullEditSession(fullEditUiState, {
      hasRenderRuntime: true,
      documentKey: 'doc-file',
      language: 'json',
      sourceText: '{"a":1,"b":2',
    });
    effects.maybeAttachFullEditSession(
      { ...fullEditUiState, phase: 'finalizing' as const },
      {
        hasRenderRuntime: true,
        documentKey: 'doc-file',
        language: 'json',
        sourceText: '{"a":1,"b":2}',
      },
    );

    expect(deps.attachFullEditDocumentJobSession).toHaveBeenCalledTimes(1);
    expect(deps.attachFullEditDocumentJobSession).toHaveBeenCalledWith({
      sessionId: 'session-file',
      documentKey: 'doc-file',
      language: 'json',
      revision: 5,
    });
    expect(deps.renderDocumentGraph).not.toHaveBeenCalled();
    expect(ownership).toEqual({ kind: 'started', documentKey: 'doc-file', revision: 5 });
  });

  it('does not start an incremental text job when source text catches up after external file graph final', () => {
    const deps = createDeps();
    const effects = createGraphViewerRenderEffects(deps);
    const fullEditUiState = {
      active: true,
      sessionId: 'session-file',
      ownerKey: 'owner-file',
      documentKey: 'doc-file',
      revision: 5,
      streamSeq: 0,
      inputByteLength: 0,
      modelVersionId: 7,
      byteLength: 0,
      language: 'json' as const,
      phase: 'streaming' as const,
      sessionKind: 'full-edit' as const,
      transportKind: 'file' as const,
      reason: 'drop-file' as const,
    };

    effects.maybeAttachFullEditSession(fullEditUiState, {
      hasRenderRuntime: true,
      documentKey: 'doc-file',
      language: 'json',
      sourceText: '{"a":',
    });
    effects.markRendered('doc-file', 5, '{"a":', 'json');
    effects.maybeRenderIncremental({
      hasRenderRuntime: true,
      isBlocked: false,
      documentKey: 'doc-file',
      language: 'json',
      sourceText: '{"a":1}',
      editorRevision: 5,
      graphAppliedRevision: 5,
    });

    expect(deps.attachFullEditDocumentJobSession).toHaveBeenCalledTimes(1);
    expect(deps.renderDocumentGraph).not.toHaveBeenCalled();
  });

});
