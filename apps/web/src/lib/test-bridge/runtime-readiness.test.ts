import { beforeEach, describe, expect, it } from 'vitest';
import type { FullEditUiState } from '../store/editor-store';
import {
  markCursorPathRequested,
  markCursorPathSettled,
  markGraphApplied,
  markGraphFlushed,
  markGraphRequested,
  markPreviewCompleted,
  markPreviewRequested,
  markSidecarRequested,
  markSidecarSettled,
  markSubgraphMaterialized,
  markSubgraphRequested,
  readRuntimeReadiness,
  resetRuntimeReadiness,
  syncGraphInteractionReadiness,
  syncRuntimeReadinessFromEditorState,
  syncSubgraphInteractionReadiness,
} from './runtime-readiness';

function idleFullEditUiState(overrides: Partial<FullEditUiState> = {}): FullEditUiState {
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
    ...overrides,
  };
}

describe('runtime-readiness', () => {
  beforeEach(() => {
    resetRuntimeReadiness();
    syncRuntimeReadinessFromEditorState({
      documentKey: 'doc-1',
      editorRevision: 1,
      fullEditUiState: idleFullEditUiState(),
    });
  });

  it('keeps graph readiness unchanged when a stale apply arrives', () => {
    markGraphRequested({ documentKey: 'doc-1', revision: 2, mode: 'committed' });
    syncRuntimeReadinessFromEditorState({
      documentKey: 'doc-1',
      editorRevision: 2,
      fullEditUiState: idleFullEditUiState(),
    });

    markGraphApplied({ documentKey: 'doc-1', revision: 1, mode: 'committed' });

    expect(readRuntimeReadiness().graph.appliedRevision).toBe(0);
  });

  it('gives each import session a distinct request id', () => {
    syncRuntimeReadinessFromEditorState({
      documentKey: 'doc-1',
      editorRevision: 1,
      fullEditUiState: idleFullEditUiState({ active: true, sessionId: 'import-1', revision: 2, phase: 'preparing' }),
    });
    const first = readRuntimeReadiness().import;
    syncRuntimeReadinessFromEditorState({
      documentKey: 'doc-1',
      editorRevision: 2,
      fullEditUiState: idleFullEditUiState({ active: true, sessionId: 'import-2', revision: 3, phase: 'preparing' }),
    });
    const second = readRuntimeReadiness().import;

    expect(second.requestId).toBeGreaterThan(first.requestId);
    expect(second.requestedRevision).toBe(3);
    expect(second.settled).toBe(false);
  });

  it('observes a preparing import before its stream session id exists', () => {
    syncRuntimeReadinessFromEditorState({
      documentKey: 'doc-1',
      editorRevision: 1,
      fullEditUiState: idleFullEditUiState({ active: true, sessionId: null, revision: 2, phase: 'preparing' }),
    });

    expect(readRuntimeReadiness().import).toMatchObject({
      requestId: 1,
      requestedRevision: 2,
      settled: false,
    });
  });

  it('keeps one request id when a preparing import later receives its session id', () => {
    syncRuntimeReadinessFromEditorState({
      documentKey: 'doc-1',
      editorRevision: 1,
      fullEditUiState: idleFullEditUiState({ active: true, sessionId: null, revision: 2, phase: 'preparing' }),
    });
    const preparingRequestId = readRuntimeReadiness().import.requestId;
    syncRuntimeReadinessFromEditorState({
      documentKey: 'doc-1',
      editorRevision: 1,
      fullEditUiState: idleFullEditUiState({ active: true, sessionId: 'import-1', revision: 2, phase: 'streaming' }),
    });

    expect(readRuntimeReadiness().import.requestId).toBe(preparingRequestId);
  });

  it('keeps an import request identity when intake creates a new document key', () => {
    syncRuntimeReadinessFromEditorState({
      documentKey: 'doc-1',
      editorRevision: 1,
      fullEditUiState: idleFullEditUiState({ active: true, revision: 2, phase: 'preparing' }),
    });
    syncRuntimeReadinessFromEditorState({
      documentKey: 'doc-2',
      editorRevision: 1,
      fullEditUiState: idleFullEditUiState({ active: true, revision: 2, phase: 'preparing' }),
    });

    expect(readRuntimeReadiness().import.requestId).toBe(1);
  });

  it('does not settle a file import until the source store has changed', () => {
    const importing = idleFullEditUiState({
      active: true,
      revision: 2,
      phase: 'preparing',
      reason: 'import-file',
    });
    syncRuntimeReadinessFromEditorState({
      documentKey: 'doc-1',
      editorRevision: 1,
      sourceText: '{"before":true}',
      fullEditUiState: importing,
    });
    syncRuntimeReadinessFromEditorState({
      documentKey: 'doc-1',
      editorRevision: 2,
      sourceText: '{"before":true}',
      fullEditUiState: idleFullEditUiState({ revision: 2, reason: 'import-file' }),
    });

    expect(readRuntimeReadiness().import).toMatchObject({
      sourceWriteObserved: false,
      settled: false,
    });

    syncRuntimeReadinessFromEditorState({
      documentKey: 'doc-1',
      editorRevision: 3,
      sourceText: '{"after":true}',
      fullEditUiState: idleFullEditUiState({ revision: 2, reason: 'import-file' }),
    });

    expect(readRuntimeReadiness().import).toMatchObject({
      sourceWriteObserved: true,
      settled: true,
    });
  });

  it('resets source-write evidence for each new file import', () => {
    syncRuntimeReadinessFromEditorState({
      documentKey: 'doc-1',
      editorRevision: 1,
      sourceText: 'before',
      fullEditUiState: idleFullEditUiState({ active: true, revision: 2, phase: 'preparing', reason: 'drop-file' }),
    });
    syncRuntimeReadinessFromEditorState({
      documentKey: 'doc-1',
      editorRevision: 2,
      sourceText: 'after-first',
      fullEditUiState: idleFullEditUiState({ revision: 2, reason: 'drop-file' }),
    });
    syncRuntimeReadinessFromEditorState({
      documentKey: 'doc-1',
      editorRevision: 2,
      sourceText: 'after-first',
      fullEditUiState: idleFullEditUiState({ active: true, revision: 3, phase: 'preparing', reason: 'drop-file' }),
    });

    expect(readRuntimeReadiness().import).toMatchObject({
      requestId: 2,
      sourceWriteObserved: false,
      settled: false,
    });
  });

  it('advances graph milestones monotonically through settled', () => {
    syncRuntimeReadinessFromEditorState({
      documentKey: 'doc-1',
      editorRevision: 3,
      fullEditUiState: idleFullEditUiState(),
    });

    markGraphRequested({ documentKey: 'doc-1', revision: 3, mode: 'committed' });
    markGraphApplied({ documentKey: 'doc-1', revision: 3, mode: 'committed' });
    markGraphFlushed({ documentKey: 'doc-1', revision: 3, mode: 'committed' });
    syncGraphInteractionReadiness({
      documentKey: 'doc-1',
      revision: 3,
      mode: 'committed',
      hasGraphData: true,
      nodeCount: 1,
      pendingRenderWork: false,
      interactiveReady: true,
    });

    expect(readRuntimeReadiness().graph).toMatchObject({
      requestedRevision: 3,
      appliedRevision: 3,
      flushedRevision: 3,
      interactiveRevision: 3,
      settledRevision: 3,
      settled: true,
    });
  });

  it('does not settle graph readiness before apply and flush catch up', () => {
    syncRuntimeReadinessFromEditorState({
      documentKey: 'doc-1',
      editorRevision: 3,
      fullEditUiState: idleFullEditUiState(),
    });

    markGraphRequested({ documentKey: 'doc-1', revision: 3, mode: 'committed' });
    syncGraphInteractionReadiness({
      documentKey: 'doc-1',
      revision: 3,
      mode: 'committed',
      hasGraphData: true,
      nodeCount: 1,
      pendingRenderWork: false,
      interactiveReady: true,
    });

    expect(readRuntimeReadiness().graph).toMatchObject({
      requestedRevision: 3,
      interactiveRevision: 3,
      appliedRevision: 0,
      flushedRevision: 0,
      settledRevision: 0,
      settled: false,
    });
  });

  it('resets revision-scoped readiness when document changes', () => {
    markGraphRequested({ documentKey: 'doc-1', revision: 2, mode: 'committed' });
    markPreviewRequested({ requestId: 1, sourceRevision: 2 });
    syncRuntimeReadinessFromEditorState({
      documentKey: 'doc-2',
      editorRevision: 1,
      fullEditUiState: idleFullEditUiState(),
    });

    expect(readRuntimeReadiness()).toMatchObject({
      documentKey: 'doc-2',
      editorRevision: 1,
      graph: expect.objectContaining({ requestedRevision: 0 }),
      preview: expect.objectContaining({ requestId: 0 }),
    });
  });

  it('ignores stale preview and subgraph completions after a newer request starts', () => {
    markPreviewRequested({ requestId: 1, sourceRevision: 2 });
    markPreviewRequested({ requestId: 2, sourceRevision: 3 });
    markPreviewCompleted({ requestId: 1, sourceRevision: 2, completedRevision: 2 });

    markSubgraphRequested({ requestId: 1, pathKey: 'k:old', sourceRevision: 2 });
    markSubgraphRequested({ requestId: 2, pathKey: 'k:new', sourceRevision: 3 });
    markSubgraphMaterialized({
      requestId: 1,
      pathKey: 'k:old',
      sourceRevision: 2,
      materializedRevision: 2,
    });
    syncSubgraphInteractionReadiness({
      requestId: 1,
      pathKey: 'k:old',
      sourceRevision: 2,
      interactiveRevision: 2,
      interactiveReady: true,
    });

    expect(readRuntimeReadiness()).toMatchObject({
      preview: expect.objectContaining({
        requestId: 2,
        completedRevision: 0,
        settled: false,
      }),
      subgraph: expect.objectContaining({
        requestId: 2,
        pathKey: 'k:new',
        materializedRevision: 0,
        interactiveRevision: 0,
        settled: false,
      }),
    });
  });

  it('ignores stale sidecar settlement after a newer request starts', () => {
    markSidecarRequested({ requestId: 1, hookId: 'right-editor', documentKey: 'sidecar:old' });
    markSidecarRequested({ requestId: 2, hookId: 'right-editor', documentKey: 'sidecar:new' });
    markSidecarSettled({
      requestId: 1,
      hookId: 'right-editor',
      documentKey: 'sidecar:old',
      revision: 1,
    });

    expect(readRuntimeReadiness().sidecar).toMatchObject({
      requestId: 2,
      hookId: 'right-editor',
      documentKey: 'sidecar:new',
      revision: 0,
      settled: false,
    });
  });

  it('tracks the latest cursor path request through settlement', () => {
    markCursorPathRequested({
      requestId: 1,
      documentKey: 'doc-1',
      revision: 1,
      lineNumber: 3,
      column: 5,
      syncGraphHighlight: true,
    });
    markCursorPathRequested({
      requestId: 2,
      documentKey: 'doc-1',
      revision: 1,
      lineNumber: 4,
      column: 9,
      syncGraphHighlight: true,
    });

    markCursorPathSettled({
      requestId: 1,
      documentKey: 'doc-1',
      revision: 1,
      lineNumber: 3,
      column: 5,
    });

    expect(readRuntimeReadiness().cursorPath).toMatchObject({
      requestId: 2,
      documentKey: 'doc-1',
      revision: 1,
      lineNumber: 4,
      column: 9,
      syncGraphHighlight: true,
      settled: false,
    });

    markCursorPathSettled({
      requestId: 2,
      documentKey: 'doc-1',
      revision: 1,
      lineNumber: 4,
      column: 9,
    });

    expect(readRuntimeReadiness().cursorPath).toMatchObject({
      requestId: 2,
      settled: true,
    });
  });

  it('treats flushed empty graph as settled even without interactive targets', () => {
    syncRuntimeReadinessFromEditorState({
      documentKey: 'doc-1',
      editorRevision: 4,
      fullEditUiState: idleFullEditUiState(),
    });

    markGraphRequested({ documentKey: 'doc-1', revision: 4, mode: 'streaming' });
    markGraphApplied({ documentKey: 'doc-1', revision: 4, mode: 'streaming' });
    markGraphFlushed({ documentKey: 'doc-1', revision: 4, mode: 'streaming' });
    syncGraphInteractionReadiness({
      documentKey: 'doc-1',
      revision: 4,
      mode: 'streaming',
      hasGraphData: false,
      nodeCount: 0,
      pendingRenderWork: false,
      interactiveReady: false,
    });

    expect(readRuntimeReadiness().graph).toMatchObject({
      interactiveRevision: 4,
      settledRevision: 4,
      settled: true,
    });
  });
});
