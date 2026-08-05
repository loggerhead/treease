import { describe, expect, it } from 'vitest';
import { createEditorWorkspaceState, transitionWorkspaceTabDocument } from './editor-workspace';
import { tabTargetStatus, targetOfTab } from './tab-target';

function workspace() {
  return createEditorWorkspaceState({
    id: 'main', role: 'primary', name: 'Untitled', documentKey: 'main:0', languageId: 'json', sourceText: '{}',
    revision: 0, graphAppliedRevision: 0, snapshotId: null,
    tempModel: { diffInputText: '', scratchText: '{}', commandQuery: '', status: 'Ready', error: '', cursor: '', selectionLength: 0, treePath: [], graphHighlight: null, diagnostics: [] },
    fullEditUiState: { active: false, sessionId: null, ownerKey: null, documentKey: null, revision: 0, streamSeq: 0, inputByteLength: 0, modelVersionId: null, byteLength: 0, language: '', phase: 'idle', sessionKind: null, transportKind: null, reason: null },
  });
}

describe('TabTarget', () => {
  it('invalidates targets after replacement and closure', () => {
    const initial = workspace();
    const target = targetOfTab(initial.tabsById.main);
    const replaced = transitionWorkspaceTabDocument(initial, { tabId: 'main', expected: { documentKey: 'main:0', languageId: 'json', revision: 0 }, next: { documentKey: 'main:1', languageId: 'json', revision: 1, sourceText: '{}' } })!;
    expect(tabTargetStatus(replaced, target)).toBe('stale');
    expect(tabTargetStatus({ ...replaced, tabsById: {} }, target)).toBe('closed');
  });

  it('invalidates the paired sidecar target with its main document replacement', () => {
    const initial = workspace();
    const sidecar = initial.tabsById[initial.tabsById.main.sidecarTabId!];
    const target = targetOfTab(sidecar);
    const replaced = transitionWorkspaceTabDocument(initial, {
      tabId: 'main',
      expected: { documentKey: 'main:0', languageId: 'json', revision: 0 },
      next: { documentKey: 'main:1', languageId: 'json', revision: 1, sourceText: '{}' },
    })!;
    expect(tabTargetStatus(replaced, target)).toBe('stale');
  });
});
