import type { SnapshotId } from '@core-wasm/index';

type ActiveDocumentSnapshotBinding = {
  documentKey: string;
  revision: number;
  snapshotId: SnapshotId | null;
};

const activeDocumentSnapshotBindings = new Map<string, ActiveDocumentSnapshotBinding>();

export function bindActiveDocumentSnapshot(binding: ActiveDocumentSnapshotBinding): void {
  activeDocumentSnapshotBindings.set(binding.documentKey, binding);
}

export function bindActiveDocumentSnapshotIfPresent(binding: ActiveDocumentSnapshotBinding): void {
  if (binding.snapshotId == null) return;
  bindActiveDocumentSnapshot(binding);
}

export function clearActiveDocumentSnapshot(documentKey: string, snapshotId?: SnapshotId | null): void {
  const current = activeDocumentSnapshotBindings.get(documentKey);
  if (!current) return;
  if (snapshotId != null && current.snapshotId !== snapshotId) return;
  activeDocumentSnapshotBindings.delete(documentKey);
}

export function getActiveDocumentSnapshotId(documentKey: string): SnapshotId | null {
  return activeDocumentSnapshotBindings.get(documentKey)?.snapshotId ?? null;
}
