import type { SnapshotId } from '@core-wasm/index';
import { derived, get, type Readable, type Writable } from 'svelte/store';
import {
  activeDocumentAuthorityStore,
  bindAuthoritySnapshot,
  clearAuthoritySnapshot,
  getAuthorityWorkspaceState,
  getAuthorityDocumentSessionState,
  patchAuthorityActiveDocument,
  resetActiveDocumentAuthority,
  setAuthorityWorkspaceState,
} from './active-document-authority';
import {
  activateWorkspaceTab,
  addWorkspaceTab,
  closeWorkspaceTab,
  createEditorWorkspaceState,
  ensureDetachedSidecarTab,
  ensureSidecarTab,
  reinitializeWorkspaceFromPrimaryTab,
  removeDetachedSidecarTab,
  summarizeWorkspaceTabs,
  syncSidecarLanguageFromPrimary,
  syncWorkspaceEditorTab,
  updateWorkspaceTab as patchWorkspaceTab,
  type EditorWorkspaceState,
  type EditorWorkspaceTab,
  type EditorWorkspaceTabPatch,
  type EditorWorkspaceTabSummary,
  type WorkspaceEditorTabInput,
} from './editor-workspace';

type WorkspaceCoordinator = {
  onWorkspaceChange?: (next: EditorWorkspaceState, previous: EditorWorkspaceState) => void;
};

let workspaceCoordinator: WorkspaceCoordinator | null = null;

function createWorkspacePrimaryTab(payload: { id: string; name: string }): EditorWorkspaceTab {
  const current = getAuthorityWorkspaceState();
  const active = current.tabsById[current.activeTabId] ?? current.tabsById[current.primaryTabId];
  return {
    ...active,
    id: payload.id,
    role: 'primary',
    name: payload.name,
    snapshotId: null,
  };
}

export const initialWorkspaceState: EditorWorkspaceState = createEditorWorkspaceState(
  createWorkspacePrimaryTab({ id: 'primary', name: 'Primary' }),
);

const authorityWorkspaceStore = derived(activeDocumentAuthorityStore, ($authority) => $authority.workspace);

/** Workspace Store is an Adapter over Active Document authority. */
export const workspaceStore: Writable<EditorWorkspaceState> = {
  subscribe: authorityWorkspaceStore.subscribe,
  set: setAuthorityWorkspaceState,
  update: (updater) => setAuthorityWorkspaceState(updater(getAuthorityWorkspaceState())),
};

function deepFreezeForRead<T>(value: T): T {
  if (!value || typeof value !== 'object') return value;
  if (Object.isFrozen(value)) return value;
  Object.freeze(value);
  if (Array.isArray(value)) {
    for (const item of value) deepFreezeForRead(item);
    return value;
  }
  for (const nestedValue of Object.values(value as Record<string, unknown>)) {
    deepFreezeForRead(nestedValue);
  }
  return value;
}

export function cloneWorkspaceStateForWrite(workspace: EditorWorkspaceState): EditorWorkspaceState {
  return {
    primaryTabId: workspace.primaryTabId,
    activeTabId: workspace.activeTabId,
    tabOrder: [...workspace.tabOrder],
    paneTabIds: {
      ...workspace.paneTabIds,
    },
    snapshotBindingsByDocumentKey: Object.fromEntries(
      Object.entries(workspace.snapshotBindingsByDocumentKey).map(([documentKey, binding]) => [
        documentKey,
        { ...binding },
      ]),
    ),
    tabsById: Object.fromEntries(
      Object.entries(workspace.tabsById).map(([tabId, tab]) => [
        tabId,
        {
          ...tab,
          tempModel: {
            ...tab.tempModel,
            treePath: tab.tempModel.treePath.map((segment) => ({ ...segment })),
            graphHighlight: tab.tempModel.graphHighlight
              ? {
                  ...tab.tempModel.graphHighlight,
                  path: tab.tempModel.graphHighlight.path.map((segment) => ({ ...segment })),
                }
              : null,
            diagnostics: tab.tempModel.diagnostics.map((diagnostic) => ({
              ...diagnostic,
              context: diagnostic.context.map((line) => ({ ...line })),
            })),
          },
          fullEditUiState: { ...tab.fullEditUiState },
        },
      ]),
    ),
  };
}

let memoizedRawWorkspace: EditorWorkspaceState | null = null;
let memoizedReadWorkspace: EditorWorkspaceState | null = null;

export function cloneWorkspaceStateForRead(workspace: EditorWorkspaceState): EditorWorkspaceState {
  if (memoizedRawWorkspace === workspace && memoizedReadWorkspace) return memoizedReadWorkspace;
  const snapshot = deepFreezeForRead(cloneWorkspaceStateForWrite(workspace));
  memoizedRawWorkspace = workspace;
  memoizedReadWorkspace = snapshot;
  return snapshot;
}

export function getWorkspaceState(): EditorWorkspaceState {
  return cloneWorkspaceStateForRead(get(workspaceStore));
}

export function getWorkspaceRawState(): EditorWorkspaceState {
  return getAuthorityWorkspaceState();
}

export function setWorkspaceState(state: EditorWorkspaceState): void {
  const previous = getAuthorityWorkspaceState();
  if (previous === state) return;
  setAuthorityWorkspaceState(state);
  workspaceCoordinator?.onWorkspaceChange?.(state, previous);
}

/** Workspace may notify non-authoritative View Runtime adapters only. */
export function registerWorkspaceCoordinator(coordinator: WorkspaceCoordinator | null): void {
  workspaceCoordinator = coordinator;
}

export function getWorkspaceTab(tabId: string) {
  return getWorkspaceState().tabsById[tabId] ?? null;
}

export function getWorkspaceSnapshotId(documentKey: string): SnapshotId | null {
  if (!documentKey) return null;
  return getAuthorityWorkspaceState().snapshotBindingsByDocumentKey[documentKey]?.snapshotId ?? null;
}

export function initWorkspaceFromPrimaryTab(payload: { id: string; name: string }): void {
  const next = reinitializeWorkspaceFromPrimaryTab(get(workspaceStore), createWorkspacePrimaryTab(payload));
  setWorkspaceState(next);
}

export function addWorkspaceTabFromEditor(payload: WorkspaceEditorTabInput): void {
  setWorkspaceState(addWorkspaceTab(get(workspaceStore), payload));
}

export function activateWorkspaceTabFromEditor(payload: WorkspaceEditorTabInput): void {
  const withSyncedTab = syncWorkspaceEditorTab(get(workspaceStore), payload, 'primary');
  setWorkspaceState(syncSidecarLanguageFromPrimary(activateWorkspaceTab(withSyncedTab, payload.id)));
}

export function closeWorkspaceTabFromEditor(tabId: string, fallback?: WorkspaceEditorTabInput): void {
  const result = closeWorkspaceTab(get(workspaceStore), tabId, fallback);
  setWorkspaceState(syncSidecarLanguageFromPrimary(result.workspace));
}

export function getWorkspaceTabSummaries(): EditorWorkspaceTabSummary[] {
  return summarizeWorkspaceTabs(get(workspaceStore));
}

export function ensureSidecarWorkspaceTab(payload: { id: string; name: string; sourceText: string }): void {
  const session = getAuthorityDocumentSessionState();
  setWorkspaceState(
    ensureSidecarTab(get(workspaceStore), {
      id: payload.id,
      name: payload.name,
      languageId: session.languageId,
      sourceText: payload.sourceText,
    }),
  );
}

export function ensureDetachedSidecarWorkspaceTab(payload: { id: string; name: string; sourceText: string }): void {
  const session = getAuthorityDocumentSessionState();
  setWorkspaceState(
    ensureDetachedSidecarTab(get(workspaceStore), {
      id: payload.id,
      name: payload.name,
      languageId: session.languageId,
      sourceText: payload.sourceText,
    }),
  );
}

export function removeDetachedSidecarWorkspaceTab(tabId: string): void {
  setWorkspaceState(removeDetachedSidecarTab(get(workspaceStore), tabId));
}

export function updateWorkspaceTab(tabId: string, patch: EditorWorkspaceTabPatch): void {
  const workspace = get(workspaceStore);
  if (tabId === workspace.primaryTabId && patch.documentKey !== undefined) {
    patchAuthorityActiveDocument({ documentKey: patch.documentKey });
    return;
  }
  if (tabId === workspace.primaryTabId) return;
  const isSidecarTab = workspace.tabsById[tabId]?.role === 'sidecar';
  const { languageId: _ignoredLanguageId, ...patchWithoutLanguage } = patch;
  setWorkspaceState(patchWorkspaceTab(workspace, tabId, isSidecarTab ? patch : patchWithoutLanguage));
}

export function bindWorkspaceSnapshot(payload: {
  documentKey: string;
  revision: number;
  snapshotId: SnapshotId | null | undefined;
}): void {
  bindAuthoritySnapshot(payload);
}

export function clearWorkspaceSnapshotBinding(documentKey: string, snapshotId?: SnapshotId | null): void {
  clearAuthoritySnapshot(documentKey, snapshotId);
}

export function syncSidecarWorkspaceLanguageFromPrimary(): void {
  setWorkspaceState(syncSidecarLanguageFromPrimary(get(workspaceStore)));
}

export function resetWorkspaceState(): void {
  resetActiveDocumentAuthority();
}

export const editorWorkspace: Readable<EditorWorkspaceState> = {
  subscribe: (run) => {
    let initialized = false;
    let currentRaw: EditorWorkspaceState | undefined;
    return workspaceStore.subscribe(($workspace) => {
      if (initialized && Object.is($workspace, currentRaw)) return;
      initialized = true;
      currentRaw = $workspace;
      run(getWorkspaceState());
    });
  },
};
