import type { SnapshotId } from '@core-wasm/index';
import type { SupportedEditorLanguageId } from '../monaco/language-support';
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
  activateWorkspaceTabTransition,
  closeWorkspaceTabTransition,
  createWorkspaceTabTransition,
  createEditorWorkspaceState,
  ensureColumnDetailDraftTab,
  reinitializeWorkspaceFromPrimaryTab,
  removeColumnDetailDraftTab,
  summarizeWorkspaceTabs,
  transitionWorkspaceTabDocument as transitionWorkspaceTabDocumentState,
  updateWorkspaceTab as patchWorkspaceTab,
  type EditorWorkspaceState,
  type EditorWorkspaceTab,
  type EditorWorkspaceTabPatch,
  type EditorWorkspaceTabSummary,
  type TargetDocumentTransition,
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
    savedText: active.sourceText,
  };
}

export const initialWorkspaceState: EditorWorkspaceState = createEditorWorkspaceState(
  createWorkspacePrimaryTab({ id: 'primary', name: 'Untitled' }),
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
          sidecarState: tab.sidecarState
            ? {
                ...tab.sidecarState,
                graph: { viewport: tab.sidecarState.graph.viewport ? { ...tab.sidecarState.graph.viewport } : null },
                navigator: {
                  ...tab.sidecarState.navigator,
                  activePath: tab.sidecarState.navigator.activePath.map((segment) => ({ ...segment })),
                  history: tab.sidecarState.navigator.history.map((path) => path.map((segment) => ({ ...segment }))),
                },
                compare: { ...tab.sidecarState.compare, outcome: { ...tab.sidecarState.compare.outcome } },
              }
            : undefined,
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

export function createWorkspaceTabTransitionFromEditor(payload: WorkspaceEditorTabInput) {
  const result = createWorkspaceTabTransition(get(workspaceStore), payload);
  if (result) setWorkspaceState(result.workspace);
  return result;
}

export function activateWorkspaceTabTransitionFromEditor(tabId: string) {
  const result = activateWorkspaceTabTransition(get(workspaceStore), tabId);
  if (result) setWorkspaceState(result.workspace);
  return result;
}

export function closeWorkspaceTabTransitionFromEditor(
  tabId: string,
  blank: { id: string; documentKey: string; name: string; languageId: SupportedEditorLanguageId },
) {
  const result = closeWorkspaceTabTransition(get(workspaceStore), tabId, blank);
  if (result) setWorkspaceState(result.workspace);
  return result;
}

export function getWorkspaceTabSummaries(): EditorWorkspaceTabSummary[] {
  return summarizeWorkspaceTabs(get(workspaceStore));
}

export function ensureColumnDetailDraftWorkspaceTab(payload: { id: string; name: string; sourceText: string }): void {
  const session = getAuthorityDocumentSessionState();
  setWorkspaceState(
    ensureColumnDetailDraftTab(get(workspaceStore), {
      id: payload.id,
      name: payload.name,
      languageId: session.languageId,
      sourceText: payload.sourceText,
    }),
  );
}

export function removeColumnDetailDraftWorkspaceTab(tabId: string): void {
  setWorkspaceState(removeColumnDetailDraftTab(get(workspaceStore), tabId));
}

export function updateWorkspaceTab(tabId: string, patch: EditorWorkspaceTabPatch): void {
  const workspace = get(workspaceStore);
  if (tabId === workspace.primaryTabId) {
    patchAuthorityActiveDocument(patch);
    return;
  }
  const isNonMainDocumentDraft = workspace.tabsById[tabId]?.role === 'sidecar'
    || workspace.tabsById[tabId]?.role === 'column-detail-draft';
  const { languageId: _ignoredLanguageId, ...patchWithoutLanguage } = patch;
  setWorkspaceState(patchWorkspaceTab(workspace, tabId, isNonMainDocumentDraft ? patch : patchWithoutLanguage));
}

/** Applies an identity-checked document replacement to one left tab. */
export function transitionWorkspaceTabDocument(transition: TargetDocumentTransition): boolean {
  const next = transitionWorkspaceTabDocumentState(getAuthorityWorkspaceState(), transition);
  if (!next) return false;
  setWorkspaceState(next);
  return true;
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
