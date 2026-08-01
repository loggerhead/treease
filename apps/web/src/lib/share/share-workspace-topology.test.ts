import { describe, expect, it } from 'vitest';

import { initialFullEditUiState } from '../store/full-edit-ui-state';
import { initialTempModel } from '../store/graph-selection-store';
import { createEditorWorkspaceState, ensureSidecarTab } from '../store/editor-workspace';
import { workspaceSessionFromWorkspace } from '../workspace-host/workspace-session';
import { promoteSharedWorkspaceTopology } from './share-workspace-topology';

function ephemeralWorkspace() {
  return createEditorWorkspaceState({
    id: 'share-tab',
    role: 'primary',
    name: 'Shared document',
    documentKey: 'share-tab:0',
    languageId: 'json',
    sourceText: '{"before":true}',
    origin: 'import',
    revision: 2,
    graphAppliedRevision: 2,
    snapshotId: 9,
    tempModel: initialTempModel,
    fullEditUiState: initialFullEditUiState,
  });
}

describe('share workspace topology promotion', () => {
  it('preserves persisted tab order and appends the current share tab once', () => {
    const result = promoteSharedWorkspaceTopology({
      workspace: ephemeralWorkspace(),
      persistedSession: {
        version: 1,
        activeTabIndex: 0,
        tabs: [
          { name: 'One', languageId: 'json', sourceText: '{"one":1}', origin: 'user', savedText: '{"one":0}' },
          { name: 'Two', languageId: 'yaml', sourceText: 'two: 2\n', origin: 'import' },
        ],
      },
      target: { tabId: 'share-tab', documentKey: 'share-tab:0', sourceText: '{"after":true}' },
    });

    expect(result.kind).toBe('promoted');
    if (result.kind !== 'promoted') return;
    expect(result.workspace.tabOrder).toEqual(['session-tab-0', 'session-tab-1', 'share-tab']);
    expect(result.workspace.activeTabId).toBe('share-tab');
    expect(result.workspace.primaryTabId).toBe('share-tab');
    expect(result.workspace.paneTabIds.left).toBe('share-tab');
    expect(result.workspace.tabsById['share-tab']).toMatchObject({
      documentKey: 'share-tab:0',
      sourceText: '{"after":true}',
      role: 'primary',
    });
    expect(result.workspace.tabsById['session-tab-0']).toMatchObject({
      name: 'One',
      sourceText: '{"one":1}',
      savedText: '{"one":0}',
      role: 'background',
      fullEditUiState: { phase: 'idle' },
    });
    expect(workspaceSessionFromWorkspace(result.workspace).tabs.map((tab) => tab.name)).toEqual([
      'One',
      'Two',
      'Shared document',
    ]);
  });

  it('keeps the runtime sidecar out of tab order and session projection', () => {
    const workspace = ensureSidecarTab(ephemeralWorkspace(), {
      id: 'share-sidecar',
      name: 'Preview',
      languageId: 'json',
      sourceText: '{"preview":true}',
    });
    const result = promoteSharedWorkspaceTopology({
      workspace,
      persistedSession: null,
      target: { tabId: 'share-tab', documentKey: 'share-tab:0', sourceText: '{"after":true}' },
    });

    expect(result.kind).toBe('promoted');
    if (result.kind !== 'promoted') return;
    expect(result.workspace.paneTabIds.right).toBe('share-sidecar');
    expect(result.workspace.tabOrder).toEqual(['share-tab']);
    expect(workspaceSessionFromWorkspace(result.workspace).tabs).toHaveLength(1);
  });

  it('rejects invalid persisted data instead of replacing it with an empty session', () => {
    const result = promoteSharedWorkspaceTopology({
      workspace: ephemeralWorkspace(),
      persistedSession: { version: 1, activeTabIndex: 0, tabs: [{ name: 'Broken' }] },
      target: { tabId: 'share-tab', documentKey: 'share-tab:0', sourceText: '{}' },
    });

    expect(result).toMatchObject({ kind: 'rejected', reason: 'invalid-session' });
  });

  it('rejects a stale target and a workspace that is no longer ephemeral', () => {
    const workspace = ephemeralWorkspace();
    expect(promoteSharedWorkspaceTopology({
      workspace,
      persistedSession: null,
      target: { tabId: 'share-tab', documentKey: 'stale', sourceText: '{}' },
    })).toEqual({ kind: 'rejected', reason: 'stale-share-target' });

    const alreadyPromoted = promoteSharedWorkspaceTopology({
      workspace,
      persistedSession: { version: 1, activeTabIndex: 0, tabs: [{ name: 'Old', languageId: 'json', sourceText: '{}' }] },
      target: { tabId: 'share-tab', documentKey: 'share-tab:0', sourceText: '{}' },
    });
    expect(alreadyPromoted.kind).toBe('promoted');
    if (alreadyPromoted.kind !== 'promoted') return;
    expect(promoteSharedWorkspaceTopology({
      workspace: alreadyPromoted.workspace,
      persistedSession: null,
      target: { tabId: 'share-tab', documentKey: 'share-tab:0', sourceText: '{}' },
    })).toEqual({ kind: 'rejected', reason: 'not-ephemeral-workspace' });
  });
});
