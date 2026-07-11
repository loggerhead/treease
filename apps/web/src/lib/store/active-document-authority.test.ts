import { beforeEach, describe, expect, it } from 'vitest';
import {
  bindAuthoritySnapshot,
  getActiveDocumentContext,
  getAuthorityDocumentSessionState,
  getAuthorityWorkspaceState,
  getActiveDocumentCommitBaseSnapshotId,
  getActiveDocumentSuccessfulSnapshotId,
  getActiveDocumentSemanticState,
  markActiveDocumentSemanticInvalid,
  markActiveDocumentSemanticPending,
  markActiveDocumentSemanticTerminal,
  markActiveDocumentSemanticValid,
  patchAuthorityActiveDocument,
  resetActiveDocumentAuthority,
  setAuthorityEditorIO,
  setAuthorityWorkspaceState,
} from './active-document-authority';
import { addWorkspaceTab, activateWorkspaceTab } from './editor-workspace';

describe('active document authority semantic outcomes', () => {
  beforeEach(() => {
    resetActiveDocumentAuthority();
  });

  it('keeps snapshot ready as the readable and commit-base snapshot for its revision', () => {
    markActiveDocumentSemanticPending({ documentKey: 'doc', language: 'json', revision: 4 });
    markActiveDocumentSemanticValid({ documentKey: 'doc', language: 'json', revision: 4, snapshotId: 41 as any });

    expect(getActiveDocumentSuccessfulSnapshotId('doc', 4)).toBe(41);
    expect(getActiveDocumentCommitBaseSnapshotId('doc')).toBe(41);
  });

  it('keeps parse failed snapshot readable only as a commit base', () => {
    markActiveDocumentSemanticValid({ documentKey: 'doc', language: 'json', revision: 1, snapshotId: 10 as any });
    markActiveDocumentSemanticPending({ documentKey: 'doc', language: 'json', revision: 2 });
    markActiveDocumentSemanticInvalid({ documentKey: 'doc', language: 'json', revision: 2, snapshotId: 20 as any });

    expect(getActiveDocumentSuccessfulSnapshotId('doc', 2)).toBeNull();
    expect(getActiveDocumentCommitBaseSnapshotId('doc')).toBe(20);
  });

  it('does not let a stale runtime outcome replace the current revision', () => {
    markActiveDocumentSemanticValid({ documentKey: 'doc', language: 'json', revision: 8, snapshotId: 80 as any });
    markActiveDocumentSemanticInvalid({ documentKey: 'doc', language: 'json', revision: 7, snapshotId: 70 as any });

    expect(getActiveDocumentSemanticState('doc')).toMatchObject({ revision: 8, status: 'valid', snapshotId: 80 });
  });

  it('clears snapshot eligibility for rejected and noSnapshot terminal outcomes', () => {
    markActiveDocumentSemanticValid({ documentKey: 'doc', language: 'json', revision: 1, snapshotId: 10 as any });
    markActiveDocumentSemanticTerminal({ documentKey: 'doc', language: 'json', revision: 2, status: 'rejected' });

    expect(getActiveDocumentSuccessfulSnapshotId('doc', 2)).toBeNull();
    expect(getActiveDocumentCommitBaseSnapshotId('doc')).toBeNull();
  });

  it('changes the single active document when a workspace tab is activated', () => {
    patchAuthorityActiveDocument({ documentKey: 'first', languageId: 'json', sourceText: '{"one":1}', revision: 1 });
    const workspace = addWorkspaceTab(getAuthorityWorkspaceState(), {
      id: 'second', name: 'Second', documentKey: 'second', languageId: 'yaml' as any, sourceText: 'two: 2\n', revision: 3,
    });
    setAuthorityWorkspaceState(activateWorkspaceTab(workspace, 'second'));

    expect(getAuthorityDocumentSessionState()).toMatchObject({ documentKey: 'second', languageId: 'yaml', sourceText: 'two: 2\n', editorRevision: 3 });
  });

  it('uses Workspace Mirror Text when the Monaco model adapter is absent', () => {
    patchAuthorityActiveDocument({ documentKey: 'doc', languageId: 'json', sourceText: '{"mirror":true}', revision: 1 });
    setAuthorityEditorIO(null);

    expect(getActiveDocumentContext()).toMatchObject({ text: '{"mirror":true}', textSource: 'workspaceTab', model: null });
  });

  it('drops stale snapshot bindings and clears them on workspace reset', () => {
    patchAuthorityActiveDocument({ documentKey: 'doc', languageId: 'json', revision: 5 });
    bindAuthoritySnapshot({ documentKey: 'doc', revision: 5, snapshotId: 50 as any });
    bindAuthoritySnapshot({ documentKey: 'doc', revision: 4, snapshotId: 40 as any });
    expect(getAuthorityWorkspaceState().snapshotBindingsByDocumentKey.doc.snapshotId).toBe(50);

    resetActiveDocumentAuthority();
    expect(getAuthorityWorkspaceState().snapshotBindingsByDocumentKey).toEqual({});
  });
});
