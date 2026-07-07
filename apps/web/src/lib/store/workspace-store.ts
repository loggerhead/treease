import type { SnapshotId } from '@core-wasm/index';
import { get, writable, type Readable } from 'svelte/store';
import { editorLanguageFallback } from '../monaco/language-support';
import { getDocumentSessionState, initialDocumentSessionState } from './document-session-store';
import { getFullEditUiStateSnapshot, initialFullEditUiState } from './full-edit-ui-store';
import { getActiveTempModelSnapshot, initialTempModel } from './graph-selection-store';
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
  const session = getDocumentSessionState();
  return {
    id: payload.id,
    role: 'primary',
    name: payload.name,
    documentKey: session.documentKey,
    languageId: session.languageId || editorLanguageFallback,
    sourceText: session.sourceText,
    revision: session.editorRevision,
    graphAppliedRevision: session.graphAppliedRevision,
    snapshotId: null,
    tempModel: getActiveTempModelSnapshot(),
    fullEditUiState: getFullEditUiStateSnapshot(),
  };
}

export const initialWorkspaceState: EditorWorkspaceState = createEditorWorkspaceState(
  createWorkspacePrimaryTab({ id: 'primary', name: 'Primary' }),
);

export const workspaceStore = writable<EditorWorkspaceState>(initialWorkspaceState);

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
  return get(workspaceStore);
}

export function setWorkspaceState(state: EditorWorkspaceState): void {
  const previous = get(workspaceStore);
  if (previous === state) return;
  workspaceStore.set(state);
  workspaceCoordinator?.onWorkspaceChange?.(state, previous);
}

export function registerWorkspaceCoordinator(coordinator: WorkspaceCoordinator | null): void {
  workspaceCoordinator = coordinator;
}

export function getWorkspaceTab(tabId: string) {
  return getWorkspaceState().tabsById[tabId] ?? null;
}

export function getWorkspaceSnapshotId(documentKey: string): SnapshotId | null {
  if (!documentKey) return null;
  return get(workspaceStore).snapshotBindingsByDocumentKey[documentKey]?.snapshotId ?? null;
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
  const session = getDocumentSessionState();
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
  const session = getDocumentSessionState();
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
  const workspace = get(workspaceStore);
  if (!payload.documentKey || payload.snapshotId == null) return;
  const current = workspace.snapshotBindingsByDocumentKey[payload.documentKey];
  const newestTabRevision = Object.values(workspace.tabsById).reduce(
    (newest, tab) => (tab.documentKey === payload.documentKey ? Math.max(newest, tab.revision) : newest),
    -1,
  );
  if (current && payload.revision < current.revision) return;
  if (newestTabRevision >= 0 && payload.revision < newestTabRevision) return;
  let tabsById = workspace.tabsById;
  for (const [tabId, tab] of Object.entries(workspace.tabsById)) {
    if (tab.documentKey !== payload.documentKey) continue;
    if (payload.revision < tab.revision) continue;
    if (tabsById === workspace.tabsById) tabsById = { ...tabsById };
    tabsById[tabId] = {
      ...tab,
      revision: Math.max(tab.revision, payload.revision),
      snapshotId: payload.snapshotId,
    };
  }
  setWorkspaceState({
    ...workspace,
    tabsById,
    snapshotBindingsByDocumentKey: {
      ...workspace.snapshotBindingsByDocumentKey,
      [payload.documentKey]: {
        documentKey: payload.documentKey,
        revision: payload.revision,
        snapshotId: payload.snapshotId,
      },
    },
  });
}

export function clearWorkspaceSnapshotBinding(documentKey: string, snapshotId?: SnapshotId | null): void {
  const workspace = get(workspaceStore);
  if (!documentKey) return;
  const current = workspace.snapshotBindingsByDocumentKey[documentKey];
  if (!current) return;
  if (snapshotId != null && current.snapshotId !== snapshotId) return;
  const snapshotBindingsByDocumentKey = { ...workspace.snapshotBindingsByDocumentKey };
  delete snapshotBindingsByDocumentKey[documentKey];
  let tabsById = workspace.tabsById;
  for (const [tabId, tab] of Object.entries(workspace.tabsById)) {
    if (tab.documentKey !== documentKey) continue;
    if (snapshotId != null && tab.snapshotId !== snapshotId) continue;
    if (tabsById === workspace.tabsById) tabsById = { ...tabsById };
    tabsById[tabId] = { ...tab, snapshotId: null };
  }
  setWorkspaceState({
    ...workspace,
    tabsById,
    snapshotBindingsByDocumentKey,
  });
}

export function syncSidecarWorkspaceLanguageFromPrimary(): void {
  setWorkspaceState(syncSidecarLanguageFromPrimary(get(workspaceStore)));
}

export function resetWorkspaceState(): void {
  setWorkspaceState(initialWorkspaceState);
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
