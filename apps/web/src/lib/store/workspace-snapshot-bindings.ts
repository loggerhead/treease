import type { SnapshotId } from '@core-wasm/index';
import {
  bindWorkspaceSnapshot,
  clearWorkspaceSnapshotBinding,
  getWorkspaceSnapshotId as getBoundWorkspaceSnapshotId,
} from './workspace-store';

export type WorkspaceSnapshotBindingPayload = {
  documentKey: string;
  revision: number;
  snapshotId: SnapshotId | null;
};

export function bindWorkspaceSnapshotIfPresent(payload: WorkspaceSnapshotBindingPayload): void {
  bindWorkspaceSnapshot(payload);
}

export function clearWorkspaceSnapshot(documentKey: string, snapshotId?: SnapshotId | null): void {
  clearWorkspaceSnapshotBinding(documentKey, snapshotId);
}

export function getWorkspaceSnapshotId(documentKey: string): SnapshotId | null {
  return getBoundWorkspaceSnapshotId(documentKey);
}
