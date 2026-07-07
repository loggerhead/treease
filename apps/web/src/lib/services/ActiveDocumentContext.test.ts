import { beforeEach, describe, expect, it } from 'vitest';
import { initialDocumentSessionState, setDocumentSessionState } from '../store/document-session-store';
import { clearActiveDocumentSemanticState, markActiveDocumentSemanticInvalid, markActiveDocumentSemanticValid } from '../store/active-document-semantic-state';
import { initialWorkspaceState, setWorkspaceState } from '../store/workspace-store';
import { getActiveDocumentContext, resolveCommitBaseSnapshotId, resolveReadableSnapshotId } from './ActiveDocumentContext';

function createWorkspaceForDocument(documentKey: string, snapshotId: number | null, revision = 3) {
  return {
    ...initialWorkspaceState,
    tabsById: {
      ...initialWorkspaceState.tabsById,
      primary: {
        ...initialWorkspaceState.tabsById.primary,
        documentKey,
        languageId: 'json' as const,
        revision,
        snapshotId,
        sourceText: '{"a":1}',
      },
    },
    snapshotBindingsByDocumentKey:
      snapshotId == null
        ? {}
        : {
            [documentKey]: { documentKey, revision, snapshotId: snapshotId as any },
          },
  };
}

describe('ActiveDocumentContext snapshot resolution', () => {
  beforeEach(() => {
    clearActiveDocumentSemanticState();
    setDocumentSessionState(initialDocumentSessionState);
    setWorkspaceState(initialWorkspaceState);
  });

  it('prefers the current revision semantic snapshot over workspace bindings', () => {
    setDocumentSessionState({
      ...initialDocumentSessionState,
      documentKey: 'doc-json',
      languageId: 'json',
      editorRevision: 3,
      sourceText: '{"a":1}',
    });
    setWorkspaceState(createWorkspaceForDocument('doc-json', 11, 2));
    markActiveDocumentSemanticValid({
      documentKey: 'doc-json',
      language: 'json',
      revision: 3,
      snapshotId: 42 as any,
    });

    expect(resolveReadableSnapshotId('doc-json', 3)).toBe(42);
    expect(getActiveDocumentContext().snapshotId).toBe(42);
  });

  it('falls back to workspace binding when the current revision has no semantic snapshot', () => {
    setDocumentSessionState({
      ...initialDocumentSessionState,
      documentKey: 'doc-json',
      languageId: 'json',
      editorRevision: 3,
      sourceText: '{"a":1}',
    });
    setWorkspaceState(createWorkspaceForDocument('doc-json', 11, 2));

    expect(resolveReadableSnapshotId('doc-json', 3)).toBe(11);
    expect(getActiveDocumentContext().snapshotId).toBe(11);
  });

  it('prefers the semantic commit base snapshot before workspace bindings', () => {
    setWorkspaceState(createWorkspaceForDocument('doc-json', 11, 2));
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
      snapshotId: 43 as any,
    });

    expect(resolveCommitBaseSnapshotId('doc-json')).toBe(43);
  });
});
