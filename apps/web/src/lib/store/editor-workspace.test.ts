import { describe, expect, it } from 'vitest';
import {
  activateWorkspaceTab,
  addWorkspaceTab,
  closeWorkspaceTabTransition,
  createEditorWorkspaceState,
  ensureSidecarTab,
  reinitializeWorkspaceFromPrimaryTab,
  summarizeWorkspaceTabs,
  isWorkspaceTabDirty,
  syncSidecarLanguageFromPrimary,
  syncWorkspaceEditorTab,
  updateWorkspaceTab,
  type EditorWorkspaceTab,
} from './editor-workspace';

function keySeg(key: string) {
  return { tag: 0, key, index: 0 };
}

function tab(overrides: Partial<EditorWorkspaceTab> = {}): EditorWorkspaceTab {
  return {
    id: 'tab-primary',
    role: 'primary',
    name: 'Untitled 1',
    documentKey: 'tab-primary:0',
    languageId: 'json' as any,
    sourceText: '{"a":1}',
    revision: 1,
    graphAppliedRevision: 1,
    snapshotId: 7,
    tempModel: {
      diffInputText: '',
      scratchText: '',
      commandQuery: '',
      status: 'Ready',
      error: '',
      cursor: 'Ln 1, Col 1',
      selectionLength: 0,
      treePath: [],
      graphHighlight: null,
      diagnostics: [],
    },
    fullEditUiState: {
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
    },
    ...overrides,
  };
}

describe('editor-workspace', () => {
  it('creates a workspace with one primary tab and no sidecar tab', () => {
    const workspace = createEditorWorkspaceState(tab());

    expect(workspace.primaryTabId).toBe('tab-primary');
    expect(workspace.activeTabId).toBe('tab-primary');
    expect(workspace.tabOrder).toEqual(['tab-primary']);
    expect(workspace.paneTabIds.left).toBe('tab-primary');
    expect(workspace.paneTabIds.right).toBeNull();
    expect(workspace.tabsById['tab-primary'].role).toBe('primary');
  });

  it('tracks ordered left tabs separately from the sidecar tab', () => {
    const workspace = ensureSidecarTab(
      addWorkspaceTab(createEditorWorkspaceState(tab()), {
        id: 'tab-second',
        name: 'Untitled 2',
        documentKey: 'tab-second:0',
        languageId: 'yaml' as any,
        sourceText: 'name: second\n',
      }),
      {
        id: 'tab-sidecar',
        name: 'Right Editor',
        languageId: 'json' as any,
        sourceText: '{"right":true}',
      },
    );

    expect(workspace.tabOrder).toEqual(['tab-primary', 'tab-second']);
    expect(workspace.activeTabId).toBe('tab-primary');
    expect(workspace.paneTabIds.left).toBe('tab-primary');
    expect(workspace.paneTabIds.right).toBe('tab-sidecar');
    expect(summarizeWorkspaceTabs(workspace)).toEqual([
      { id: 'tab-primary', name: 'Untitled 1', languageId: 'json', dirty: false },
      { id: 'tab-second', name: 'Untitled 2', languageId: 'yaml', dirty: false },
    ]);
  });

  it('keeps file linkage per tab and derives dirty state from the saved text', () => {
    const linked = tab({
      fileLinkedDocument: { grantId: 'file-1', name: 'config.json' },
      savedText: '{"a":1}',
    });
    const dirty = updateWorkspaceTab(createEditorWorkspaceState(linked), linked.id, { sourceText: '{"a":2}' });

    expect(isWorkspaceTabDirty(linked)).toBe(false);
    expect(isWorkspaceTabDirty(dirty.tabsById[linked.id])).toBe(true);
    expect(dirty.tabsById[linked.id].fileLinkedDocument).toEqual({ grantId: 'file-1', name: 'config.json' });
  });

  it('activates a background tab as the only primary tab', () => {
    const workspace = addWorkspaceTab(createEditorWorkspaceState(tab()), {
      id: 'tab-second',
      name: 'Untitled 2',
      documentKey: 'tab-second:0',
      languageId: 'yaml' as any,
      sourceText: 'name: second\n',
    });

    const next = activateWorkspaceTab(workspace, 'tab-second');

    expect(next.primaryTabId).toBe('tab-second');
    expect(next.activeTabId).toBe('tab-second');
    expect(next.paneTabIds.left).toBe('tab-second');
    expect(next.tabsById['tab-primary'].role).toBe('background');
    expect(next.tabsById['tab-second'].role).toBe('primary');
  });

  it('ignores attempts to activate the sidecar tab as primary', () => {
    const workspace = ensureSidecarTab(createEditorWorkspaceState(tab()), {
      id: 'tab-sidecar',
      name: 'Right Editor',
      languageId: 'json' as any,
      sourceText: '{}',
    });

    const next = activateWorkspaceTab(workspace, 'tab-sidecar');

    expect(next).toBe(workspace);
    expect(next.primaryTabId).toBe('tab-primary');
    expect(next.paneTabIds.right).toBe('tab-sidecar');
  });

  it('closes an inactive background tab without changing the active primary tab', () => {
    const workspace = addWorkspaceTab(createEditorWorkspaceState(tab()), {
      id: 'tab-second',
      name: 'Untitled 2',
      documentKey: 'tab-second:0',
      languageId: 'yaml' as any,
      sourceText: 'name: second\n',
    });

    const result = closeWorkspaceTabTransition(workspace, 'tab-second', { id: 'blank', documentKey: 'blank:0', name: 'Untitled', languageId: 'json' as any })!;

    expect(result.effect).toEqual({ kind: 'activate-existing', tabId: 'tab-primary', disposeTabId: 'tab-second' });
    expect(result.workspace.tabOrder).toEqual(['tab-primary']);
    expect(result.workspace.primaryTabId).toBe('tab-primary');
    expect(result.workspace.tabsById['tab-second']).toBeUndefined();
  });

  it('closes the active tab and promotes the previous left tab', () => {
    const workspace = activateWorkspaceTab(
      addWorkspaceTab(createEditorWorkspaceState(tab()), {
        id: 'tab-second',
        name: 'Untitled 2',
        documentKey: 'tab-second:0',
        languageId: 'yaml' as any,
        sourceText: 'name: second\n',
      }),
      'tab-second',
    );

    const result = closeWorkspaceTabTransition(workspace, 'tab-second', { id: 'blank', documentKey: 'blank:0', name: 'Untitled', languageId: 'json' as any })!;

    expect(result.effect).toEqual({ kind: 'activate-existing', tabId: 'tab-primary', disposeTabId: 'tab-second' });
    expect(result.workspace.primaryTabId).toBe('tab-primary');
    expect(result.workspace.tabsById['tab-primary'].role).toBe('primary');
  });

  it('replaces the last left tab with a new blank primary document', () => {
    const workspace = createEditorWorkspaceState(tab());

    const result = closeWorkspaceTabTransition(workspace, 'tab-primary', { id: 'tab-blank', documentKey: 'tab-blank:0', name: 'Untitled 2', languageId: 'json' as any })!;

    expect(result.effect).toEqual({ kind: 'activate-new-blank', tabId: 'tab-blank', documentKey: 'tab-blank:0', disposeTabId: 'tab-primary' });
    expect(result.workspace.tabOrder).toEqual(['tab-blank']);
    expect(result.workspace.tabsById['tab-blank']).toMatchObject({ role: 'primary', sourceText: '', documentKey: 'tab-blank:0' });
  });

  it('preserves the sidecar when closing the last left tab', () => {
    const workspace = ensureSidecarTab(createEditorWorkspaceState(tab()), {
      id: 'tab-sidecar',
      name: 'Right Editor',
      languageId: 'json' as any,
      sourceText: '{"right":true}',
    });

    const result = closeWorkspaceTabTransition(workspace, 'tab-primary', { id: 'tab-blank', name: 'Untitled', documentKey: 'tab-blank:0', languageId: 'yaml' as any })!;

    expect(result.workspace.primaryTabId).toBe('tab-blank');
    expect(result.workspace.activeTabId).toBe('tab-blank');
    expect(result.workspace.paneTabIds.left).toBe('tab-blank');
    expect(result.workspace.paneTabIds.right).toBe('tab-sidecar');
    expect(result.workspace.tabOrder).toEqual(['tab-blank']);
    expect(result.workspace.tabsById['tab-blank']).toMatchObject({
      role: 'primary',
      name: 'Untitled',
      documentKey: 'tab-blank:0',
      languageId: 'yaml',
      sourceText: '',
    });
    expect(result.workspace.tabsById['tab-sidecar']).toMatchObject({
      role: 'sidecar',
      sourceText: '{"right":true}',
    });
  });

  it('drops only the removed document snapshot binding', () => {
    const workspace = addWorkspaceTab(createEditorWorkspaceState(tab()), {
      id: 'tab-second',
      name: 'Untitled 2',
      documentKey: 'tab-second:0',
      languageId: 'yaml' as any,
      sourceText: 'name: second\n',
    });
    const withBindings = { ...workspace, snapshotBindingsByDocumentKey: {
      'tab-primary:0': { documentKey: 'tab-primary:0', revision: 1, snapshotId: 7 },
      'tab-second:0': { documentKey: 'tab-second:0', revision: 1, snapshotId: 8 },
    } };

    const result = closeWorkspaceTabTransition(withBindings, 'tab-second', {
      id: 'tab-blank', documentKey: 'tab-blank:0', name: 'Untitled', languageId: 'json' as any,
    })!;

    expect(result.workspace.snapshotBindingsByDocumentKey).toEqual({
      'tab-primary:0': { documentKey: 'tab-primary:0', revision: 1, snapshotId: 7 },
    });
  });

  it('rejects a close transition whose generated blank id conflicts with a sidecar', () => {
    const workspace = ensureSidecarTab(createEditorWorkspaceState(tab()), {
      id: 'tab-sidecar',
      name: 'Right Editor',
      languageId: 'json' as any,
      sourceText: '{"right":true}',
    });

    const result = closeWorkspaceTabTransition(workspace, 'tab-primary', {
      id: 'tab-sidecar',
      name: 'Fallback',
      documentKey: 'tab-sidecar:0',
      languageId: 'yaml' as any,
    });

    expect(result).toBeNull();
  });

  it('rejects a close transition whose generated blank id already exists', () => {
    const workspace = createEditorWorkspaceState(tab());

    const result = closeWorkspaceTabTransition(workspace, 'tab-primary', {
      id: 'tab-primary',
      name: 'Fallback',
      documentKey: 'tab-primary:next',
      languageId: 'yaml' as any,
    });

    expect(result).toBeNull();
  });

  it('syncs editor tab fields without mutating sidecar state', () => {
    const workspace = ensureSidecarTab(createEditorWorkspaceState(tab()), {
      id: 'tab-sidecar',
      name: 'Right Editor',
      languageId: 'json' as any,
      sourceText: '{"right":true}',
    });

    const next = syncWorkspaceEditorTab(
      workspace,
      {
        id: 'tab-primary',
        name: 'Renamed',
        documentKey: 'tab-primary:9',
        languageId: 'toml' as any,
        sourceText: 'name = "primary"\n',
        revision: 9,
        graphAppliedRevision: 8,
        snapshotId: 77,
        tempModel: tab().tempModel,
        fullEditUiState: tab().fullEditUiState,
      },
      'primary',
    );

    expect(next.tabsById['tab-primary']).toMatchObject({
      name: 'Renamed',
      role: 'primary',
      documentKey: 'tab-primary:9',
      languageId: 'toml',
      sourceText: 'name = "primary"\n',
      revision: 9,
      graphAppliedRevision: 8,
      snapshotId: 77,
    });
    expect(next.tabsById['tab-sidecar']).toMatchObject({
      role: 'sidecar',
      sourceText: '{"right":true}',
    });
  });

  it('keeps the active primary tab primary when syncing it as background', () => {
    const workspace = addWorkspaceTab(createEditorWorkspaceState(tab()), {
      id: 'tab-second',
      name: 'Untitled 2',
      documentKey: 'tab-second:0',
      languageId: 'yaml' as any,
      sourceText: 'name: second\n',
    });

    const next = syncWorkspaceEditorTab(
      workspace,
      {
        id: workspace.primaryTabId,
        name: 'Primary Synced',
        documentKey: 'tab-primary:2',
        languageId: 'json' as any,
        sourceText: '{"synced":true}',
        revision: 2,
      },
      'background',
    );

    expect(next.primaryTabId).toBe(workspace.primaryTabId);
    expect(next.activeTabId).toBe(workspace.activeTabId);
    expect(next.tabsById[workspace.primaryTabId].role).toBe('primary');
    expect(next.tabsById['tab-second'].role).toBe('background');
  });

  it('syncs an inactive background tab as the only primary left tab', () => {
    const workspace = ensureSidecarTab(
      addWorkspaceTab(createEditorWorkspaceState(tab()), {
        id: 'tab-second',
        name: 'Untitled 2',
        documentKey: 'tab-second:0',
        languageId: 'yaml' as any,
        sourceText: 'name: second\n',
      }),
      {
        id: 'tab-sidecar',
        name: 'Right Editor',
        languageId: 'json' as any,
        sourceText: '{"right":true}',
      },
    );

    const next = syncWorkspaceEditorTab(
      workspace,
      {
        id: 'tab-second',
        name: 'Second Synced',
        documentKey: 'tab-second:4',
        languageId: 'yaml' as any,
        sourceText: 'name: synced\n',
        revision: 4,
      },
      'primary',
    );

    expect(next.primaryTabId).toBe('tab-second');
    expect(next.activeTabId).toBe('tab-second');
    expect(next.paneTabIds.left).toBe('tab-second');
    expect(next.tabsById['tab-second']).toMatchObject({
      role: 'primary',
      name: 'Second Synced',
      documentKey: 'tab-second:4',
      sourceText: 'name: synced\n',
      revision: 4,
    });
    expect(next.tabsById['tab-primary'].role).toBe('background');
    expect(next.tabsById['tab-sidecar'].role).toBe('sidecar');
    expect(next.paneTabIds.right).toBe('tab-sidecar');
    expect(next.tabOrder).toEqual(['tab-primary', 'tab-second']);
  });

  it('deep clones nested primary tempModel state when creating a workspace', () => {
    const primary = tab({
      tempModel: {
        diffInputText: 'diff',
        scratchText: 'scratch',
        commandQuery: 'cmd',
        status: 'Ready',
        error: '',
        cursor: 'Ln 2, Col 3',
        selectionLength: 4,
        treePath: [keySeg('outer')],
        graphHighlight: {
          path: [keySeg('inner')],
          revision: 1,
          source: 'editor',
          target: 'node',
        },
        diagnostics: [
          {
            code: 'syntax-error',
            message: 'bad',
            startLineNumber: 1,
            startColumn: 1,
            endLineNumber: 1,
            endColumn: 2,
            context: [{ lineNumber: 1, text: 'line-1' }],
          },
        ],
      },
    });
    const workspace = createEditorWorkspaceState(primary);

    primary.tempModel.treePath[0].key = 'mutated';
    if (primary.tempModel.graphHighlight) primary.tempModel.graphHighlight.path[0].key = 'mutated';
    primary.tempModel.diagnostics[0].context[0].text = 'changed';

    expect(workspace.tabsById['tab-primary'].tempModel.treePath[0].key).toBe('outer');
    expect(workspace.tabsById['tab-primary'].tempModel.graphHighlight?.path[0].key).toBe('inner');
    expect(workspace.tabsById['tab-primary'].tempModel.diagnostics[0].context[0].text).toBe('line-1');
  });

  it('creates a sidecar tab without changing the primary tab', () => {
    const workspace = createEditorWorkspaceState(tab());
    const next = ensureSidecarTab(workspace, {
      id: 'tab-sidecar',
      name: 'Right Editor',
      languageId: 'yaml' as any,
      sourceText: 'a: 1\n',
    });

    expect(next.primaryTabId).toBe('tab-primary');
    expect(next.paneTabIds.left).toBe('tab-primary');
    expect(next.paneTabIds.right).toBe('tab-sidecar');
    expect(next.tabsById['tab-primary'].sourceText).toBe('{"a":1}');
    expect(next.tabsById['tab-sidecar']).toMatchObject({
      id: 'tab-sidecar',
      role: 'sidecar',
      documentKey: 'sidecar:tab-sidecar:0',
      languageId: 'yaml',
      sourceText: 'a: 1\n',
      revision: 0,
      graphAppliedRevision: 0,
      snapshotId: null,
    });
  });

  it('reinitializes the primary workspace without dropping sidecar tabs or their bindings', () => {
    const workspace = ensureSidecarTab(createEditorWorkspaceState(tab()), {
      id: 'tab-sidecar',
      name: 'Right Editor',
      languageId: 'yaml' as any,
      sourceText: 'a: 1\n',
    });
    const withBinding = {
      ...updateWorkspaceTab(workspace, 'tab-sidecar', {
        revision: 3,
        snapshotId: 12,
      }),
      snapshotBindingsByDocumentKey: {
        'tab-primary:0': { documentKey: 'tab-primary:0', revision: 1, snapshotId: 7 },
        'sidecar:tab-sidecar:0': { documentKey: 'sidecar:tab-sidecar:0', revision: 3, snapshotId: 12 },
        stale: { documentKey: 'stale', revision: 9, snapshotId: 99 },
      },
    };

    const next = reinitializeWorkspaceFromPrimaryTab(
      withBinding,
      tab({
        id: 'tab-bootstrap',
        name: 'Untitled bootstrap',
        documentKey: 'tab-bootstrap:0',
        sourceText: '{"boot":true}',
        revision: 2,
        snapshotId: 20,
      }),
    );

    expect(next.primaryTabId).toBe('tab-bootstrap');
    expect(next.activeTabId).toBe('tab-bootstrap');
    expect(next.paneTabIds.left).toBe('tab-bootstrap');
    expect(next.paneTabIds.right).toBe('tab-sidecar');
    expect(next.tabOrder).toEqual(['tab-bootstrap']);
    expect(next.tabsById['tab-sidecar']).toMatchObject({
      role: 'sidecar',
      languageId: 'yaml',
      sourceText: 'a: 1\n',
      revision: 3,
      snapshotId: 12,
    });
    expect(next.snapshotBindingsByDocumentKey).toEqual({
      'sidecar:tab-sidecar:0': { documentKey: 'sidecar:tab-sidecar:0', revision: 3, snapshotId: 12 },
    });
  });

  it('updates one tab without mutating the other tab', () => {
    const workspace = ensureSidecarTab(createEditorWorkspaceState(tab()), {
      id: 'tab-sidecar',
      name: 'Right Editor',
      languageId: 'json' as any,
      sourceText: '{}',
    });

    const next = updateWorkspaceTab(workspace, 'tab-sidecar', {
      sourceText: '{"right":true}',
      revision: 3,
      snapshotId: 12,
    });

    expect(next.tabsById['tab-sidecar']).toMatchObject({
      sourceText: '{"right":true}',
      revision: 3,
      snapshotId: 12,
    });
    expect(next.tabsById['tab-primary']).toMatchObject({
      sourceText: '{"a":1}',
      revision: 1,
      snapshotId: 7,
    });
  });

  it('allows snapshotId to be cleared with null', () => {
    const workspace = ensureSidecarTab(createEditorWorkspaceState(tab()), {
      id: 'tab-sidecar',
      name: 'Right Editor',
      languageId: 'json' as any,
      sourceText: '{}',
    });

    const next = updateWorkspaceTab(workspace, 'tab-sidecar', { snapshotId: null });

    expect(next.tabsById['tab-sidecar'].snapshotId).toBeNull();
  });

  it('ignores undefined snapshotId patches', () => {
    const workspace = ensureSidecarTab(createEditorWorkspaceState(tab()), {
      id: 'tab-sidecar',
      name: 'Right Editor',
      languageId: 'json' as any,
      sourceText: '{}',
    });

    const withSnapshot = updateWorkspaceTab(workspace, 'tab-sidecar', { snapshotId: 12 });
    const next = updateWorkspaceTab(withSnapshot, 'tab-sidecar', { snapshotId: undefined } as any);

    expect(next.tabsById['tab-sidecar'].snapshotId).toBe(12);
  });

  it('ignores runtime attempts to overwrite id role or documentKey', () => {
    const workspace = ensureSidecarTab(createEditorWorkspaceState(tab()), {
      id: 'tab-sidecar',
      name: 'Right Editor',
      languageId: 'json' as any,
      sourceText: '{}',
    });

    const unsafePatch = {
      id: 'evil',
      role: 'background',
      documentKey: 'evil-doc',
      sourceText: 'ok',
    } as any;

    const next = updateWorkspaceTab(workspace, 'tab-sidecar', unsafePatch);

    expect(next.tabsById['tab-sidecar']).toMatchObject({
      id: 'tab-sidecar',
      role: 'sidecar',
      documentKey: 'sidecar:tab-sidecar:0',
      sourceText: 'ok',
    });
  });

  it('syncs sidecar language from primary while preserving sidecar text and revision', () => {
    const workspace = ensureSidecarTab(createEditorWorkspaceState(tab({ languageId: 'toml' as any })), {
      id: 'tab-sidecar',
      name: 'Right Editor',
      languageId: 'json' as any,
      sourceText: '{"right":true}',
    });

    const next = syncSidecarLanguageFromPrimary(workspace);

    expect(next.tabsById['tab-sidecar']).toMatchObject({
      languageId: 'toml',
      sourceText: '{"right":true}',
      revision: 0,
    });
  });

  it('creates a clean sidecar tempModel without inheriting primary ui noise', () => {
    const workspace = ensureSidecarTab(
      createEditorWorkspaceState(
        tab({
          fullEditUiState: {
            active: true,
            sessionId: 'session-1',
            ownerKey: 'owner-1',
            documentKey: 'primary-doc',
            revision: 99,
            streamSeq: 5,
            inputByteLength: 11,
            modelVersionId: 3,
            byteLength: 22,
            language: 'yaml',
            phase: 'streaming',
            sessionKind: 'full-edit',
            transportKind: 'memory',
            reason: 'language-switch',
          },
          tempModel: {
            diffInputText: 'primary-diff',
            scratchText: 'primary-scratch',
            commandQuery: 'primary-cmd',
            status: 'Error',
            error: 'boom',
            cursor: 'Ln 9, Col 9',
            selectionLength: 99,
            treePath: [{ kind: 'key', value: 'primary' } as any],
            graphHighlight: {
              path: [{ kind: 'key', value: 'highlight' } as any],
              revision: 8,
              source: 'graph',
              target: 'key',
            },
            diagnostics: [
              {
                code: 'missing-node',
                message: 'missing',
                startLineNumber: 3,
                startColumn: 4,
                endLineNumber: 3,
                endColumn: 8,
                context: [{ lineNumber: 3, text: 'primary-context' }],
              },
            ],
          },
        }),
      ),
      {
        id: 'tab-sidecar',
        name: 'Right Editor',
        languageId: 'json' as any,
        sourceText: '{"right":true}',
      },
    );

    expect(workspace.tabsById['tab-sidecar'].tempModel).toMatchObject({
      diffInputText: '',
      scratchText: '{"right":true}',
      commandQuery: '',
      status: 'Ready',
      error: '',
      cursor: 'Ln 1, Col 1',
      selectionLength: 0,
      treePath: [],
      graphHighlight: null,
      diagnostics: [],
    });
    expect(workspace.tabsById['tab-sidecar'].fullEditUiState).toEqual({
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
    });
  });

  it('deep clones patch tempModel so later patch mutation does not leak into workspace', () => {
    const workspace = ensureSidecarTab(createEditorWorkspaceState(tab()), {
      id: 'tab-sidecar',
      name: 'Right Editor',
      languageId: 'json' as any,
      sourceText: '{}',
    });
    const patch: { tempModel: EditorWorkspaceTab['tempModel'] } = {
      tempModel: {
        diffInputText: 'diff',
        scratchText: 'scratch',
        commandQuery: 'cmd',
        status: 'Loading',
        error: '',
        cursor: 'Ln 4, Col 5',
        selectionLength: 2,
        treePath: [keySeg('root')],
        graphHighlight: {
          path: [keySeg('highlight')],
          revision: 3,
          source: 'editor',
          target: 'node',
        },
        diagnostics: [
          {
            code: 'syntax-error',
            message: 'bad',
            startLineNumber: 1,
            startColumn: 1,
            endLineNumber: 1,
            endColumn: 2,
            context: [{ lineNumber: 1, text: 'ctx' }],
          },
        ],
      },
    };

    const next = updateWorkspaceTab(workspace, 'tab-sidecar', patch);

    patch.tempModel.treePath[0].key = 'mutated';
    if (patch.tempModel.graphHighlight) patch.tempModel.graphHighlight.path[0].key = 'mutated';
    patch.tempModel.diagnostics[0].context[0].text = 'changed';

    expect(next.tabsById['tab-sidecar'].tempModel.treePath[0].key).toBe('root');
    expect(next.tabsById['tab-sidecar'].tempModel.graphHighlight?.path[0].key).toBe('highlight');
    expect(next.tabsById['tab-sidecar'].tempModel.diagnostics[0].context[0].text).toBe('ctx');
  });

  it('deep clones patch fullEditUiState so later patch mutation does not leak into workspace', () => {
    const workspace = ensureSidecarTab(createEditorWorkspaceState(tab()), {
      id: 'tab-sidecar',
      name: 'Right Editor',
      languageId: 'json' as any,
      sourceText: '{}',
    });
    const patch: { fullEditUiState: EditorWorkspaceTab['fullEditUiState'] } = {
      fullEditUiState: {
        active: true,
        sessionId: 'session-2',
        ownerKey: 'owner-2',
        documentKey: 'sidecar-doc',
        revision: 4,
        streamSeq: 7,
        inputByteLength: 18,
        modelVersionId: 5,
        byteLength: 44,
        language: 'json' as any,
        phase: 'streaming',
        sessionKind: 'full-edit' as const,
        transportKind: 'memory' as const,
        reason: 'tab-reactivate' as const,
      },
    };

    const next = updateWorkspaceTab(workspace, 'tab-sidecar', patch);
    patch.fullEditUiState.active = false;
    patch.fullEditUiState.sessionId = 'mutated';
    patch.fullEditUiState.revision = 123;

    expect(next.tabsById['tab-sidecar'].fullEditUiState).toEqual({
      active: true,
      sessionId: 'session-2',
      ownerKey: 'owner-2',
      documentKey: 'sidecar-doc',
      revision: 4,
      streamSeq: 7,
      inputByteLength: 18,
      modelVersionId: 5,
      byteLength: 44,
      language: 'json',
      phase: 'streaming',
      sessionKind: 'full-edit',
      transportKind: 'memory',
      reason: 'tab-reactivate',
    });
  });
});
