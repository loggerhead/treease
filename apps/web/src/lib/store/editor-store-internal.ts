import type { SnapshotId } from '@core-wasm/index';
import { writable, get, derived, type Writable, type Readable } from 'svelte/store';
import { type SupportedEditorLanguageId } from '../monaco/language-support';
import type { PathSeg } from './tree-path';
import {
  clearEditorMutation as clearDocumentSessionEditorMutation,
  compareEditToken as documentSessionCompareEditTokenStore,
  documentKey as documentSessionDocumentKeyStore,
  documentSessionStore,
  editorIO as documentSessionEditorIOStore,
  editorMutation as documentSessionEditorMutationStore,
  editorMutationRawStore,
  editorRevision as documentSessionEditorRevisionStore,
  getDocumentSessionState,
  getEditorMutationRawState,
  getEditorMutationState,
  graphAppliedRevision as documentSessionGraphAppliedRevisionStore,
  initialDocumentSessionState,
  languageId as documentSessionLanguageIdStore,
  previousSourceText as documentSessionPreviousSourceTextStore,
  setCompareEditToken,
  setDocumentSessionState,
  setEditorMutationState,
  setEditorRevision,
  setGraphAppliedRevision,
  setDocumentKey as setDocumentSessionKey,
  setEditorIO as setDocumentSessionEditorIO,
  setLanguageId as setDocumentSessionLanguageId,
  setSourceText as setDocumentSessionSourceText,
  sourceText as documentSessionSourceTextStore,
  emitEditorMutation as emitDocumentSessionEditorMutation,
} from './document-session-store';
import {
  syncSidecarLanguageFromPrimary,
  type EditorWorkspaceState,
  type EditorWorkspaceTab,
  type EditorWorkspaceTabPatch,
  type WorkspaceEditorTabInput,
} from './editor-workspace';
import {
  activateWorkspaceTabFromEditor as activateWorkspaceTabInWorkspace,
  addWorkspaceTabFromEditor as addWorkspaceTabInWorkspace,
  bindWorkspaceSnapshot as bindWorkspaceSnapshotState,
  clearWorkspaceSnapshotBinding,
  cloneWorkspaceStateForRead,
  cloneWorkspaceStateForWrite,
  closeWorkspaceTabFromEditor as closeWorkspaceTabInWorkspace,
  editorWorkspace,
  ensureDetachedSidecarWorkspaceTab,
  ensureSidecarWorkspaceTab,
  getWorkspaceRawState,
  getWorkspaceSnapshotId as getWorkspaceSnapshotIdState,
  getWorkspaceState,
  getWorkspaceTabSummaries as getWorkspaceTabSummariesState,
  initWorkspaceFromPrimaryTab as initWorkspaceFromPrimaryTabState,
  initialWorkspaceState,
  removeDetachedSidecarWorkspaceTab,
  registerWorkspaceCoordinator,
  syncSidecarWorkspaceLanguageFromPrimary,
  setWorkspaceState,
  updateWorkspaceTab as updateWorkspaceTabState,
  workspaceStore,
} from './workspace-store';
import { resetActiveDocumentAuthority } from './active-document-authority';
import {
  clearJsonBlockSelectionForDocument,
  completeFullEditStreamUi,
  fullEditUiState as fullEditUiStateWritable,
  fullEditUiStateStore,
  getFullEditUiStateRaw,
  getFullEditUiStateSnapshot,
  getJsonBlockSelectionRaw,
  getJsonBlockSelectionSnapshot,
  initialFullEditUiState,
  jsonBlockSelection as jsonBlockSelectionWritable,
  jsonBlockSelectionStore,
  finishFullEditStream as finishPublicFullEditStream,
  markFullEditStreamFinalizing,
  markFullEditStreamSettled,
  registerFullEditUiCoordinator,
  resetFullEditUiState,
  setFullEditUiState,
  setJsonBlockSelection,
  cancelFullEditStream as cancelPublicFullEditStream,
} from './full-edit-ui-store';
import {
  getActiveTempModelRaw,
  getActiveTempModelSnapshot,
  getTreeStateRaw,
  getTreeStateSnapshot,
  initialTempModel,
  initialTreeState,
  registerGraphSelectionCoordinator,
  setTempModelState,
  setTreeStateState,
  tempModelStore,
  treeSyncStore,
} from './graph-selection-store';
import type {
  DiagnosticItem,
  DocumentSessionState,
  EditorIO,
  EditorMutation,
  EditorMutationEnvelope,
  EditorState,
  FullEditTransportKind,
  FullEditUiState,
  GraphHighlightState,
  JsonBlockSelection,
  TempModel,
  TreeSyncState,
} from './editor-store-types';
export type { PathSeg };
export type * from './editor-store-types';

const initialEditorState: EditorState = {
  ...initialDocumentSessionState,
  editorMutation: null,
  treeState: initialTreeState,
  fullEditUiState: initialFullEditUiState,
  jsonBlockSelection: null,
  tempModel: initialTempModel,
  workspace: initialWorkspaceState,
};

registerFullEditUiCoordinator({
  onFullEditUiStateChange: (next) => {
    workspaceStore.update((workspace) => {
      const primaryTab = workspace.tabsById[workspace.primaryTabId];
      if (!primaryTab) return workspace;
      return {
        ...workspace,
        tabsById: {
          ...workspace.tabsById,
          [primaryTab.id]: {
            ...primaryTab,
            fullEditUiState: next,
          },
        },
      };
    });
  },
});

registerGraphSelectionCoordinator({
  onTempModelChange: (next) => {
    workspaceStore.update((workspace) => {
      const primaryTab = workspace.tabsById[workspace.primaryTabId];
      if (!primaryTab) return workspace;
      return {
        ...workspace,
        tabsById: {
          ...workspace.tabsById,
          [primaryTab.id]: {
            ...primaryTab,
            tempModel: next,
          },
        },
      };
    });
  },
});

registerWorkspaceCoordinator({
  onWorkspaceChange: (next) => {
    const primaryTab = next.tabsById[next.primaryTabId];
    if (!primaryTab) return;
    setTempModelState(primaryTab.tempModel);
    setFullEditUiState(primaryTab.fullEditUiState);
  },
});

function getRawEditorState(): EditorState {
  return {
    ...get(documentSessionStore),
    editorMutation: getEditorMutationRawState(),
    treeState: getTreeStateRaw(),
    fullEditUiState: getFullEditUiStateRaw(),
    jsonBlockSelection: getJsonBlockSelectionRaw(),
    tempModel: getActiveTempModelRaw(),
    workspace: getWorkspaceRawState(),
  };
}

function setRawEditorState(state: EditorState): void {
  const currentDocumentSession = get(documentSessionStore);
  const nextDocumentSession = {
    sourceText: state.sourceText,
    previousSourceText: state.previousSourceText,
    documentKey: state.documentKey,
    languageId: state.languageId,
    compareEditToken: state.compareEditToken,
    editorRevision: state.editorRevision,
    graphAppliedRevision: state.graphAppliedRevision,
    editorIO: state.editorIO,
  };
  const sessionChanged =
    currentDocumentSession.sourceText !== nextDocumentSession.sourceText ||
    currentDocumentSession.previousSourceText !== nextDocumentSession.previousSourceText ||
    currentDocumentSession.documentKey !== nextDocumentSession.documentKey ||
    currentDocumentSession.languageId !== nextDocumentSession.languageId ||
    currentDocumentSession.compareEditToken !== nextDocumentSession.compareEditToken ||
    currentDocumentSession.editorRevision !== nextDocumentSession.editorRevision ||
    currentDocumentSession.graphAppliedRevision !== nextDocumentSession.graphAppliedRevision ||
    currentDocumentSession.editorIO !== nextDocumentSession.editorIO;
  if (sessionChanged) {
    setDocumentSessionState(nextDocumentSession);
  }
  if (get(editorMutationRawStore) !== state.editorMutation) {
    setEditorMutationState(state.editorMutation);
  }
  if (get(treeSyncStore) !== state.treeState) {
    setTreeStateState(state.treeState);
  }
  if (get(fullEditUiStateStore) !== state.fullEditUiState) {
    fullEditUiStateStore.set(state.fullEditUiState);
  }
  if (get(jsonBlockSelectionStore) !== state.jsonBlockSelection) {
    jsonBlockSelectionStore.set(state.jsonBlockSelection);
  }
  if (get(tempModelStore) !== state.tempModel) {
    setTempModelState(state.tempModel);
  }
  // A Document Session change has already advanced authority. Replaying the
  // aggregate's pre-change Workspace snapshot would restore stale identity.
  if (!sessionChanged && getWorkspaceRawState() !== state.workspace) {
    setWorkspaceState(state.workspace);
  }
}

const internalStore = derived(
  [
    documentSessionSourceTextStore,
    documentSessionPreviousSourceTextStore,
    documentSessionDocumentKeyStore,
    documentSessionLanguageIdStore,
    documentSessionCompareEditTokenStore,
    documentSessionEditorRevisionStore,
    documentSessionGraphAppliedRevisionStore,
    documentSessionEditorIOStore,
    documentSessionEditorMutationStore,
    treeSyncStore,
    fullEditUiStateWritable,
    jsonBlockSelectionWritable,
    tempModelStore,
    workspaceStore,
  ],
  ([
    $sourceText,
    $previousSourceText,
    $documentKey,
    $languageId,
    $compareEditToken,
    $editorRevision,
    $graphAppliedRevision,
    $editorIO,
    $editorMutation,
    $treeState,
    $fullEditUiState,
    $jsonBlockSelection,
    $tempModel,
    $workspace,
  ]) => ({
    sourceText: $sourceText,
    previousSourceText: $previousSourceText,
    documentKey: $documentKey,
    languageId: $languageId,
    compareEditToken: $compareEditToken,
    editorRevision: $editorRevision,
    graphAppliedRevision: $graphAppliedRevision,
    editorIO: $editorIO,
    editorMutation: $editorMutation,
    treeState: $treeState,
    fullEditUiState: $fullEditUiState,
    jsonBlockSelection: $jsonBlockSelection,
    tempModel: $tempModel,
    workspace: $workspace,
  }),
);

function updateState(fn: (s: EditorState) => EditorState): void {
  const current = getRawEditorState();
  const next = fn(current);
  if (next === current) return;
  setRawEditorState(next);
}

function pendingGraphAppliedRevision(current: number, nextRevision: number): number {
  const floor = Math.max(0, nextRevision - 1);
  return Math.min(current, floor);
}

function updatePrimaryWorkspaceTab(
  state: EditorState,
  updater: (tab: EditorWorkspaceTab) => EditorWorkspaceTab,
): EditorState {
  const primaryTab = state.workspace.tabsById[state.workspace.primaryTabId];
  if (!primaryTab) return state;
  const nextPrimaryTab = updater(primaryTab);
  if (nextPrimaryTab === primaryTab) return state;
  return {
    ...state,
    workspace: {
      ...state.workspace,
      tabsById: {
        ...state.workspace.tabsById,
        [primaryTab.id]: nextPrimaryTab,
      },
    },
  };
}

function mirrorPrimaryWorkspaceTab(
  state: EditorState,
  patch: Partial<
    Pick<
      EditorWorkspaceTab,
      | 'sourceText'
      | 'documentKey'
      | 'languageId'
      | 'revision'
      | 'graphAppliedRevision'
      | 'tempModel'
      | 'fullEditUiState'
    >
  >,
): EditorState {
  return updatePrimaryWorkspaceTab(state, (primaryTab) => ({
    ...primaryTab,
    ...patch,
  }));
}

function setPrimaryFullEditUiState(
  state: EditorState,
  fullEditUiState: FullEditUiState,
  graphAppliedRevision = state.graphAppliedRevision,
): EditorState {
  return mirrorPrimaryWorkspaceTab(
    { ...state, graphAppliedRevision, fullEditUiState },
    { graphAppliedRevision, fullEditUiState },
  );
}

type FullEditOwnerPayload = {
  sessionId: string;
  ownerKey: string;
};

function buildActiveFullEditUiState(payload: {
  sessionId: string | null;
  ownerKey: string | null;
  documentKey: string;
  revision: number;
  language: SupportedEditorLanguageId | '';
  transportKind: FullEditTransportKind;
  reason: FullEditUiState['reason'];
  phase: 'preparing' | 'streaming';
}): FullEditUiState {
  return {
    active: true,
    sessionId: payload.sessionId,
    ownerKey: payload.ownerKey,
    documentKey: payload.documentKey,
    revision: payload.revision,
    streamSeq: 0,
    inputByteLength: 0,
    modelVersionId: null,
    byteLength: 0,
    language: payload.language,
    phase: payload.phase,
    sessionKind: 'full-edit',
    transportKind: payload.transportKind,
    reason: payload.reason,
  };
}

function matchesFullEditOwner(
  current: FullEditUiState,
  payload: FullEditOwnerPayload,
): boolean {
  return current.active && current.sessionId === payload.sessionId && current.ownerKey === payload.ownerKey;
}

function updateOwnedFullEditUiState(
  state: EditorState,
  payload: FullEditOwnerPayload,
  updater: (current: FullEditUiState) => FullEditUiState | null,
): EditorState {
  const current = state.fullEditUiState;
  if (!matchesFullEditOwner(current, payload)) return state;
  const next = updater(current);
  return next ? setPrimaryFullEditUiState(state, next) : state;
}

function clonePathSegs(path: PathSeg[]): PathSeg[] {
  return path.map((segment) => ({ ...segment }));
}

function cloneGraphHighlight(graphHighlight: GraphHighlightState | null): GraphHighlightState | null {
  if (!graphHighlight) return null;
  return {
    ...graphHighlight,
    path: clonePathSegs(graphHighlight.path),
  };
}

function cloneDiagnostics(diagnostics: DiagnosticItem[]): DiagnosticItem[] {
  return diagnostics.map((diagnostic) => ({
    ...diagnostic,
    context: diagnostic.context.map((line) => ({ ...line })),
  }));
}

function cloneUnknownForRead<T>(value: T): T {
  if (Array.isArray(value)) {
    return value.map((item) => cloneUnknownForRead(item)) as T;
  }
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>).map(([key, nestedValue]) => [key, cloneUnknownForRead(nestedValue)]),
    ) as T;
  }
  return value;
}

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

function baseCloneTempModelForRead(tempModel: TempModel): TempModel {
  return {
    ...tempModel,
    treePath: clonePathSegs(tempModel.treePath),
    graphHighlight: cloneGraphHighlight(tempModel.graphHighlight),
    diagnostics: cloneDiagnostics(tempModel.diagnostics),
  };
}

const tempModelReadSnapshots = new WeakMap<TempModel, TempModel>();

function cloneTempModelForRead(tempModel: TempModel): TempModel {
  const cached = tempModelReadSnapshots.get(tempModel);
  if (cached) return cached;
  const snapshot = deepFreezeForRead(baseCloneTempModelForRead(tempModel));
  tempModelReadSnapshots.set(tempModel, snapshot);
  return snapshot;
}

function baseCloneJsonBlockSelectionForRead(jsonBlockSelection: JsonBlockSelection | null): JsonBlockSelection | null {
  if (!jsonBlockSelection) return null;
  return { ...jsonBlockSelection };
}

let memoizedRawJsonBlockSelection: JsonBlockSelection | null = null;
let memoizedReadJsonBlockSelection: JsonBlockSelection | null = null;

function cloneJsonBlockSelectionForRead(jsonBlockSelection: JsonBlockSelection | null): JsonBlockSelection | null {
  if (memoizedRawJsonBlockSelection === jsonBlockSelection) return memoizedReadJsonBlockSelection;
  const snapshot = deepFreezeForRead(baseCloneJsonBlockSelectionForRead(jsonBlockSelection));
  memoizedRawJsonBlockSelection = jsonBlockSelection;
  memoizedReadJsonBlockSelection = snapshot;
  return snapshot;
}

function cloneTreeStateForWrite(treeState: TreeSyncState): TreeSyncState {
  return {
    ...treeState,
    tree: cloneUnknownForRead(treeState.tree),
    value: cloneUnknownForRead(treeState.value),
  };
}

function cloneFullEditUiStateForWrite(fullEditUiState: FullEditUiState): FullEditUiState {
  return { ...fullEditUiState };
}

function cloneJsonBlockSelectionForWrite(jsonBlockSelection: JsonBlockSelection | null): JsonBlockSelection | null {
  if (!jsonBlockSelection) return null;
  return { ...jsonBlockSelection };
}

function cloneEditorMutationForWrite(mutation: EditorMutation): EditorMutation {
  return {
    ...mutation,
    payload: {
      ...mutation.payload,
      graphEditFallback: mutation.payload.graphEditFallback
        ? {
            ...mutation.payload.graphEditFallback,
            path: clonePathSegs(mutation.payload.graphEditFallback.path),
          }
        : undefined,
    },
  };
}

function cloneTempModelForWrite(tempModel: TempModel): TempModel {
  return baseCloneTempModelForRead(tempModel);
}

function cloneTempModelPatchForWrite(partial: Partial<TempModel>): Partial<TempModel> {
  const safePartial: Partial<TempModel> = { ...partial };
  if ('treePath' in partial) {
    safePartial.treePath = partial.treePath ? clonePathSegs(partial.treePath) : partial.treePath;
  }
  if ('graphHighlight' in partial) {
    safePartial.graphHighlight =
      partial.graphHighlight !== undefined ? cloneGraphHighlight(partial.graphHighlight) : partial.graphHighlight;
  }
  if ('diagnostics' in partial) {
    safePartial.diagnostics = partial.diagnostics ? cloneDiagnostics(partial.diagnostics) : partial.diagnostics;
  }
  return safePartial;
}

let memoizedRawTreeState: TreeSyncState | null = null;
let memoizedReadTreeState: TreeSyncState | null = null;

function cloneTreeStateForRead(treeState: TreeSyncState): TreeSyncState {
  if (memoizedRawTreeState === treeState && memoizedReadTreeState) return memoizedReadTreeState;
  const snapshot = deepFreezeForRead({
    ...treeState,
    tree: cloneUnknownForRead(treeState.tree),
    value: cloneUnknownForRead(treeState.value),
  });
  memoizedRawTreeState = treeState;
  memoizedReadTreeState = snapshot;
  return snapshot;
}

function baseCloneEditorMutationForRead(editorMutation: EditorMutationEnvelope | null): EditorMutationEnvelope | null {
  if (!editorMutation) return null;
  const mutation = editorMutation.mutation;
  return {
    ...editorMutation,
    mutation: {
      ...mutation,
      payload: {
        ...mutation.payload,
        graphEditFallback: mutation.payload.graphEditFallback
          ? {
              ...mutation.payload.graphEditFallback,
              path: clonePathSegs(mutation.payload.graphEditFallback.path),
            }
          : undefined,
      },
    },
  };
}

let memoizedRawEditorMutation: EditorMutationEnvelope | null = null;
let memoizedReadEditorMutation: EditorMutationEnvelope | null = null;

function cloneEditorMutationForRead(editorMutation: EditorMutationEnvelope | null): EditorMutationEnvelope | null {
  if (memoizedRawEditorMutation === editorMutation) return memoizedReadEditorMutation;
  const snapshot = deepFreezeForRead(baseCloneEditorMutationForRead(editorMutation));
  memoizedRawEditorMutation = editorMutation;
  memoizedReadEditorMutation = snapshot;
  return snapshot;
}

let memoizedRawFullEditUiState: FullEditUiState | null = null;
let memoizedReadFullEditUiState: FullEditUiState | null = null;

function cloneFullEditUiStateForRead(fullEditUiState: FullEditUiState): FullEditUiState {
  if (memoizedRawFullEditUiState === fullEditUiState && memoizedReadFullEditUiState)
    return memoizedReadFullEditUiState;
  const snapshot = deepFreezeForRead({ ...fullEditUiState });
  memoizedRawFullEditUiState = fullEditUiState;
  memoizedReadFullEditUiState = snapshot;
  return snapshot;
}

function cloneEditorStateForRead(state: EditorState): EditorState {
  return {
    ...state,
    treeState: cloneTreeStateForRead(state.treeState),
    tempModel: cloneTempModelForRead(state.tempModel),
    fullEditUiState: cloneFullEditUiStateForRead(state.fullEditUiState),
    editorMutation: cloneEditorMutationForRead(state.editorMutation),
    jsonBlockSelection: cloneJsonBlockSelectionForRead(state.jsonBlockSelection),
    workspace: cloneWorkspaceStateForRead(state.workspace),
  };
}

function cloneFieldValue<K extends keyof EditorState>(key: K, value: EditorState[K]): EditorState[K] {
  if (key === 'workspace') return cloneWorkspaceStateForRead(value as EditorWorkspaceState) as EditorState[K];
  if (key === 'tempModel') return cloneTempModelForRead(value as TempModel) as EditorState[K];
  if (key === 'fullEditUiState') return cloneFullEditUiStateForRead(value as FullEditUiState) as EditorState[K];
  if (key === 'treeState') return cloneTreeStateForRead(value as TreeSyncState) as EditorState[K];
  if (key === 'jsonBlockSelection') {
    return cloneJsonBlockSelectionForRead(value as JsonBlockSelection | null) as EditorState[K];
  }
  if (key === 'editorMutation') {
    return cloneEditorMutationForRead(value as EditorMutationEnvelope | null) as EditorState[K];
  }
  return value;
}

function cloneFieldValueForUpdate<K extends keyof EditorState>(key: K, value: EditorState[K]): EditorState[K] {
  if (key === 'workspace') return cloneWorkspaceStateForWrite(value as EditorWorkspaceState) as EditorState[K];
  if (key === 'tempModel') return baseCloneTempModelForRead(value as TempModel) as EditorState[K];
  if (key === 'fullEditUiState') return { ...(value as FullEditUiState) } as EditorState[K];
  if (key === 'treeState') return cloneTreeStateForWrite(value as TreeSyncState) as EditorState[K];
  if (key === 'jsonBlockSelection') {
    return cloneJsonBlockSelectionForWrite(value as JsonBlockSelection | null) as EditorState[K];
  }
  if (key === 'editorMutation') {
    const editorMutationValue = value as EditorMutationEnvelope | null;
    if (!editorMutationValue) return editorMutationValue as EditorState[K];
    return {
      ...editorMutationValue,
      mutation: cloneEditorMutationForWrite(editorMutationValue.mutation),
    } as EditorState[K];
  }
  return value;
}

/**
 * Create the editor state-management store and encapsulate all state mutations.
 * @returns Store object with subscribe, actions, and reset
 */
function createEditorStore() {
  return {
    subscribe: (run: (value: EditorState) => void) =>
      derived(internalStore, ($s) => cloneEditorStateForRead($s)).subscribe(run),
    actions: {
      setSourceText: (text: string) => setDocumentSessionSourceText(text),
      setDocumentKey: (key: string) =>
        updateState((s) => {
          const nextState = mirrorPrimaryWorkspaceTab({ ...s, documentKey: key }, { documentKey: key });
          setDocumentSessionKey(key);
          return nextState;
        }),
      setLanguageId: (lang: SupportedEditorLanguageId) =>
        updateState((s) => {
          const nextState = mirrorPrimaryWorkspaceTab({ ...s, languageId: lang }, { languageId: lang });
          setDocumentSessionLanguageId(lang);
          return {
            ...nextState,
            workspace: syncSidecarLanguageFromPrimary(nextState.workspace),
          };
        }),
      incrementCompareEditToken: () => setCompareEditToken(getDocumentSessionState().compareEditToken + 1),
      incrementEditorRevision: () =>
        updateState((s) => {
          const editorRevision = s.editorRevision + 1;
          setEditorRevision(editorRevision);
          return mirrorPrimaryWorkspaceTab({ ...s, editorRevision }, { revision: editorRevision });
        }),
      setGraphAppliedRevision: (rev: number) =>
        updateState((s) => {
          setGraphAppliedRevision(rev);
          return mirrorPrimaryWorkspaceTab({ ...s, graphAppliedRevision: rev }, { graphAppliedRevision: rev });
        }),
      setEditorIO: (io: EditorIO | null) =>
        updateState((s) => {
          setDocumentSessionEditorIO(io);
          return { ...s, editorIO: io };
        }),
      emitMutation: (mutation: EditorMutation) => {
        emitDocumentSessionEditorMutation(mutation);
        updateState((s) => ({ ...s, editorMutation: getEditorMutationState() }));
      },
      clearMutation: () =>
        updateState((s) => {
          clearDocumentSessionEditorMutation();
          return { ...s, editorMutation: null };
        }),
      setTreeState: (treeState: TreeSyncState) =>
        updateState((s) => ({ ...s, treeState: cloneTreeStateForWrite(treeState) })),
      setFullEditUiState: (fullEditUiState: FullEditUiState) => {
        const nextFullEditUiState = cloneFullEditUiStateForWrite(fullEditUiState);
        updateState((s) => {
          setFullEditUiState(nextFullEditUiState);
          return mirrorPrimaryWorkspaceTab(
            { ...s, fullEditUiState: nextFullEditUiState },
            { fullEditUiState: nextFullEditUiState },
          );
        });
      },
      setJsonBlockSelection: (jsonBlockSelection: JsonBlockSelection | null) =>
        updateState((s) => {
          setJsonBlockSelection(jsonBlockSelection);
          return { ...s, jsonBlockSelection: cloneJsonBlockSelectionForWrite(jsonBlockSelection) };
        }),
      initWorkspaceFromPrimaryTab: (payload: { id: string; name: string }) => initWorkspaceFromPrimaryTabState(payload),
      addWorkspaceTabFromEditor: (payload: WorkspaceEditorTabInput) => addWorkspaceTabInWorkspace(payload),
      activateWorkspaceTabFromEditor: (payload: WorkspaceEditorTabInput) => activateWorkspaceTabInWorkspace(payload),
      closeWorkspaceTabFromEditor: (tabId: string, fallback?: WorkspaceEditorTabInput) =>
        closeWorkspaceTabInWorkspace(tabId, fallback),
      getWorkspaceTabSummaries: () => getWorkspaceTabSummariesState(),
      ensureSidecarWorkspaceTab: (payload: { id: string; name: string; sourceText: string }) =>
        ensureSidecarWorkspaceTab(payload),
      ensureDetachedSidecarWorkspaceTab: (payload: { id: string; name: string; sourceText: string }) =>
        ensureDetachedSidecarWorkspaceTab(payload),
      removeDetachedSidecarWorkspaceTab: (tabId: string) => removeDetachedSidecarWorkspaceTab(tabId),
      updateWorkspaceTab: (tabId: string, patch: EditorWorkspaceTabPatch) => updateWorkspaceTabState(tabId, patch),
      bindWorkspaceSnapshot: (payload: { documentKey: string; revision: number; snapshotId: SnapshotId | null | undefined }) =>
        bindWorkspaceSnapshotState(payload),
      clearWorkspaceSnapshot: (documentKey: string, snapshotId?: SnapshotId | null) =>
        clearWorkspaceSnapshotBinding(documentKey, snapshotId),
      getWorkspaceSnapshotId: (documentKey: string): SnapshotId | null => getWorkspaceSnapshotIdState(documentKey),
      syncSidecarLanguageFromPrimary: () => syncSidecarWorkspaceLanguageFromPrimary(),
      clearJsonBlockSelectionForDocument: (documentKey: string) =>
        updateState((s) => {
          clearJsonBlockSelectionForDocument(documentKey);
          return s.jsonBlockSelection?.sourceDocumentKey === documentKey ? { ...s, jsonBlockSelection: null } : s;
        }),
      prepareFullEditStream: (payload: {
        documentKey: string;
        revision: number;
        language: SupportedEditorLanguageId | '';
        transportKind: FullEditTransportKind;
        reason: FullEditUiState['reason'];
      }) =>
        updateState((s) => {
          const graphAppliedRevision = pendingGraphAppliedRevision(s.graphAppliedRevision, payload.revision);
          const fullEditUiState = buildActiveFullEditUiState({
            sessionId: null,
            ownerKey: null,
            documentKey: payload.documentKey,
            revision: payload.revision,
            language: payload.language,
            transportKind: payload.transportKind,
            reason: payload.reason,
            phase: 'preparing',
          });
          setFullEditUiState(fullEditUiState);
          return setPrimaryFullEditUiState(s, fullEditUiState, graphAppliedRevision);
        }),
      cancelPreparedFullEditStream: (payload: {
        documentKey: string;
        revision: number;
        reason: FullEditUiState['reason'];
      }) =>
        updateState((s) => {
          const current = s.fullEditUiState;
          if (
            !current.active ||
            current.sessionId !== null ||
            current.phase !== 'preparing' ||
            current.documentKey !== payload.documentKey ||
            current.revision !== payload.revision ||
            current.reason !== payload.reason
          )
            return s;
          setFullEditUiState(initialFullEditUiState);
          return setPrimaryFullEditUiState(s, initialFullEditUiState);
        }),
      beginFullEditStream: (payload: {
        sessionId: string;
        ownerKey: string;
        documentKey: string;
        revision: number;
        language: SupportedEditorLanguageId | '';
        transportKind: FullEditTransportKind;
        reason: FullEditUiState['reason'];
      }) =>
        updateState((s) => {
          const graphAppliedRevision = pendingGraphAppliedRevision(s.graphAppliedRevision, payload.revision);
          const fullEditUiState = buildActiveFullEditUiState({
            sessionId: payload.sessionId,
            ownerKey: payload.ownerKey,
            documentKey: payload.documentKey,
            revision: payload.revision,
            language: payload.language,
            transportKind: payload.transportKind,
            reason: payload.reason,
            phase: 'streaming',
          });
          setFullEditUiState(fullEditUiState);
          return setPrimaryFullEditUiState(s, fullEditUiState, graphAppliedRevision);
        }),
      appendFullEditStreamChunkMeta: (payload: {
        sessionId: string;
        ownerKey: string;
        streamSeq: number;
        inputByteLength: number;
        modelVersionId?: number | null;
      }) =>
        updateState((s) =>
          updateOwnedFullEditUiState(s, payload, (current) => {
            if (current.phase !== 'streaming') return null;
            if (payload.streamSeq <= current.streamSeq) return null;
            if (payload.inputByteLength < current.inputByteLength) return null;
            return {
              ...current,
              streamSeq: payload.streamSeq,
              inputByteLength: payload.inputByteLength,
              byteLength: payload.inputByteLength,
              modelVersionId:
                typeof payload.modelVersionId === 'number' ? payload.modelVersionId : current.modelVersionId,
            };
          }),
        ),
      markFullEditStreamFinalizing: (payload: FullEditOwnerPayload) =>
        updateState((s) => {
          markFullEditStreamFinalizing(payload);
          return updateOwnedFullEditUiState(s, payload, (current) =>
            current.phase === 'streaming' ? { ...current, phase: 'finalizing' } : null,
          );
        }),
      markFullEditStreamSettled: (payload: FullEditOwnerPayload) =>
        updateState((s) => {
          markFullEditStreamSettled(payload);
          return updateOwnedFullEditUiState(s, payload, (current) =>
            current.phase === 'finalizing' ? { ...current, phase: 'settled' } : null,
          );
        }),
      completeFullEditStreamUi: (payload: FullEditOwnerPayload) =>
        updateState((s) => {
          completeFullEditStreamUi(payload);
          return updateOwnedFullEditUiState(s, payload, (current) => ({ ...current, phase: 'idle' }));
        }),
      finishFullEditStream: (payload: FullEditOwnerPayload) =>
        updateState((s) => {
          finishPublicFullEditStream(payload);
          return updateOwnedFullEditUiState(s, payload, () => initialFullEditUiState);
        }),
      cancelFullEditStream: (payload: FullEditOwnerPayload) =>
        updateState((s) => {
          cancelPublicFullEditStream(payload);
          return updateOwnedFullEditUiState(s, payload, () => initialFullEditUiState);
        }),
      updateTempModel: (partial: Partial<TempModel>) =>
        updateState((s) => {
          const safePartial = cloneTempModelPatchForWrite(partial);
          const tempModel = { ...s.tempModel, ...safePartial };
          return mirrorPrimaryWorkspaceTab({ ...s, tempModel }, { tempModel });
        }),
      setTempModel: (model: TempModel) => {
        const nextTempModel = cloneTempModelForWrite(model);
        updateState((s) => mirrorPrimaryWorkspaceTab({ ...s, tempModel: nextTempModel }, { tempModel: nextTempModel }));
      },
      resetTempModel: () =>
        updateState((s) => mirrorPrimaryWorkspaceTab({ ...s, tempModel: initialTempModel }, { tempModel: initialTempModel })),
    },
    reset: (): void => {
      resetActiveDocumentAuthority();
      setRawEditorState(initialEditorState);
    },
    get: (): EditorState => {
      const state = getRawEditorState();
      return cloneEditorStateForRead(state);
    },
  };
}

export const editorStore = createEditorStore();

export function getEditorStateSnapshot(): EditorState {
  return cloneEditorStateForRead(getRawEditorState());
}

export function resetEditorState(): void {
  editorStore.reset();
}

function createFieldStore<K extends keyof EditorState>(
  key: K,
  setter: (value: EditorState[K]) => void,
): Writable<EditorState[K]> {
  return {
    subscribe: (run: (value: EditorState[K]) => void) => {
      let initialized = false;
      let currentRaw: EditorState[K] | undefined;
      return internalStore.subscribe(($s) => {
        const nextRaw = $s[key];
        if (initialized && Object.is(nextRaw, currentRaw)) return;
        initialized = true;
        currentRaw = nextRaw;
        run(cloneFieldValue(key, nextRaw));
      });
    },
    set: setter,
    update: (fn: (value: EditorState[K]) => EditorState[K]) => {
      const current = cloneFieldValueForUpdate(key, getRawEditorState()[key]);
      setter(fn(current));
    },
  };
}

export {
  compareEditToken,
  documentKey,
  editorIO,
  editorMutation,
  editorRevision,
  graphAppliedRevision,
  languageId,
  previousSourceText,
  sourceText,
} from './document-session-store';
export { fullEditUiState, jsonBlockSelection } from './full-edit-ui-store';
export { activeTempModel, treeState } from './graph-selection-store';
export { editorWorkspace } from './workspace-store';
