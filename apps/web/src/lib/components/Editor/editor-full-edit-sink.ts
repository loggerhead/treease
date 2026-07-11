import type { SnapshotId } from '@core-wasm/index';
import {
  appendFullEditStreamChunkMeta,
  beginFullEditStream,
  cancelFullEditStream,
  finishFullEditStream,
  getFullEditUiStateSnapshot,
  markFullEditStreamFinalizing,
  type FullEditTransportKind,
  type FullEditUiState,
} from '../../store/full-edit-ui-store';
import { getWorkspaceTab, updateWorkspaceTab } from '../../store/workspace-store';

type FullEditReason = FullEditUiState['reason'];

export type FullEditBeginPayload = {
  sessionId: string;
  ownerKey: string;
  documentKey: string;
  revision: number;
  language: FullEditUiState['language'];
  transportKind: FullEditTransportKind;
  reason: FullEditReason;
};

export type FullEditSessionPayload = {
  sessionId: string;
  ownerKey: string;
};

export type FullEditChunkPayload = FullEditSessionPayload & {
  streamSeq: number;
  inputByteLength: number;
  modelVersionId?: number | null;
};

export type FullEditSnapshotPayload = {
  documentKey: string;
  revision: number;
  snapshotId: SnapshotId | null;
};

export type FullEditSink = {
  getState: () => FullEditUiState;
  begin: (payload: FullEditBeginPayload) => void;
  appendChunkMeta: (payload: FullEditChunkPayload) => void;
  markFinalizing: (payload: FullEditSessionPayload) => void;
  finish: (payload: FullEditSessionPayload) => void;
  cancel: (payload: FullEditSessionPayload) => void;
  bindSnapshot: (payload: FullEditSnapshotPayload) => void;
};

const idleFullEditState: FullEditUiState = {
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

function createIdleFullEditState(): FullEditUiState {
  return { ...idleFullEditState };
}

function applyIfCurrent(
  tabId: string,
  payload: FullEditSessionPayload,
  next: (current: FullEditUiState) => FullEditUiState,
) {
  const tab = getWorkspaceTab(tabId);
  const current = tab?.fullEditUiState;
  if (!tab || !current.active || current.sessionId !== payload.sessionId || current.ownerKey !== payload.ownerKey) {
    return;
  }
  const nextFullEditUiState = next(current);
  if (nextFullEditUiState === current) return;
  updateWorkspaceTab(tabId, {
    fullEditUiState: nextFullEditUiState,
  });
}

export function createPrimaryFullEditSink(): FullEditSink {
  return {
    getState: () => getFullEditUiStateSnapshot(),
    begin: (payload) => beginFullEditStream(payload),
    appendChunkMeta: (payload) => appendFullEditStreamChunkMeta(payload),
    markFinalizing: (payload) => markFullEditStreamFinalizing(payload),
    finish: (payload) => finishFullEditStream(payload),
    cancel: (payload) => cancelFullEditStream(payload),
    // DocumentSnapshot binding belongs to EditorCommitTransaction's authority
    // landing. This sink only mirrors Full Edit UI lifecycle.
    bindSnapshot: () => undefined,
  };
}

export function createWorkspaceTabFullEditSink(tabId: string): FullEditSink {
  return {
    getState: () => getWorkspaceTab(tabId)?.fullEditUiState ?? createIdleFullEditState(),
    begin: (payload) => {
      updateWorkspaceTab(tabId, {
        revision: payload.revision,
        fullEditUiState: {
          active: true,
          sessionId: payload.sessionId,
          ownerKey: payload.ownerKey,
          documentKey: payload.documentKey,
          revision: payload.revision,
          streamSeq: 0,
          inputByteLength: 0,
          modelVersionId: null,
          byteLength: 0,
          language: payload.language,
          phase: 'streaming',
          sessionKind: 'full-edit',
          transportKind: payload.transportKind,
          reason: payload.reason,
        },
      });
    },
    appendChunkMeta: (payload) =>
      applyIfCurrent(tabId, payload, (current) => {
        if (current.phase !== 'streaming') return current;
        if (payload.streamSeq <= current.streamSeq) return current;
        if (payload.inputByteLength < current.inputByteLength) return current;
        return {
          ...current,
          streamSeq: payload.streamSeq,
          inputByteLength: payload.inputByteLength,
          byteLength: payload.inputByteLength,
          modelVersionId: typeof payload.modelVersionId === 'number' ? payload.modelVersionId : current.modelVersionId,
        };
      }),
    markFinalizing: (payload) =>
      applyIfCurrent(tabId, payload, (current) =>
        current.phase === 'streaming' ? { ...current, phase: 'finalizing' } : current,
      ),
    finish: (payload) => applyIfCurrent(tabId, payload, () => createIdleFullEditState()),
    cancel: (payload) => applyIfCurrent(tabId, payload, () => createIdleFullEditState()),
    // The transaction authority owns the workspace snapshot binding. The
    // sidecar sink only mirrors that already-authoritative snapshot on its tab.
    bindSnapshot: (payload) => {
      const tab = getWorkspaceTab(tabId);
      if (!tab || payload.snapshotId == null) return;
      if (tab.documentKey !== payload.documentKey || payload.revision < tab.revision) return;
      updateWorkspaceTab(tabId, { revision: payload.revision, snapshotId: payload.snapshotId });
    },
  };
}
