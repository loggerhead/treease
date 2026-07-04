import type { SnapshotId } from '@core-wasm/index';
import { editorStore } from './editor-store';

export type WorkspaceSnapshotBindingPayload = {
  documentKey: string;
  revision: number;
  snapshotId: SnapshotId | null;
};

export function bindWorkspaceSnapshotIfPresent(payload: WorkspaceSnapshotBindingPayload): void {
  editorStore.actions.bindWorkspaceSnapshot(payload);
}

export function clearWorkspaceSnapshot(documentKey: string, snapshotId?: SnapshotId | null): void {
  editorStore.actions.clearWorkspaceSnapshot(documentKey, snapshotId);
}

export function getWorkspaceSnapshotId(documentKey: string): SnapshotId | null {
  return editorStore.actions.getWorkspaceSnapshotId(documentKey);
}
