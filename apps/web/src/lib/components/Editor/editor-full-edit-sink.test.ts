import { beforeEach, describe, expect, it } from 'vitest';
import {
  clearWorkspaceSnapshot,
  getWorkspaceSnapshotId,
} from '../../store/workspace-snapshot-bindings';
import { editorStore, type FullEditUiState } from '../../store/editor-store';
import { createPrimaryFullEditSink, createWorkspaceTabFullEditSink } from './editor-full-edit-sink';

const primaryTabId = 'tab-primary';
const sidecarTabId = 'tab-sidecar';
const primaryDocumentKey = 'primary-doc:0';
const sidecarDocumentKey = 'sidecar:tab-sidecar:0';

function expectLegacyPrimaryFullEditState(expected: Partial<FullEditUiState> = { active: false, phase: 'idle' }) {
  const state = editorStore.get();
  expect(state.fullEditUiState).toMatchObject(expected);
  expect(state.workspace.tabsById[primaryTabId].fullEditUiState).toMatchObject(expected);
}

describe('editor-full-edit-sink', () => {
  beforeEach(() => {
    editorStore.reset();
    clearWorkspaceSnapshot(primaryDocumentKey);
    clearWorkspaceSnapshot(sidecarDocumentKey);
    editorStore.actions.setDocumentKey(primaryDocumentKey);
    editorStore.actions.setLanguageId('json');
    editorStore.actions.initWorkspaceFromPrimaryTab({
      id: primaryTabId,
      name: 'Untitled 1',
    });
    editorStore.actions.ensureSidecarWorkspaceTab({
      id: sidecarTabId,
      name: 'Right Editor',
      sourceText: '{}',
    });
  });

  it('primary sink publishes to legacy fullEditUiState', () => {
    const sink = createPrimaryFullEditSink();

    sink.begin({
      sessionId: 'primary-doc:1',
      ownerKey: 'inmemory://model/tab-primary',
      documentKey: primaryDocumentKey,
      revision: 1,
      language: 'json',
      transportKind: 'memory',
      reason: 'whole-document-replacement',
    });

    expect(editorStore.get().fullEditUiState).toMatchObject({
      active: true,
      sessionId: 'primary-doc:1',
      documentKey: primaryDocumentKey,
      revision: 1,
      phase: 'streaming',
    });
  });

  it('workspace tab sink publishes begin, chunk, finalizing, cancel, finish, and snapshot only to target sidecar tab', () => {
    const sink = createWorkspaceTabFullEditSink(sidecarTabId);

    sink.begin({
      sessionId: 'sidecar-doc:1',
      ownerKey: 'inmemory://scratch/tab-sidecar',
      documentKey: sidecarDocumentKey,
      revision: 2,
      language: 'json',
      transportKind: 'memory',
      reason: 'whole-document-replacement',
    });
    expectLegacyPrimaryFullEditState();
    expect(editorStore.get().workspace.tabsById[sidecarTabId]).toMatchObject({
      revision: 2,
      fullEditUiState: {
        active: true,
        sessionId: 'sidecar-doc:1',
        ownerKey: 'inmemory://scratch/tab-sidecar',
        documentKey: sidecarDocumentKey,
        revision: 2,
        phase: 'streaming',
      },
    });

    sink.appendChunkMeta({
      sessionId: 'sidecar-doc:1',
      ownerKey: 'inmemory://scratch/tab-sidecar',
      streamSeq: 1,
      inputByteLength: 12,
      modelVersionId: 7,
    });
    expectLegacyPrimaryFullEditState();
    expect(editorStore.get().workspace.tabsById[sidecarTabId].fullEditUiState).toMatchObject({
      streamSeq: 1,
      inputByteLength: 12,
      byteLength: 12,
      modelVersionId: 7,
    });

    sink.markFinalizing({
      sessionId: 'sidecar-doc:1',
      ownerKey: 'inmemory://scratch/tab-sidecar',
    });
    expectLegacyPrimaryFullEditState();
    expect(editorStore.get().workspace.tabsById[sidecarTabId].fullEditUiState).toMatchObject({
      active: true,
      phase: 'finalizing',
    });

    sink.cancel({
      sessionId: 'sidecar-doc:1',
      ownerKey: 'inmemory://scratch/tab-sidecar',
    });
    expectLegacyPrimaryFullEditState();
    expect(editorStore.get().workspace.tabsById[sidecarTabId].fullEditUiState).toMatchObject({
      active: false,
      phase: 'idle',
    });

    sink.begin({
      sessionId: 'sidecar-doc:2',
      ownerKey: 'inmemory://scratch/tab-sidecar',
      documentKey: sidecarDocumentKey,
      revision: 3,
      language: 'json',
      transportKind: 'memory',
      reason: 'whole-document-replacement',
    });
    sink.finish({
      sessionId: 'sidecar-doc:2',
      ownerKey: 'inmemory://scratch/tab-sidecar',
    });
    expectLegacyPrimaryFullEditState();
    expect(editorStore.get().workspace.tabsById[sidecarTabId].fullEditUiState).toMatchObject({
      active: false,
      phase: 'idle',
    });

    sink.bindSnapshot({
      documentKey: sidecarDocumentKey,
      revision: 3,
      snapshotId: 99,
    });

    const state = editorStore.get();
    expect(state.workspace.tabsById[sidecarTabId]).toMatchObject({
      revision: 3,
      snapshotId: 99,
      fullEditUiState: {
        active: false,
        phase: 'idle',
      },
    });
    expect(state.workspace.tabsById[primaryTabId].snapshotId).toBeNull();
    expectLegacyPrimaryFullEditState();
  });

  it('workspace tab sink ignores stale and non-monotonic chunk payloads', () => {
    const sink = createWorkspaceTabFullEditSink(sidecarTabId);
    sink.begin({
      sessionId: 'sidecar-doc:1',
      ownerKey: 'inmemory://scratch/tab-sidecar',
      documentKey: sidecarDocumentKey,
      revision: 2,
      language: 'json',
      transportKind: 'memory',
      reason: 'whole-document-replacement',
    });
    sink.appendChunkMeta({
      sessionId: 'sidecar-doc:1',
      ownerKey: 'inmemory://scratch/tab-sidecar',
      streamSeq: 2,
      inputByteLength: 20,
      modelVersionId: 2,
    });

    sink.appendChunkMeta({
      sessionId: 'stale-session',
      ownerKey: 'inmemory://scratch/tab-sidecar',
      streamSeq: 3,
      inputByteLength: 30,
      modelVersionId: 3,
    });
    sink.appendChunkMeta({
      sessionId: 'sidecar-doc:1',
      ownerKey: 'inmemory://scratch/tab-sidecar',
      streamSeq: 2,
      inputByteLength: 40,
      modelVersionId: 4,
    });
    sink.appendChunkMeta({
      sessionId: 'sidecar-doc:1',
      ownerKey: 'inmemory://scratch/tab-sidecar',
      streamSeq: 3,
      inputByteLength: 10,
      modelVersionId: 5,
    });
    sink.markFinalizing({
      sessionId: 'stale-session',
      ownerKey: 'inmemory://scratch/tab-sidecar',
    });
    sink.finish({
      sessionId: 'stale-session',
      ownerKey: 'inmemory://scratch/tab-sidecar',
    });
    sink.cancel({
      sessionId: 'stale-session',
      ownerKey: 'inmemory://scratch/tab-sidecar',
    });

    expect(editorStore.get().workspace.tabsById[sidecarTabId].fullEditUiState).toMatchObject({
      active: true,
      sessionId: 'sidecar-doc:1',
      streamSeq: 2,
      inputByteLength: 20,
      byteLength: 20,
      modelVersionId: 2,
      phase: 'streaming',
    });
    expectLegacyPrimaryFullEditState();
  });

  it('sidecar bindSnapshot does not pollute primary tab snapshot or legacy primary fullEditUiState', () => {
    const primarySink = createPrimaryFullEditSink();
    const sidecarSink = createWorkspaceTabFullEditSink(sidecarTabId);
    primarySink.begin({
      sessionId: 'primary-doc:1',
      ownerKey: 'inmemory://model/tab-primary',
      documentKey: primaryDocumentKey,
      revision: 1,
      language: 'json',
      transportKind: 'memory',
      reason: 'whole-document-replacement',
    });
    const legacyBeforeBind = editorStore.get().fullEditUiState;

    sidecarSink.bindSnapshot({
      documentKey: sidecarDocumentKey,
      revision: 4,
      snapshotId: 101,
    });

    const state = editorStore.get();
    expect(state.fullEditUiState).toEqual(legacyBeforeBind);
    expect(state.workspace.tabsById[primaryTabId].fullEditUiState).toEqual(legacyBeforeBind);
    expect(state.workspace.tabsById[primaryTabId].snapshotId).toBeNull();
    expect(state.workspace.tabsById[sidecarTabId]).toMatchObject({
      revision: 4,
      snapshotId: 101,
    });
    expect(getWorkspaceSnapshotId(primaryDocumentKey)).toBeNull();
    expect(getWorkspaceSnapshotId(sidecarDocumentKey)).toBe(101);
  });

  it('sidecar bindSnapshot ignores foreign documentKey without binding the primary document snapshot', () => {
    const primarySink = createPrimaryFullEditSink();
    const sidecarSink = createWorkspaceTabFullEditSink(sidecarTabId);
    primarySink.begin({
      sessionId: 'primary-doc:1',
      ownerKey: 'inmemory://model/tab-primary',
      documentKey: primaryDocumentKey,
      revision: 1,
      language: 'json',
      transportKind: 'memory',
      reason: 'whole-document-replacement',
    });
    const legacyBeforeBind = editorStore.get().fullEditUiState;
    const sidecarBeforeBind = editorStore.get().workspace.tabsById[sidecarTabId];

    sidecarSink.bindSnapshot({
      documentKey: primaryDocumentKey,
      revision: 9,
      snapshotId: 202,
    });

    const state = editorStore.get();
    expect(state.fullEditUiState).toEqual(legacyBeforeBind);
    expect(state.workspace.tabsById[primaryTabId].fullEditUiState).toEqual(legacyBeforeBind);
    expect(state.workspace.tabsById[sidecarTabId]).toMatchObject({
      revision: sidecarBeforeBind.revision,
      snapshotId: sidecarBeforeBind.snapshotId,
    });
    expect(getWorkspaceSnapshotId(primaryDocumentKey)).toBeNull();
    expect(getWorkspaceSnapshotId(sidecarDocumentKey)).toBeNull();
  });

  it('sidecar bindSnapshot ignores stale revision without overwriting the current tab snapshot', () => {
    const sidecarSink = createWorkspaceTabFullEditSink(sidecarTabId);
    sidecarSink.begin({
      sessionId: 'sidecar-doc:1',
      ownerKey: 'inmemory://scratch/tab-sidecar',
      documentKey: sidecarDocumentKey,
      revision: 10,
      language: 'json',
      transportKind: 'memory',
      reason: 'whole-document-replacement',
    });
    sidecarSink.finish({
      sessionId: 'sidecar-doc:1',
      ownerKey: 'inmemory://scratch/tab-sidecar',
    });

    sidecarSink.bindSnapshot({
      documentKey: sidecarDocumentKey,
      revision: 9,
      snapshotId: 303,
    });

    const state = editorStore.get();
    expect(state.workspace.tabsById[sidecarTabId]).toMatchObject({
      revision: 10,
      snapshotId: null,
    });
    expect(getWorkspaceSnapshotId(sidecarDocumentKey)).toBeNull();
    expectLegacyPrimaryFullEditState();
  });

  it('sidecar bindSnapshot ignores missing tabs without binding a snapshot', () => {
    const missingSink = createWorkspaceTabFullEditSink('missing-tab');

    expect(() =>
      missingSink.bindSnapshot({
        documentKey: sidecarDocumentKey,
        revision: 1,
        snapshotId: 404,
      }),
    ).not.toThrow();

    expect(getWorkspaceSnapshotId(sidecarDocumentKey)).toBeNull();
    expectLegacyPrimaryFullEditState();
  });
});
