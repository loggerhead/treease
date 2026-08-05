import type { SnapshotId } from '@core-wasm/index';
import type { SupportedEditorLanguageId } from '../monaco/language-support';
import type { DiagnosticItem, TempModel } from './diagnostics-store';
import type { FullEditUiState } from './full-edit-ui-state';
import type { GraphHighlightState } from './graph-selection-store';
import type { PathSeg } from './tree-path';
import type { DocumentOrigin } from '../document-origin';

export type EditorPaneId = 'left' | 'right';
export type EditorWorkspaceTabRole = 'primary' | 'sidecar' | 'background' | 'column-detail-draft';
type EditorWorkspaceMainTabRole = 'primary' | 'background';

export type SidecarPaneState = {
  surfaceMode: 'graph' | 'compare';
  graph: {
    viewport: { x: number; y: number; scaleX: number; scaleY: number } | null;
  };
  navigator: {
    activePath: PathSeg[];
    history: PathSeg[][];
    historyIndex: number;
    collapsed: boolean;
    expanded: boolean;
    columnsMaterialized: boolean;
  };
  compare: {
    scrollTop: number;
    scrollLeft: number;
    outcome: CompareOutcomeState;
  };
};

export type CompareOutcomeState =
  | { kind: 'none' }
  | { kind: 'equal'; mode: 'tree' | 'text' }
  | { kind: 'different'; mode: 'tree' | 'text' };

/** Remote persistence state; intentionally independent of local dirty state. */
export type CloudSyncStatus = 'synced' | 'syncing' | 'pending' | 'error' | 'offline';

/** Opaque host-owned reference to a user-selected local file. */
export type FileLinkedDocument = {
  grantId: string;
  name: string;
};

export type EditorWorkspaceTab = {
  id: string;
  role: EditorWorkspaceTabRole;
  /** A left tab and this tab's right-side state form one inseparable tab pair. */
  sidecarTabId?: string;
  /** Present only on paired sidecars. */
  ownerMainTabId?: string;
  /** Present only on paired sidecars; every field is right-pane local. */
  sidecarState?: SidecarPaneState;
  name: string;
  documentKey: string;
  /** Increments whenever this tab entity receives a replacement document. */
  generation?: number;
  languageId: SupportedEditorLanguageId;
  sourceText: string;
  origin?: DocumentOrigin;
  revision: number;
  graphAppliedRevision: number;
  snapshotId: SnapshotId | null;
  tempModel: TempModel;
  fullEditUiState: FullEditUiState;
  fileLinkedDocument?: FileLinkedDocument;
  savedText?: string;
  syncStatus?: CloudSyncStatus;
};

export type EditorWorkspaceState = {
  tabsById: Record<string, EditorWorkspaceTab>;
  primaryTabId: string;
  activeTabId: string;
  tabOrder: string[];
  paneTabIds: Record<EditorPaneId, string | null>;
  snapshotBindingsByDocumentKey: Record<string, WorkspaceSnapshotBinding>;
};

export type WorkspaceSnapshotBinding = {
  documentKey: string;
  revision: number;
  snapshotId: SnapshotId;
};

export type EditorWorkspaceTabSummary = {
  id: string;
  name: string;
  languageId: SupportedEditorLanguageId;
  dirty: boolean;
  syncStatus?: CloudSyncStatus;
};

export type WorkspaceEditorTabInput = {
  id: string;
  name: string;
  documentKey: string;
  generation?: number;
  languageId: SupportedEditorLanguageId;
  sourceText: string;
  origin?: DocumentOrigin;
  revision?: number;
  graphAppliedRevision?: number;
  snapshotId?: SnapshotId | null;
  tempModel?: TempModel;
  fullEditUiState?: FullEditUiState;
  fileLinkedDocument?: FileLinkedDocument;
  savedText?: string;
  syncStatus?: CloudSyncStatus;
};

export type TabTopologyEffect =
  | { kind: 'activate-existing'; tabId: string; disposeTabId?: string }
  | { kind: 'activate-new-blank'; tabId: string; documentKey: string; disposeTabId: string };

export type TabTopologyTransition = {
  workspace: EditorWorkspaceState;
  effect: TabTopologyEffect;
};

export type EditorWorkspaceTabPatch = {
  name?: string;
  documentKey?: string;
  languageId?: SupportedEditorLanguageId;
  sourceText?: string;
  origin?: DocumentOrigin;
  revision?: number;
  graphAppliedRevision?: number;
  snapshotId?: SnapshotId | null;
  tempModel?: TempModel;
  fullEditUiState?: FullEditUiState;
  fileLinkedDocument?: FileLinkedDocument;
  savedText?: string;
  syncStatus?: CloudSyncStatus;
  sidecarState?: SidecarPaneState;
};

/** Verifies the explicit one-to-one topology; ids and tab order are never used as pairing evidence. */
export function hasValidTabPairs(workspace: EditorWorkspaceState): boolean {
  for (const mainId of workspace.tabOrder) {
    const main = workspace.tabsById[mainId];
    const sidecar = main?.sidecarTabId ? workspace.tabsById[main.sidecarTabId] : null;
    if (!isMainWorkspaceTab(main) || !sidecar || sidecar.role !== 'sidecar' || sidecar.ownerMainTabId !== main.id) return false;
  }
  return Object.values(workspace.tabsById).every((tab) => tab.role !== 'sidecar'
    || (Boolean(tab.ownerMainTabId) && workspace.tabsById[tab.ownerMainTabId!]?.sidecarTabId === tab.id && !workspace.tabOrder.includes(tab.id)));
}

function isMainWorkspaceTab(tab: EditorWorkspaceTab | undefined): tab is EditorWorkspaceTab {
  return tab?.role === 'primary' || tab?.role === 'background';
}

/**
 * The only pure transition allowed to replace a left document's identity.
 * Ordinary workspace patches intentionally cannot alter documentKey or an
 * inactive left tab's language, because those fields bind Document Runtime
 * results to a specific document generation.
 */
export type TargetDocumentTransition = {
  tabId: string;
  expected: Pick<EditorWorkspaceTab, 'documentKey' | 'languageId' | 'revision'>;
  next: Pick<EditorWorkspaceTab, 'documentKey' | 'languageId' | 'revision' | 'sourceText'>;
};

function sidecarDocumentKey(tabId: string, generation: string | number = 0): string {
  return `sidecar:${tabId}:${generation}`;
}

function pairedSidecarTabId(mainTabId: string): string {
  return `${mainTabId}:sidecar`;
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

function cloneTempModel(tempModel: TempModel): TempModel {
  return {
    ...tempModel,
    treePath: clonePathSegs(tempModel.treePath),
    graphHighlight: cloneGraphHighlight(tempModel.graphHighlight),
    diagnostics: cloneDiagnostics(tempModel.diagnostics),
  };
}

function createInitialSidecarPaneState(): SidecarPaneState {
  return {
    surfaceMode: 'graph',
    graph: { viewport: null },
    navigator: { activePath: [], history: [], historyIndex: -1, collapsed: false, expanded: false, columnsMaterialized: false },
    compare: { scrollTop: 0, scrollLeft: 0, outcome: { kind: 'none' } },
  };
}

function cloneSidecarPaneState(state: SidecarPaneState | undefined): SidecarPaneState | undefined {
  if (!state) return undefined;
  return {
    ...state,
    navigator: {
      ...state.navigator,
      activePath: clonePathSegs(state.navigator.activePath),
      history: state.navigator.history.map(clonePathSegs),
    },
    graph: { viewport: state.graph.viewport ? { ...state.graph.viewport } : null },
    compare: { ...state.compare },
  };
}

function cloneFullEditUiState(fullEditUiState: FullEditUiState): FullEditUiState {
  return { ...fullEditUiState };
}

function cloneTab(tab: EditorWorkspaceTab): EditorWorkspaceTab {
  return {
    ...tab,
    sidecarState: cloneSidecarPaneState(tab.sidecarState),
    tempModel: cloneTempModel(tab.tempModel),
    fullEditUiState: cloneFullEditUiState(tab.fullEditUiState),
  };
}

function appendTabOrder(tabOrder: string[], tabId: string): string[] {
  return tabOrder.includes(tabId) ? tabOrder : [...tabOrder, tabId];
}

function assignLeftTabRoles(
  tabsById: Record<string, EditorWorkspaceTab>,
  tabOrder: string[],
  primaryTabId: string | null,
): void {
  for (const leftTabId of tabOrder) {
    const tab = tabsById[leftTabId];
    if (!isMainWorkspaceTab(tab)) continue;
    const role = leftTabId === primaryTabId ? 'primary' : 'background';
    if (tab.role !== role) {
      tabsById[leftTabId] = {
        ...tab,
        role,
      };
    }
  }
}

function createCleanTempModel(sourceText: string): TempModel {
  return {
    diffInputText: '',
    scratchText: sourceText,
    commandQuery: '',
    status: 'Ready',
    error: '',
    cursor: 'Ln 1, Col 1',
    selectionLength: 0,
    treePath: [],
    graphHighlight: null,
    diagnostics: [],
  };
}

function createInactiveFullEditUiState(): FullEditUiState {
  return {
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
}

function createEditorTabFromInput(
  input: WorkspaceEditorTabInput,
  role: EditorWorkspaceMainTabRole,
  existing?: EditorWorkspaceTab,
): EditorWorkspaceTab {
  return {
    id: input.id,
    role,
    name: input.name,
    documentKey: input.documentKey,
    generation: input.generation ?? existing?.generation ?? 0,
    languageId: input.languageId,
    sourceText: input.sourceText,
    origin: input.origin ?? existing?.origin ?? 'user',
    revision: input.revision ?? existing?.revision ?? 0,
    graphAppliedRevision: input.graphAppliedRevision ?? existing?.graphAppliedRevision ?? 0,
    snapshotId: input.snapshotId !== undefined ? input.snapshotId : existing?.snapshotId ?? null,
    tempModel: input.tempModel
      ? cloneTempModel(input.tempModel)
      : existing
        ? cloneTempModel(existing.tempModel)
        : createCleanTempModel(input.sourceText),
    fullEditUiState: input.fullEditUiState
      ? cloneFullEditUiState(input.fullEditUiState)
      : existing
        ? cloneFullEditUiState(existing.fullEditUiState)
        : createInactiveFullEditUiState(),
    fileLinkedDocument: input.fileLinkedDocument ?? existing?.fileLinkedDocument,
    savedText: input.savedText ?? existing?.savedText,
    syncStatus: input.syncStatus ?? existing?.syncStatus,
    sidecarState: cloneSidecarPaneState(existing?.sidecarState),
  };
}

function createPairedSidecar(mainTab: EditorWorkspaceTab, id: string): EditorWorkspaceTab {
  return {
    id,
    role: 'sidecar',
    ownerMainTabId: mainTab.id,
    sidecarState: createInitialSidecarPaneState(),
    name: `${mainTab.name} sidecar`,
    documentKey: sidecarDocumentKey(id),
    languageId: mainTab.languageId,
    // This is compare input only. The paired main tab remains the sole source-text authority.
    sourceText: '',
    origin: 'user',
    revision: 0,
    graphAppliedRevision: 0,
    snapshotId: null,
    tempModel: createCleanTempModel(''),
    fullEditUiState: createInactiveFullEditUiState(),
    fileLinkedDocument: undefined,
    savedText: undefined,
  };
}

function pairMainTab(
  tabsById: Record<string, EditorWorkspaceTab>,
  mainTab: EditorWorkspaceTab,
): Record<string, EditorWorkspaceTab> {
  const sidecarId = mainTab.sidecarTabId ?? pairedSidecarTabId(mainTab.id);
  const sidecar = tabsById[sidecarId];
  if (sidecar && sidecar.role === 'sidecar' && sidecar.ownerMainTabId === mainTab.id) {
    if (mainTab.sidecarTabId === sidecarId) return tabsById;
    return { ...tabsById, [mainTab.id]: { ...mainTab, sidecarTabId: sidecarId } };
  }
  const pairedMain = { ...mainTab, sidecarTabId: sidecarId };
  const createdSidecar = createPairedSidecar(pairedMain, sidecarId);
  return { ...tabsById, [pairedMain.id]: pairedMain, [createdSidecar.id]: createdSidecar };
}

export function createEditorWorkspaceState(primaryTab: EditorWorkspaceTab): EditorWorkspaceState {
  const unpairedPrimary: EditorWorkspaceTab = {
    ...cloneTab(primaryTab),
    role: 'primary',
    sidecarTabId: undefined,
    ownerMainTabId: undefined,
    sidecarState: undefined,
  };
  const tabsById = pairMainTab({ [unpairedPrimary.id]: unpairedPrimary }, unpairedPrimary);
  const normalizedPrimary = tabsById[unpairedPrimary.id]!;
  return {
    tabsById,
    primaryTabId: normalizedPrimary.id,
    activeTabId: normalizedPrimary.id,
    tabOrder: [normalizedPrimary.id],
    paneTabIds: {
      left: normalizedPrimary.id,
      right: normalizedPrimary.sidecarTabId ?? null,
    },
    snapshotBindingsByDocumentKey: {},
  };
}

export function reinitializeWorkspaceFromPrimaryTab(
  workspace: EditorWorkspaceState,
  primaryTab: EditorWorkspaceTab,
): EditorWorkspaceState {
  // Reinitialization is a new workspace topology, not a way to retain a
  // previous document's right-side state. Its fresh primary gets a fresh pair.
  return createEditorWorkspaceState(primaryTab);
}

export function syncWorkspaceEditorTab(
  workspace: EditorWorkspaceState,
  input: WorkspaceEditorTabInput,
  role: EditorWorkspaceMainTabRole,
): EditorWorkspaceState {
  const existing = workspace.tabsById[input.id];
  if (existing && !isMainWorkspaceTab(existing)) return workspace;
  const effectiveRole = input.id === workspace.primaryTabId || input.id === workspace.activeTabId ? 'primary' : role;
  const nextTabOrder = appendTabOrder(workspace.tabOrder, input.id);
  const nextTabsById = {
    ...workspace.tabsById,
    [input.id]: createEditorTabFromInput(input, effectiveRole, existing),
  };
  if (effectiveRole !== 'primary') {
    return {
      ...workspace,
      tabsById: nextTabsById,
      tabOrder: nextTabOrder,
    };
  }
  assignLeftTabRoles(nextTabsById, nextTabOrder, input.id);
  return {
    ...workspace,
    tabsById: nextTabsById,
    primaryTabId: input.id,
    activeTabId: input.id,
    tabOrder: nextTabOrder,
    paneTabIds: {
      ...workspace.paneTabIds,
      left: input.id,
    },
  };
}

export function addWorkspaceTab(workspace: EditorWorkspaceState, input: WorkspaceEditorTabInput): EditorWorkspaceState {
  if (workspace.tabsById[input.id] && !isMainWorkspaceTab(workspace.tabsById[input.id])) return workspace;
  const existing = workspace.tabsById[input.id];
  const mainTab = createEditorTabFromInput(input, existing?.role === 'primary' ? 'primary' : 'background', existing);
  const tabsById = pairMainTab({ ...workspace.tabsById, [input.id]: mainTab }, mainTab);
  return {
    ...workspace,
    tabsById,
    tabOrder: appendTabOrder(workspace.tabOrder, input.id),
  };
}

/** Create and select a new left document in one topology transition. */
export function createWorkspaceTabTransition(
  workspace: EditorWorkspaceState,
  input: WorkspaceEditorTabInput,
): TabTopologyTransition | null {
  if (workspace.tabsById[input.id]) return null;
  const withTab = addWorkspaceTab(workspace, input);
  return {
    workspace: activateWorkspaceTab(withTab, input.id),
    effect: { kind: 'activate-existing', tabId: input.id },
  };
}

/** Select an existing left tab. Sidecars and unknown ids have no transition. */
export function activateWorkspaceTabTransition(
  workspace: EditorWorkspaceState,
  tabId: string,
): TabTopologyTransition | null {
  const target = workspace.tabsById[tabId];
  if (!isMainWorkspaceTab(target)) return null;
  return {
    workspace: activateWorkspaceTab(workspace, tabId),
    effect: { kind: 'activate-existing', tabId },
  };
}

export function activateWorkspaceTab(workspace: EditorWorkspaceState, tabId: string): EditorWorkspaceState {
  const target = workspace.tabsById[tabId];
  if (!isMainWorkspaceTab(target)) return workspace;
  const pairedSidecarId = target.sidecarTabId;
  const pairedSidecar = pairedSidecarId ? workspace.tabsById[pairedSidecarId] : null;
  if (!pairedSidecarId || !pairedSidecar || pairedSidecar.role !== 'sidecar' || pairedSidecar.ownerMainTabId !== tabId) return workspace;
  if (
    workspace.primaryTabId === tabId &&
    workspace.activeTabId === tabId &&
    workspace.paneTabIds.left === tabId &&
    workspace.paneTabIds.right === pairedSidecarId
  ) return workspace;
  const nextTabsById = { ...workspace.tabsById };
  assignLeftTabRoles(nextTabsById, workspace.tabOrder, tabId);
  return {
    ...workspace,
    tabsById: nextTabsById,
    primaryTabId: tabId,
    activeTabId: tabId,
    paneTabIds: {
      ...workspace.paneTabIds,
      left: tabId,
      right: pairedSidecarId,
    },
  };
}

/**
 * Pure left-tab topology transition. Closing the last tab is a product action:
 * it replaces that document with a newly identified empty primary document.
 */
export function closeWorkspaceTabTransition(
  workspace: EditorWorkspaceState,
  tabId: string,
  blank: { id: string; documentKey: string; name: string; languageId: SupportedEditorLanguageId },
): TabTopologyTransition | null {
  const closedTab = workspace.tabsById[tabId];
  if (!isMainWorkspaceTab(closedTab)) return null;
  const closedIndex = workspace.tabOrder.indexOf(tabId);
  if (closedIndex < 0) return null;
  let nextTabOrder = workspace.tabOrder.filter((id) => id !== tabId);
  if (nextTabOrder.length === 0) {
    if (workspace.tabsById[blank.id] || workspace.paneTabIds.right === blank.id) return null;
  }
  const nextTabsById = { ...workspace.tabsById };
  delete nextTabsById[tabId];
  if (closedTab.sidecarTabId) delete nextTabsById[closedTab.sidecarTabId];
  if (nextTabOrder.length === 0) {
    const blankTab = createEditorTabFromInput({ ...blank, sourceText: '', origin: 'user' }, 'primary');
    const pairedTabs = pairMainTab({ ...nextTabsById, [blank.id]: blankTab }, blankTab);
    const pairedBlank = pairedTabs[blank.id]!;
    nextTabOrder = [blank.id];
    const nextWorkspace = {
      ...workspace,
      tabsById: pairedTabs,
      primaryTabId: blank.id,
      activeTabId: blank.id,
      tabOrder: nextTabOrder,
      paneTabIds: { ...workspace.paneTabIds, left: blank.id, right: pairedBlank.sidecarTabId ?? null },
      snapshotBindingsByDocumentKey: Object.fromEntries(
        Object.entries(workspace.snapshotBindingsByDocumentKey).filter(([key]) => key !== closedTab.documentKey),
      ),
    };
    return { workspace: nextWorkspace, effect: { kind: 'activate-new-blank', tabId: blank.id, documentKey: blank.documentKey, disposeTabId: tabId } };
  }
  const wasActive = workspace.primaryTabId === tabId || workspace.activeTabId === tabId || workspace.paneTabIds.left === tabId;
  const nextActiveTabId = wasActive
    ? nextTabOrder[Math.max(0, Math.min(closedIndex - 1, nextTabOrder.length - 1))] ?? null
    : workspace.activeTabId;
  assignLeftTabRoles(nextTabsById, nextTabOrder, nextActiveTabId);
  const nextWorkspace: EditorWorkspaceState = {
    ...workspace,
    tabsById: nextTabsById,
    primaryTabId: nextActiveTabId ?? '',
    activeTabId: nextActiveTabId ?? '',
    tabOrder: nextTabOrder,
    paneTabIds: {
      ...workspace.paneTabIds,
      left: nextActiveTabId,
    },
  };
  const snapshotBindingsByDocumentKey = Object.fromEntries(
    Object.entries(nextWorkspace.snapshotBindingsByDocumentKey).filter(([key]) => key !== closedTab.documentKey),
  );
  return {
    workspace: { ...nextWorkspace, snapshotBindingsByDocumentKey },
    effect: wasActive
      ? { kind: 'activate-existing', tabId: nextActiveTabId!, disposeTabId: tabId }
      : { kind: 'activate-existing', tabId: workspace.activeTabId, disposeTabId: tabId },
  };
}

export function summarizeWorkspaceTabs(workspace: EditorWorkspaceState): EditorWorkspaceTabSummary[] {
  return workspace.tabOrder
    .map((tabId) => workspace.tabsById[tabId])
    .filter((tab): tab is EditorWorkspaceTab => isMainWorkspaceTab(tab))
    .map((tab) => ({
      id: tab.id,
      name: tab.name,
      languageId: tab.languageId,
      dirty: isWorkspaceTabDirty(tab),
      ...(tab.syncStatus ? { syncStatus: tab.syncStatus } : {}),
    }));
}

export function isWorkspaceTabDirty(tab: EditorWorkspaceTab): boolean {
  return tab.savedText !== undefined && tab.sourceText !== tab.savedText;
}

/**
 * Column Detail drafts use a tab-shaped Monaco backing store, but are not a
 * paired right-sidecar. They project and commit through the main document.
 */
export function ensureColumnDetailDraftTab(
  workspace: EditorWorkspaceState,
  input: {
    id: string;
    name: string;
    languageId: SupportedEditorLanguageId;
    sourceText: string;
  },
): EditorWorkspaceState {
  const existing = workspace.tabsById[input.id];
  if (existing?.role === 'column-detail-draft') return workspace;
  if (existing) return workspace;
  const sidecar: EditorWorkspaceTab = {
    id: input.id,
    role: 'column-detail-draft',
    name: input.name,
    documentKey: sidecarDocumentKey(input.id),
    languageId: input.languageId,
    sourceText: input.sourceText,
    revision: 0,
    graphAppliedRevision: 0,
    snapshotId: null,
    tempModel: createCleanTempModel(input.sourceText),
    fullEditUiState: createInactiveFullEditUiState(),
    fileLinkedDocument: undefined,
    savedText: undefined,
  };
  return {
    ...workspace,
    tabsById: {
      ...workspace.tabsById,
      [sidecar.id]: sidecar,
    },
  };
}

export function removeColumnDetailDraftTab(workspace: EditorWorkspaceState, tabId: string): EditorWorkspaceState {
  const current = workspace.tabsById[tabId];
  if (!current || current.role !== 'column-detail-draft') return workspace;
  const nextTabsById = { ...workspace.tabsById };
  delete nextTabsById[tabId];
  return {
    ...workspace,
    tabsById: nextTabsById,
    paneTabIds:
      workspace.paneTabIds.right === tabId
        ? {
            ...workspace.paneTabIds,
            right: null,
          }
        : workspace.paneTabIds,
  };
}

export function updateWorkspaceTab(
  workspace: EditorWorkspaceState,
  tabId: string,
  patch: EditorWorkspaceTabPatch,
): EditorWorkspaceState {
  const current = workspace.tabsById[tabId];
  if (!current) return workspace;
  const nextTempModel = patch.tempModel ? cloneTempModel(patch.tempModel) : current.tempModel;
  const nextFullEditUiState = patch.fullEditUiState ? cloneFullEditUiState(patch.fullEditUiState) : current.fullEditUiState;
  const nextSnapshotId = patch.snapshotId !== undefined ? patch.snapshotId : current.snapshotId;
  const nextTab: EditorWorkspaceTab = {
    id: current.id,
    role: current.role,
    sidecarTabId: current.sidecarTabId,
    ownerMainTabId: current.ownerMainTabId,
    name: patch.name ?? current.name,
    documentKey: current.documentKey,
    languageId: patch.languageId ?? current.languageId,
    sourceText: patch.sourceText ?? current.sourceText,
    origin: patch.origin ?? current.origin,
    revision: patch.revision ?? current.revision,
    graphAppliedRevision: patch.graphAppliedRevision ?? current.graphAppliedRevision,
    snapshotId: nextSnapshotId,
    tempModel: nextTempModel,
    fullEditUiState: nextFullEditUiState,
    fileLinkedDocument: patch.fileLinkedDocument ?? current.fileLinkedDocument,
    savedText: patch.savedText ?? current.savedText,
    syncStatus: patch.syncStatus ?? current.syncStatus,
    sidecarState: patch.sidecarState ? cloneSidecarPaneState(patch.sidecarState) : cloneSidecarPaneState(current.sidecarState),
  };
  return {
    ...workspace,
    tabsById: {
      ...workspace.tabsById,
      [tabId]: nextTab,
    },
  };
}

export function transitionWorkspaceTabDocument(
  workspace: EditorWorkspaceState,
  transition: TargetDocumentTransition,
): EditorWorkspaceState | null {
  const current = workspace.tabsById[transition.tabId];
  if (
    !current ||
    !isMainWorkspaceTab(current) ||
    current.documentKey !== transition.expected.documentKey ||
    current.languageId !== transition.expected.languageId ||
    current.revision !== transition.expected.revision ||
    (current.documentKey === transition.next.documentKey &&
      current.languageId === transition.next.languageId &&
      current.sourceText === transition.next.sourceText &&
      current.revision === transition.next.revision)
  ) {
    return null;
  }

  const nextTab: EditorWorkspaceTab = {
    ...current,
    documentKey: transition.next.documentKey,
    generation: (current.generation ?? 0) + 1,
    languageId: transition.next.languageId,
    sourceText: transition.next.sourceText,
    revision: transition.next.revision,
    graphAppliedRevision: Math.min(current.graphAppliedRevision, Math.max(0, transition.next.revision - 1)),
    snapshotId: null,
  };
  const snapshotBindingsByDocumentKey = { ...workspace.snapshotBindingsByDocumentKey };
  delete snapshotBindingsByDocumentKey[current.documentKey];
  const sidecar = current.sidecarTabId ? workspace.tabsById[current.sidecarTabId] : null;
  const nextSidecar = sidecar?.role === 'sidecar' && sidecar.ownerMainTabId === current.id
    ? {
        ...sidecar,
        documentKey: sidecarDocumentKey(sidecar.id, `${transition.next.documentKey}:${transition.next.revision}`),
        generation: (sidecar.generation ?? 0) + 1,
        languageId: transition.next.languageId,
        sourceText: '',
        revision: 0,
        graphAppliedRevision: 0,
        snapshotId: null,
        tempModel: createCleanTempModel(''),
        fullEditUiState: createInactiveFullEditUiState(),
        sidecarState: createInitialSidecarPaneState(),
      }
    : null;

  return {
    ...workspace,
    tabsById: {
      ...workspace.tabsById,
      [current.id]: nextTab,
      ...(nextSidecar ? { [nextSidecar.id]: nextSidecar } : {}),
    },
    snapshotBindingsByDocumentKey,
  };
}
