import {
  activateWorkspaceTab,
  addWorkspaceTab,
  createEditorWorkspaceState,
  type EditorWorkspaceState,
  type EditorWorkspaceTab,
} from '../store/editor-workspace';
import type { WorkspaceSession } from '../workspace-host';
import { validateWorkspaceSession, workspaceTabInputFromSession } from '../workspace-host/workspace-session';

export type SharedWorkspacePromoteTopologyResult =
  | { kind: 'promoted'; workspace: EditorWorkspaceState }
  | {
      kind: 'rejected';
      reason: 'invalid-session' | 'missing-share-tab' | 'stale-share-target' | 'not-ephemeral-workspace';
      detail?: string;
    };

export type SharedWorkspacePromoteTarget = {
  tabId: string;
  documentKey: string;
  sourceText: string;
};

function cloneTab(tab: EditorWorkspaceTab): EditorWorkspaceTab {
  return {
    ...tab,
    tempModel: {
      ...tab.tempModel,
      treePath: tab.tempModel.treePath.map((segment) => ({ ...segment })),
      graphHighlight: tab.tempModel.graphHighlight
        ? { ...tab.tempModel.graphHighlight, path: tab.tempModel.graphHighlight.path.map((segment) => ({ ...segment })) }
        : null,
      diagnostics: tab.tempModel.diagnostics.map((diagnostic) => ({
        ...diagnostic,
        context: diagnostic.context.map((line) => ({ ...line })),
      })),
    },
    fullEditUiState: { ...tab.fullEditUiState },
  };
}

function recoveredTabId(index: number, reservedIds: Set<string>): string {
  const base = `session-tab-${index}`;
  if (!reservedIds.has(base)) return base;
  let suffix = 1;
  while (reservedIds.has(`${base}-${suffix}`)) suffix += 1;
  return `${base}-${suffix}`;
}

function restorePersistedTabs(session: WorkspaceSession, reservedIds: Set<string>): EditorWorkspaceState | null {
  if (session.tabs.length === 0) return null;
  const ids: string[] = [];
  for (let index = 0; index < session.tabs.length; index += 1) {
    const id = recoveredTabId(index, reservedIds);
    reservedIds.add(id);
    ids.push(id);
  }
  let workspace = createEditorWorkspaceState({
    ...workspaceTabInputFromSession(session.tabs[0], ids[0]),
    role: 'primary',
    revision: 0,
    graphAppliedRevision: 0,
    snapshotId: null,
    tempModel: {
      diffInputText: '',
      scratchText: session.tabs[0].sourceText,
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
  });
  for (let index = 1; index < session.tabs.length; index += 1) {
    workspace = addWorkspaceTab(workspace, workspaceTabInputFromSession(session.tabs[index], ids[index]));
  }
  return workspace;
}

export function promoteSharedWorkspaceTopology(input: {
  workspace: EditorWorkspaceState;
  persistedSession: unknown;
  target: SharedWorkspacePromoteTarget;
}): SharedWorkspacePromoteTopologyResult {
  const shareTab = input.workspace.tabsById[input.target.tabId];
  if (!shareTab || shareTab.role === 'sidecar') return { kind: 'rejected', reason: 'missing-share-tab' };
  if (shareTab.documentKey !== input.target.documentKey) return { kind: 'rejected', reason: 'stale-share-target' };
  if (input.workspace.tabOrder.length !== 1 || input.workspace.tabOrder[0] !== input.target.tabId) {
    return { kind: 'rejected', reason: 'not-ephemeral-workspace' };
  }

  const validation = input.persistedSession == null
    ? { kind: 'valid' as const, session: { version: 1 as const, activeTabIndex: 0, tabs: [] } }
    : validateWorkspaceSession(input.persistedSession);
  if (validation.kind === 'invalid') {
    return { kind: 'rejected', reason: 'invalid-session', detail: validation.reason };
  }

  const sidecars = Object.values(input.workspace.tabsById).filter((tab) => tab.role === 'sidecar');
  const reservedIds = new Set(Object.keys(input.workspace.tabsById));
  const persistedWorkspace = restorePersistedTabs(validation.session, reservedIds);
  const promotedShareTab = cloneTab({ ...shareTab, sourceText: input.target.sourceText });
  let merged = persistedWorkspace
    ? addWorkspaceTab(persistedWorkspace, promotedShareTab)
    : createEditorWorkspaceState({ ...promotedShareTab, role: 'primary' });
  merged = activateWorkspaceTab(merged, promotedShareTab.id);

  const tabsById = { ...merged.tabsById };
  for (const sidecar of sidecars) tabsById[sidecar.id] = cloneTab(sidecar);
  const retainedDocumentKeys = new Set(Object.values(tabsById).map((tab) => tab.documentKey));
  const snapshotBindingsByDocumentKey = Object.fromEntries(
    Object.entries(input.workspace.snapshotBindingsByDocumentKey)
      .filter(([documentKey]) => retainedDocumentKeys.has(documentKey))
      .map(([documentKey, binding]) => [documentKey, { ...binding }]),
  );

  return {
    kind: 'promoted',
    workspace: {
      ...merged,
      tabsById,
      paneTabIds: {
        left: promotedShareTab.id,
        right: input.workspace.paneTabIds.right && tabsById[input.workspace.paneTabIds.right]?.role === 'sidecar'
          ? input.workspace.paneTabIds.right
          : null,
      },
      snapshotBindingsByDocumentKey,
    },
  };
}
