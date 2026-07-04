import type { DocumentTextEdit, SnapshotId } from '@core-wasm/index';
import type * as Monaco from 'monaco-editor';
import { writable, get, derived, type Writable, type Readable } from 'svelte/store';
import { editorLanguageFallback, type SupportedEditorLanguageId } from '../monaco/language-support';
import type { TreeNode } from '@core-wasm/index'
import type { PathSeg } from './tree-path';
import {
  activateWorkspaceTab,
  addWorkspaceTab,
  closeWorkspaceTab,
  createEditorWorkspaceState,
  ensureSidecarTab,
  ensureDetachedSidecarTab,
  removeDetachedSidecarTab,
  summarizeWorkspaceTabs,
  syncWorkspaceEditorTab,
  syncSidecarLanguageFromPrimary,
  updateWorkspaceTab as patchWorkspaceTab,
  type EditorWorkspaceState,
  type EditorWorkspaceTab,
  type EditorWorkspaceTabPatch,
  type WorkspaceEditorTabInput,
} from './editor-workspace';
export type { PathSeg };

export type EditorIoContext = 'editor' | 'scratch';
export type EditorIO = {
  context: EditorIoContext;
  getModel: () => Monaco.editor.ITextModel | null;
  getText: () => string;
  setText: (value: string) => void;
  applyTextEdits: (edits: DocumentTextEdit[]) => boolean;
  getLanguage: () => SupportedEditorLanguageId;
};
export type DiagnosticContextLine = {
  lineNumber: number;
  text: string;
};
export type DiagnosticItem = {
  code: 'syntax-error' | 'missing-node';
  message: string;
  startLineNumber: number;
  startColumn: number;
  endLineNumber: number;
  endColumn: number;
  context: DiagnosticContextLine[];
};
export type GraphHighlightTarget = 'key' | 'value' | 'node';
export type TreeSelectionSource = 'editor' | 'graph' | 'breadcrumb' | 'search';
export type GraphHighlightState = {
  path: PathSeg[];
  target?: GraphHighlightTarget;
  revision: number;
  source: TreeSelectionSource;
};
export type TempModel = {
  diffInputText: string;
  scratchText: string;
  commandQuery: string;
  status: string;
  error: string;
  cursor: string;
  selectionLength: number;
  treePath: PathSeg[];
  graphHighlight: GraphHighlightState | null;
  diagnostics: DiagnosticItem[];
};
export type GraphEditReplaceFallbackReason =
  | 'graph-edit-not-single-range'
  | 'missingAnalysis'
  | 'missingDocument'
  | 'invalidPath'
  | 'invalidReplacement'
  | 'unsupportedLanguage'
  | 'unsupportedEdit'
  | 'unsafeEdit';
export type EditorMutation = {
  type: 'replaceSourceText';
  payload: {
    text: string;
    graphEditFallback?: {
      reason: GraphEditReplaceFallbackReason;
      path: PathSeg[];
      kind: 'key' | 'value';
    };
  };
};
export type EditorMutationEnvelope = { id: number; mutation: EditorMutation };
export type TreeSyncSource = 'editor' | 'graph';
export type TreeSyncState = {
  tree: TreeNode | null;
  value: unknown;
  revision: number;
  source: TreeSyncSource;
};
export type FullEditUiPhase = 'idle' | 'preparing' | 'streaming' | 'finalizing' | 'settled';

export type FullEditSessionKind = 'full-edit';
export type FullEditTransportKind = 'memory' | 'file';

// Full-edit UI state drives the editor/graph control plane while a full rebuild is in progress.
export type FullEditUiState = {
  active: boolean;
  sessionId: string | null;
  ownerKey: string | null;
  documentKey: string | null;
  revision: number;
  streamSeq: number;
  inputByteLength: number;
  modelVersionId: number | null;
  byteLength: number;
  language: SupportedEditorLanguageId | '';
  phase: FullEditUiPhase;
  sessionKind: FullEditSessionKind | null;
  transportKind: FullEditTransportKind | null;
  reason:
    | 'initial-example'
    | 'language-example'
    | 'language-switch'
    | 'whole-document-replacement'
    | 'tab-reactivate'
    | 'import-file'
    | 'drop-file'
    | null;
};
export type JsonBlockSelection = {
  sourceDocumentKey: string;
  blockDocumentKey: string;
  revision: number;
  language: 'json';
  text: string;
  startByte: number;
  endByte: number;
  startLineNumber: number;
  startColumn: number;
  endLineNumber: number;
  endColumn: number;
};

export type EditorState = {
  sourceText: string;
  previousSourceText: string;
  documentKey: string;
  languageId: SupportedEditorLanguageId;
  compareEditToken: number;
  editorRevision: number;
  graphAppliedRevision: number;
  editorIO: EditorIO | null;
  editorMutation: EditorMutationEnvelope | null;
  treeState: TreeSyncState;
  fullEditUiState: FullEditUiState;
  jsonBlockSelection: JsonBlockSelection | null;
  tempModel: TempModel;
  workspace: EditorWorkspaceState;
};

const initialTempModel: TempModel = {
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
};

const initialTreeState: TreeSyncState = {
  tree: null,
  value: null,
  revision: 0,
  source: 'editor',
};

const initialFullEditState: FullEditUiState = {
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
};

function createWorkspacePrimaryTab(state: Pick<
  EditorState,
  | 'sourceText'
  | 'documentKey'
  | 'languageId'
  | 'editorRevision'
  | 'graphAppliedRevision'
  | 'tempModel'
  | 'fullEditUiState'
>, payload: { id: string; name: string }): EditorWorkspaceTab {
  return {
    id: payload.id,
    role: 'primary',
    name: payload.name,
    documentKey: state.documentKey,
    languageId: state.languageId,
    sourceText: state.sourceText,
    revision: state.editorRevision,
    graphAppliedRevision: state.graphAppliedRevision,
    snapshotId: null,
    tempModel: state.tempModel,
    fullEditUiState: state.fullEditUiState,
  };
}

function createInitialWorkspace(): EditorWorkspaceState {
  return createEditorWorkspaceState(
    createWorkspacePrimaryTab(
      {
        sourceText: '',
        documentKey: '',
        languageId: editorLanguageFallback,
        editorRevision: 0,
        graphAppliedRevision: 0,
        tempModel: initialTempModel,
        fullEditUiState: initialFullEditState,
      },
      {
        id: 'primary',
        name: 'Primary',
      },
    ),
  );
}

const initialEditorState: EditorState = {
  sourceText: '',
  previousSourceText: '',
  documentKey: '',
  languageId: editorLanguageFallback,
  compareEditToken: 0,
  editorRevision: 0,
  graphAppliedRevision: 0,
  editorIO: null,
  editorMutation: null,
  treeState: initialTreeState,
  fullEditUiState: initialFullEditState,
  jsonBlockSelection: null,
  tempModel: initialTempModel,
  workspace: createInitialWorkspace(),
};

let editorMutationId = 0;

const internalStore = writable<EditorState>(initialEditorState);

function updateState(fn: (s: EditorState) => EditorState): void {
  internalStore.update(fn);
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

function baseCloneWorkspaceForRead(workspace: EditorWorkspaceState): EditorWorkspaceState {
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
          tempModel: cloneTempModelForRead(tab.tempModel),
          fullEditUiState: { ...tab.fullEditUiState },
        },
      ]),
    ),
  };
}

function bindWorkspaceSnapshot(
  state: EditorState,
  payload: { documentKey: string; revision: number; snapshotId: SnapshotId | null | undefined },
): EditorState {
  if (!payload.documentKey || payload.snapshotId == null) return state;
  const current = state.workspace.snapshotBindingsByDocumentKey[payload.documentKey];
  if (current && payload.revision < current.revision) return state;
  const newestTabRevision = Object.values(state.workspace.tabsById).reduce(
    (newest, tab) => (tab.documentKey === payload.documentKey ? Math.max(newest, tab.revision) : newest),
    -1,
  );
  if (newestTabRevision >= 0 && payload.revision < newestTabRevision) return state;
  let tabsById = state.workspace.tabsById;
  for (const [tabId, tab] of Object.entries(state.workspace.tabsById)) {
    if (tab.documentKey !== payload.documentKey) continue;
    if (payload.revision < tab.revision) continue;
    if (tabsById === state.workspace.tabsById) tabsById = { ...tabsById };
    tabsById[tabId] = {
      ...tab,
      revision: Math.max(tab.revision, payload.revision),
      snapshotId: payload.snapshotId,
    };
  }
  return {
    ...state,
    workspace: {
      ...state.workspace,
      tabsById,
      snapshotBindingsByDocumentKey: {
        ...state.workspace.snapshotBindingsByDocumentKey,
        [payload.documentKey]: {
          documentKey: payload.documentKey,
          revision: payload.revision,
          snapshotId: payload.snapshotId,
        },
      },
    },
  };
}

function clearWorkspaceSnapshot(
  state: EditorState,
  payload: { documentKey: string; snapshotId?: SnapshotId | null },
): EditorState {
  if (!payload.documentKey) return state;
  const current = state.workspace.snapshotBindingsByDocumentKey[payload.documentKey];
  if (!current) return state;
  if (payload.snapshotId != null && current.snapshotId !== payload.snapshotId) return state;
  const snapshotBindingsByDocumentKey = { ...state.workspace.snapshotBindingsByDocumentKey };
  delete snapshotBindingsByDocumentKey[payload.documentKey];
  let tabsById = state.workspace.tabsById;
  for (const [tabId, tab] of Object.entries(state.workspace.tabsById)) {
    if (tab.documentKey !== payload.documentKey) continue;
    if (payload.snapshotId != null && tab.snapshotId !== payload.snapshotId) continue;
    if (tabsById === state.workspace.tabsById) tabsById = { ...tabsById };
    tabsById[tabId] = { ...tab, snapshotId: null };
  }
  return {
    ...state,
    workspace: {
      ...state.workspace,
      tabsById,
      snapshotBindingsByDocumentKey,
    },
  };
}

let memoizedRawWorkspace: EditorWorkspaceState | null = null;
let memoizedReadWorkspace: EditorWorkspaceState | null = null;

function cloneWorkspaceForRead(workspace: EditorWorkspaceState): EditorWorkspaceState {
  if (memoizedRawWorkspace === workspace && memoizedReadWorkspace) return memoizedReadWorkspace;
  const snapshot = deepFreezeForRead(baseCloneWorkspaceForRead(workspace));
  memoizedRawWorkspace = workspace;
  memoizedReadWorkspace = snapshot;
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

function mirrorEditorTabPayloadToPrimaryFields(state: EditorState, payload: WorkspaceEditorTabInput): EditorState {
  const tempModel = cloneTempModelForWrite(payload.tempModel ?? state.tempModel);
  const fullEditUiState = cloneFullEditUiStateForWrite(payload.fullEditUiState ?? state.fullEditUiState);
  return {
    ...state,
    sourceText: payload.sourceText,
    previousSourceText: state.sourceText,
    documentKey: payload.documentKey,
    languageId: payload.languageId,
    editorRevision: payload.revision ?? 0,
    graphAppliedRevision: payload.graphAppliedRevision ?? 0,
    tempModel,
    fullEditUiState,
  };
}

function workspaceTabToEditorTabInput(tab: EditorWorkspaceTab): WorkspaceEditorTabInput {
  return {
    id: tab.id,
    name: tab.name,
    documentKey: tab.documentKey,
    languageId: tab.languageId,
    sourceText: tab.sourceText,
    revision: tab.revision,
    graphAppliedRevision: tab.graphAppliedRevision,
    snapshotId: tab.snapshotId,
    tempModel: tab.tempModel,
    fullEditUiState: tab.fullEditUiState,
  };
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
    workspace: cloneWorkspaceForRead(state.workspace),
  };
}

function cloneFieldValue<K extends keyof EditorState>(key: K, value: EditorState[K]): EditorState[K] {
  if (key === 'workspace') return cloneWorkspaceForRead(value as EditorWorkspaceState) as EditorState[K];
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
  if (key === 'workspace') return baseCloneWorkspaceForRead(value as EditorWorkspaceState) as EditorState[K];
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
 * 创建编辑器状态管理 store，封装所有状态变更操作
 * @returns 带有 subscribe、actions 和 reset 的 store 对象
 */
function createEditorStore() {
  return {
    subscribe: (run: (value: EditorState) => void) =>
      derived(internalStore, ($s) => cloneEditorStateForRead($s)).subscribe(run),
    actions: {
      setSourceText: (text: string) =>
        updateState((s) =>
          mirrorPrimaryWorkspaceTab(
            { ...s, previousSourceText: s.sourceText, sourceText: text },
            { sourceText: text },
          ),
        ),
      setDocumentKey: (key: string) =>
        updateState((s) => mirrorPrimaryWorkspaceTab({ ...s, documentKey: key }, { documentKey: key })),
      setLanguageId: (lang: SupportedEditorLanguageId) =>
        updateState((s) => {
          const nextState = mirrorPrimaryWorkspaceTab({ ...s, languageId: lang }, { languageId: lang });
          return {
            ...nextState,
            workspace: syncSidecarLanguageFromPrimary(nextState.workspace),
          };
        }),
      incrementCompareEditToken: () => updateState((s) => ({ ...s, compareEditToken: s.compareEditToken + 1 })),
      incrementEditorRevision: () =>
        updateState((s) => {
          const editorRevision = s.editorRevision + 1;
          return mirrorPrimaryWorkspaceTab({ ...s, editorRevision }, { revision: editorRevision });
        }),
      setGraphAppliedRevision: (rev: number) =>
        updateState((s) => mirrorPrimaryWorkspaceTab({ ...s, graphAppliedRevision: rev }, { graphAppliedRevision: rev })),
      setEditorIO: (io: EditorIO | null) => updateState((s) => ({ ...s, editorIO: io })),
      emitMutation: (mutation: EditorMutation) => {
        editorMutationId += 1;
        const nextMutation = cloneEditorMutationForWrite(mutation);
        updateState((s) => ({ ...s, editorMutation: { id: editorMutationId, mutation: nextMutation } }));
      },
      clearMutation: () => updateState((s) => ({ ...s, editorMutation: null })),
      setTreeState: (treeState: TreeSyncState) =>
        updateState((s) => ({ ...s, treeState: cloneTreeStateForWrite(treeState) })),
      setFullEditUiState: (fullEditUiState: FullEditUiState) => {
        const nextFullEditUiState = cloneFullEditUiStateForWrite(fullEditUiState);
        updateState((s) => mirrorPrimaryWorkspaceTab({ ...s, fullEditUiState: nextFullEditUiState }, { fullEditUiState: nextFullEditUiState }));
      },
      setJsonBlockSelection: (jsonBlockSelection: JsonBlockSelection | null) =>
        updateState((s) => ({ ...s, jsonBlockSelection: cloneJsonBlockSelectionForWrite(jsonBlockSelection) })),
      initWorkspaceFromPrimaryTab: (payload: { id: string; name: string }) =>
        updateState((s) => ({
          ...s,
          workspace: createEditorWorkspaceState(createWorkspacePrimaryTab(s, payload)),
        })),
      addWorkspaceTabFromEditor: (payload: WorkspaceEditorTabInput) =>
        updateState((s) => ({
          ...s,
          workspace: addWorkspaceTab(s.workspace, payload),
        })),
      activateWorkspaceTabFromEditor: (payload: WorkspaceEditorTabInput) =>
        updateState((s) => {
          const withSyncedTab = syncWorkspaceEditorTab(s.workspace, payload, 'primary');
          const workspace = syncSidecarLanguageFromPrimary(activateWorkspaceTab(withSyncedTab, payload.id));
          const resolvedTab = workspace.tabsById[payload.id];
          if (!resolvedTab || resolvedTab.role === 'sidecar') {
            return {
              ...s,
              workspace,
            };
          }
          return {
            ...mirrorEditorTabPayloadToPrimaryFields(s, workspaceTabToEditorTabInput(resolvedTab)),
            workspace,
          };
        }),
      closeWorkspaceTabFromEditor: (tabId: string, fallback?: WorkspaceEditorTabInput) =>
        updateState((s) => {
          const previousActiveTabId = s.workspace.activeTabId;
          const result = closeWorkspaceTab(s.workspace, tabId, fallback);
          if (result.workspace === s.workspace) return s;
          const workspace = syncSidecarLanguageFromPrimary(result.workspace);
          if (result.nextActiveTabId === previousActiveTabId) {
            return {
              ...s,
              workspace,
            };
          }
          const nextPrimary =
            result.nextActiveTabId != null ? workspace.tabsById[result.nextActiveTabId] : null;
          if (!nextPrimary || nextPrimary.role === 'sidecar') {
            return {
              ...s,
              workspace,
            };
          }
          return {
            ...mirrorEditorTabPayloadToPrimaryFields(s, workspaceTabToEditorTabInput(nextPrimary)),
            workspace,
          };
        }),
      getWorkspaceTabSummaries: () => summarizeWorkspaceTabs(get(internalStore).workspace),
      ensureSidecarWorkspaceTab: (payload: { id: string; name: string; sourceText: string }) =>
        updateState((s) => ({
          ...s,
          workspace: ensureSidecarTab(s.workspace, {
            id: payload.id,
            name: payload.name,
            languageId: s.languageId,
            sourceText: payload.sourceText,
          }),
        })),
      ensureDetachedSidecarWorkspaceTab: (payload: { id: string; name: string; sourceText: string }) =>
        updateState((s) => ({
          ...s,
          workspace: ensureDetachedSidecarTab(s.workspace, {
            id: payload.id,
            name: payload.name,
            languageId: s.languageId,
            sourceText: payload.sourceText,
          }),
        })),
      removeDetachedSidecarWorkspaceTab: (tabId: string) =>
        updateState((s) => ({
          ...s,
          workspace: removeDetachedSidecarTab(s.workspace, tabId),
        })),
      updateWorkspaceTab: (tabId: string, patch: EditorWorkspaceTabPatch) =>
        updateState((s) => {
          if (tabId === s.workspace.primaryTabId) return s;
          const isSidecarTab = s.workspace.tabsById[tabId]?.role === 'sidecar';
          const { languageId: _ignoredLanguageId, ...patchWithoutLanguage } = patch;
          const safePatch = isSidecarTab ? patch : patchWithoutLanguage;
          return {
            ...s,
            workspace: patchWorkspaceTab(s.workspace, tabId, safePatch),
          };
        }),
      bindWorkspaceSnapshot: (payload: { documentKey: string; revision: number; snapshotId: SnapshotId | null | undefined }) =>
        updateState((s) => bindWorkspaceSnapshot(s, payload)),
      clearWorkspaceSnapshot: (documentKey: string, snapshotId?: SnapshotId | null) =>
        updateState((s) => clearWorkspaceSnapshot(s, { documentKey, snapshotId })),
      getWorkspaceSnapshotId: (documentKey: string): SnapshotId | null => {
        if (!documentKey) return null;
        const state = get(internalStore);
        return state.workspace.snapshotBindingsByDocumentKey[documentKey]?.snapshotId ?? null;
      },
      syncSidecarLanguageFromPrimary: () =>
        updateState((s) => ({
          ...s,
          workspace: syncSidecarLanguageFromPrimary(s.workspace),
        })),
      clearJsonBlockSelectionForDocument: (documentKey: string) =>
        updateState((s) =>
          s.jsonBlockSelection?.sourceDocumentKey === documentKey ? { ...s, jsonBlockSelection: null } : s,
        ),
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
          return setPrimaryFullEditUiState(s, initialFullEditState);
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
        updateState((s) =>
          updateOwnedFullEditUiState(s, payload, (current) =>
            current.phase === 'streaming' ? { ...current, phase: 'finalizing' } : null,
          ),
        ),
      markFullEditStreamSettled: (payload: FullEditOwnerPayload) =>
        updateState((s) =>
          updateOwnedFullEditUiState(s, payload, (current) =>
            current.phase === 'finalizing' ? { ...current, phase: 'settled' } : null,
          ),
        ),
      completeFullEditStreamUi: (payload: FullEditOwnerPayload) =>
        updateState((s) => updateOwnedFullEditUiState(s, payload, (current) => ({ ...current, phase: 'idle' }))),
      finishFullEditStream: (payload: FullEditOwnerPayload) =>
        updateState((s) => updateOwnedFullEditUiState(s, payload, () => initialFullEditState)),
      cancelFullEditStream: (payload: FullEditOwnerPayload) =>
        updateState((s) => updateOwnedFullEditUiState(s, payload, () => initialFullEditState)),
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
      editorMutationId = 0;
      internalStore.set(initialEditorState);
    },
    get: (): EditorState => {
      const state = get(internalStore);
      return cloneEditorStateForRead(state);
    },
  };
}

export const editorStore = createEditorStore();

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
      const current = cloneFieldValueForUpdate(key, get(internalStore)[key]);
      setter(fn(current));
    },
  };
}

export const sourceText = createFieldStore('sourceText', editorStore.actions.setSourceText);
export const previousSourceText = createFieldStore('previousSourceText', editorStore.actions.setSourceText as any);
export const documentKey = createFieldStore('documentKey', editorStore.actions.setDocumentKey);
export const languageId = createFieldStore('languageId', editorStore.actions.setLanguageId);
export const compareEditToken: Writable<number> = {
  subscribe: (run) => derived(internalStore, ($s) => $s.compareEditToken).subscribe(run),
  set: (value) => updateState((s) => ({ ...s, compareEditToken: value })),
  update: (fn) => {
    const current = get(internalStore).compareEditToken;
    updateState((s) => ({ ...s, compareEditToken: fn(current) }));
  },
};
export const editorRevision: Writable<number> = {
  subscribe: (run) => derived(internalStore, ($s) => $s.editorRevision).subscribe(run),
  set: (value) =>
    updateState((s) => mirrorPrimaryWorkspaceTab({ ...s, editorRevision: value }, { revision: value })),
  update: (fn) => {
    const current = get(internalStore).editorRevision;
    const next = fn(current);
    updateState((s) => mirrorPrimaryWorkspaceTab({ ...s, editorRevision: next }, { revision: next }));
  },
};
export const graphAppliedRevision = createFieldStore(
  'graphAppliedRevision',
  editorStore.actions.setGraphAppliedRevision,
);
export const editorIO = createFieldStore('editorIO', editorStore.actions.setEditorIO);
export const editorMutation: Readable<EditorMutationEnvelope | null> = {
  subscribe: (run) => {
    let initialized = false;
    let currentRaw: EditorMutationEnvelope | null | undefined;
    return internalStore.subscribe(($s) => {
      const nextRaw = $s.editorMutation;
      if (initialized && Object.is(nextRaw, currentRaw)) return;
      initialized = true;
      currentRaw = nextRaw;
      run(cloneEditorMutationForRead(nextRaw));
    });
  },
};
export const treeState: Writable<TreeSyncState> = {
  subscribe: (run) => {
    let initialized = false;
    let currentRaw: TreeSyncState | null = null;
    return internalStore.subscribe(($s) => {
      if (initialized && $s.treeState === currentRaw) return;
      initialized = true;
      currentRaw = $s.treeState;
      run(cloneTreeStateForRead($s.treeState));
    });
  },
  set: editorStore.actions.setTreeState,
  update: (fn) => {
    const current = cloneTreeStateForWrite(get(internalStore).treeState);
    editorStore.actions.setTreeState(fn(current));
  },
};
export const fullEditUiState = createFieldStore('fullEditUiState', editorStore.actions.setFullEditUiState);
export const jsonBlockSelection = createFieldStore(
  'jsonBlockSelection',
  editorStore.actions.setJsonBlockSelection,
);
export const activeTempModel = createFieldStore('tempModel', editorStore.actions.setTempModel);
export const editorWorkspace: Readable<EditorWorkspaceState> = {
  subscribe: (run) => {
    let initialized = false;
    let currentRaw: EditorWorkspaceState | undefined;
    return internalStore.subscribe(($s) => {
      const nextRaw = $s.workspace;
      if (initialized && Object.is(nextRaw, currentRaw)) return;
      initialized = true;
      currentRaw = nextRaw;
      run(cloneWorkspaceForRead(nextRaw));
    });
  },
};
