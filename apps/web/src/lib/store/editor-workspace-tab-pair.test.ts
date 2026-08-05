import { describe, expect, it } from 'vitest';

import {
  activateWorkspaceTab,
  closeWorkspaceTabTransition,
  createEditorWorkspaceState,
  createWorkspaceTabTransition,
  ensureColumnDetailDraftTab,
  hasValidTabPairs,
  removeColumnDetailDraftTab,
  transitionWorkspaceTabDocument,
  type EditorWorkspaceTab,
} from './editor-workspace';
import { editorLanguageFallback } from '../monaco/language-support';
import { initialFullEditUiState } from './full-edit-ui-store';
import { initialTempModel } from './graph-selection-store';

function mainTab(id = 'main'): EditorWorkspaceTab {
  return {
    id,
    role: 'primary',
    name: id,
    documentKey: `${id}:0`,
    languageId: editorLanguageFallback,
    sourceText: '{"value":1}',
    revision: 0,
    graphAppliedRevision: 0,
    snapshotId: null,
    tempModel: initialTempModel,
    fullEditUiState: initialFullEditUiState,
  };
}

describe('editor workspace tab pairs', () => {
  it('creates and activates one explicit sidecar for every left tab', () => {
    const initial = createEditorWorkspaceState(mainTab());
    const firstSidecarId = initial.tabsById.main.sidecarTabId;
    expect(firstSidecarId).toBeTruthy();
    expect(initial.tabsById[firstSidecarId!]).toMatchObject({ role: 'sidecar', ownerMainTabId: 'main' });
    expect(initial.paneTabIds.right).toBe(firstSidecarId);
    expect(initial.tabOrder).toEqual(['main']);
    expect(hasValidTabPairs(initial)).toBe(true);

    const transition = createWorkspaceTabTransition(initial, {
      id: 'second', name: 'second', documentKey: 'second:0', languageId: editorLanguageFallback, sourceText: '',
    });
    expect(transition).not.toBeNull();
    const next = transition!.workspace;
    const secondSidecarId = next.tabsById.second.sidecarTabId;
    expect(secondSidecarId).toBeTruthy();
    expect(next.tabsById[secondSidecarId!]).toMatchObject({ role: 'sidecar', ownerMainTabId: 'second' });
    expect(next.paneTabIds.right).toBe(secondSidecarId);
    expect(hasValidTabPairs(next)).toBe(true);

    const restored = activateWorkspaceTab(next, 'main');
    expect(restored.paneTabIds.right).toBe(firstSidecarId);
    expect(restored.tabsById[secondSidecarId!].sourceText).toBe('');
  });

  it('does not reuse a stale sidecar id when rebuilding a workspace primary', () => {
    const restored = createEditorWorkspaceState({ ...mainTab('restored'), sidecarTabId: 'closed:sidecar' });
    expect(restored.tabsById.restored.sidecarTabId).toBe('restored:sidecar');
    expect(restored.tabsById['restored:sidecar']).toMatchObject({ ownerMainTabId: 'restored' });
    expect(restored.tabsById['closed:sidecar']).toBeUndefined();
  });

  it('keeps a Column Detail draft outside paired-sidecar topology', () => {
    const withDraft = ensureColumnDetailDraftTab(createEditorWorkspaceState(mainTab()), {
      id: 'column-detail',
      name: 'Column Detail',
      languageId: editorLanguageFallback,
      sourceText: '1',
    });

    expect(withDraft.tabsById['column-detail']).toMatchObject({ role: 'column-detail-draft' });
    expect(hasValidTabPairs(withDraft)).toBe(true);
    expect(removeColumnDetailDraftTab(withDraft, 'column-detail').tabsById['column-detail']).toBeUndefined();
  });

  it('removes the paired sidecar and gives a replacement blank tab a fresh pair', () => {
    const initial = createEditorWorkspaceState(mainTab());
    const closedSidecarId = initial.tabsById.main.sidecarTabId!;
    const transition = closeWorkspaceTabTransition(initial, 'main', {
      id: 'blank', documentKey: 'blank:0', name: 'Untitled', languageId: editorLanguageFallback,
    });
    expect(transition).not.toBeNull();
    const next = transition!.workspace;
    expect(next.tabsById[closedSidecarId]).toBeUndefined();
    expect(next.tabsById.blank.sidecarTabId).toBeTruthy();
    expect(next.tabsById[next.tabsById.blank.sidecarTabId!]).toMatchObject({ ownerMainTabId: 'blank', sourceText: '' });
    expect(hasValidTabPairs(next)).toBe(true);
  });

  it('replaces the paired sidecar identity and clears compare state when its document is replaced', () => {
    const initial = createEditorWorkspaceState(mainTab());
    const sidecarId = initial.tabsById.main.sidecarTabId!;
    const withCompare = {
      ...initial,
      tabsById: {
        ...initial.tabsById,
        [sidecarId]: { ...initial.tabsById[sidecarId], sourceText: '{"compare":true}' },
      },
    };
    const next = transitionWorkspaceTabDocument(withCompare, {
      tabId: 'main',
      expected: { documentKey: 'main:0', languageId: editorLanguageFallback, revision: 0 },
      next: { documentKey: 'main:1', languageId: editorLanguageFallback, revision: 0, sourceText: '' },
    });
    expect(next?.tabsById[sidecarId]).toMatchObject({
      ownerMainTabId: 'main',
      sourceText: '',
    });
    expect(next?.tabsById[sidecarId].documentKey).not.toBe(`sidecar:${sidecarId}:0`);
  });
});
