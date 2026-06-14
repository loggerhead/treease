import { beforeEach, describe, expect, it } from 'vitest';
import {
  bindActiveDocumentSnapshot,
  bindActiveDocumentSnapshotIfPresent,
  clearActiveDocumentSnapshot,
  getActiveDocumentSnapshotId,
} from './DocumentSessionService';

describe('DocumentSessionService', () => {
  beforeEach(() => {
    clearActiveDocumentSnapshot('doc-1');
    clearActiveDocumentSnapshot('doc-2');
  });

  it('returns the explicitly bound snapshot for a document', () => {
    bindActiveDocumentSnapshot({ documentKey: 'doc-1', revision: 9, snapshotId: 909 });

    expect(getActiveDocumentSnapshotId('doc-1')).toBe(909);
  });

  it('ignores null snapshots in bindActiveDocumentSnapshotIfPresent', () => {
    bindActiveDocumentSnapshotIfPresent({ documentKey: 'doc-2', revision: 1, snapshotId: null });

    expect(getActiveDocumentSnapshotId('doc-2')).toBeNull();
  });
});
