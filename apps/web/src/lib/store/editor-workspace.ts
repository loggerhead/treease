import type { SnapshotId } from '@core-wasm/index';
import type { SupportedEditorLanguageId } from '../monaco/language-support';
import type { DiagnosticItem, TempModel } from './diagnostics-store';
import type { FullEditUiState } from './full-edit-ui-state';
import type { GraphHighlightState } from './graph-selection-store';
import type { PathSeg } from './tree-path';
import type { DocumentOrigin } from '../document-origin';

export type EditorPaneId = 'left' | 'right';
export type EditorWorkspaceTabRole = 'primary' | 'sidecar' | 'background';

/** Opaque host-owned reference to a user-selected local file. */
export type FileLinkedDocument = {
  grantId: string;
  name: string;
};

export type EditorWorkspaceTab = {
  id: string;
  role: EditorWorkspaceTabRole;
  name: string;
  documentKey: string;
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

export type EditorWorkspaceTabSummary = { id: string; name: string; languageId: SupportedEditorLanguageId; dirty: boolean };

export type WorkspaceEditorTabInput = {
  id: string;
  name: string;
  documentKey: string;
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
};

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

function sidecarDocumentKey(tabId: string): string {
  return `sidecar:${tabId}:0`;
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

function cloneFullEditUiState(fullEditUiState: FullEditUiState): FullEditUiState {
  return { ...fullEditUiState };
}

function cloneTab(tab: EditorWorkspaceTab): EditorWorkspaceTab {
  return {
    ...tab,
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
    if (!tab || tab.role === 'sidecar') continue;
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
  role: Exclude<EditorWorkspaceTabRole, 'sidecar'>,
  existing?: EditorWorkspaceTab,
): EditorWorkspaceTab {
  return {
    id: input.id,
    role,
    name: input.name,
    documentKey: input.documentKey,
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
  };
}

export function createEditorWorkspaceState(primaryTab: EditorWorkspaceTab): EditorWorkspaceState {
  const normalizedPrimary: EditorWorkspaceTab = {
    ...cloneTab(primaryTab),
    role: 'primary',
  };
  return {
    tabsById: {
      [normalizedPrimary.id]: normalizedPrimary,
    },
    primaryTabId: normalizedPrimary.id,
    activeTabId: normalizedPrimary.id,
    tabOrder: [normalizedPrimary.id],
    paneTabIds: {
      left: normalizedPrimary.id,
      right: null,
    },
    snapshotBindingsByDocumentKey: {},
  };
}

export function reinitializeWorkspaceFromPrimaryTab(
  workspace: EditorWorkspaceState,
  primaryTab: EditorWorkspaceTab,
): EditorWorkspaceState {
  const nextWorkspace = createEditorWorkspaceState(primaryTab);
  const keptTabs = Object.values(workspace.tabsById)
    .filter((tab): tab is EditorWorkspaceTab => tab.role === 'sidecar')
    .map((tab) => cloneTab(tab));

  if (keptTabs.length === 0) {
    return nextWorkspace;
  }

  const tabsById = { ...nextWorkspace.tabsById };
  for (const sidecarTab of keptTabs) {
    tabsById[sidecarTab.id] = sidecarTab;
  }

  const retainedDocumentKeys = new Set([
    nextWorkspace.tabsById[nextWorkspace.primaryTabId]?.documentKey,
    ...keptTabs.map((tab) => tab.documentKey),
  ]);
  const snapshotBindingsByDocumentKey = Object.fromEntries(
    Object.entries(workspace.snapshotBindingsByDocumentKey).filter(([documentKey]) => retainedDocumentKeys.has(documentKey)),
  );

  return {
    ...nextWorkspace,
    tabsById,
    paneTabIds: {
      ...nextWorkspace.paneTabIds,
      right:
        workspace.paneTabIds.right && tabsById[workspace.paneTabIds.right]?.role === 'sidecar'
          ? workspace.paneTabIds.right
          : null,
    },
    snapshotBindingsByDocumentKey,
  };
}

export function syncWorkspaceEditorTab(
  workspace: EditorWorkspaceState,
  input: WorkspaceEditorTabInput,
  role: Exclude<EditorWorkspaceTabRole, 'sidecar'>,
): EditorWorkspaceState {
  const existing = workspace.tabsById[input.id];
  if (existing?.role === 'sidecar') return workspace;
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
  if (workspace.tabsById[input.id]?.role === 'sidecar') return workspace;
  const existing = workspace.tabsById[input.id];
  return {
    ...workspace,
    tabsById: {
      ...workspace.tabsById,
      [input.id]: createEditorTabFromInput(input, existing?.role === 'primary' ? 'primary' : 'background', existing),
    },
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
  if (!target || target.role === 'sidecar') return null;
  return {
    workspace: activateWorkspaceTab(workspace, tabId),
    effect: { kind: 'activate-existing', tabId },
  };
}

export function activateWorkspaceTab(workspace: EditorWorkspaceState, tabId: string): EditorWorkspaceState {
  const target = workspace.tabsById[tabId];
  if (!target || target.role === 'sidecar') return workspace;
  if (workspace.primaryTabId === tabId && workspace.activeTabId === tabId && workspace.paneTabIds.left === tabId) return workspace;
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
  if (!closedTab || closedTab.role === 'sidecar') return null;
  const closedIndex = workspace.tabOrder.indexOf(tabId);
  if (closedIndex < 0) return null;
  let nextTabOrder = workspace.tabOrder.filter((id) => id !== tabId);
  if (nextTabOrder.length === 0) {
    if (workspace.tabsById[blank.id] || workspace.paneTabIds.right === blank.id) return null;
  }
  const nextTabsById = { ...workspace.tabsById };
  delete nextTabsById[tabId];
  if (nextTabOrder.length === 0) {
    nextTabsById[blank.id] = createEditorTabFromInput({ ...blank, sourceText: '', origin: 'user' }, 'primary');
    nextTabOrder = [blank.id];
    const nextWorkspace = {
      ...workspace,
      tabsById: nextTabsById,
      primaryTabId: blank.id,
      activeTabId: blank.id,
      tabOrder: nextTabOrder,
      paneTabIds: { ...workspace.paneTabIds, left: blank.id },
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
    .filter((tab): tab is EditorWorkspaceTab => Boolean(tab && tab.role !== 'sidecar'))
    .map((tab) => ({ id: tab.id, name: tab.name, languageId: tab.languageId, dirty: isWorkspaceTabDirty(tab) }));
}

export function isWorkspaceTabDirty(tab: EditorWorkspaceTab): boolean {
  return tab.savedText !== undefined && tab.sourceText !== tab.savedText;
}

export function ensureSidecarTab(
  workspace: EditorWorkspaceState,
  input: {
    id: string;
    name: string;
    languageId: SupportedEditorLanguageId;
    sourceText: string;
  },
): EditorWorkspaceState {
  const existingId = workspace.paneTabIds.right;
  if (existingId && workspace.tabsById[existingId]) return workspace;
  const existing = workspace.tabsById[input.id];
  if (existing?.role === 'sidecar') {
    return {
      ...workspace,
      paneTabIds: {
        ...workspace.paneTabIds,
        right: existing.id,
      },
    };
  }
  const sidecar: EditorWorkspaceTab = {
    id: input.id,
    role: 'sidecar',
    name: input.name,
    documentKey: sidecarDocumentKey(input.id),
    languageId: input.languageId,
    sourceText: input.sourceText,
    origin: 'user',
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
    paneTabIds: {
      ...workspace.paneTabIds,
      right: sidecar.id,
    },
  };
}

export function ensureDetachedSidecarTab(
  workspace: EditorWorkspaceState,
  input: {
    id: string;
    name: string;
    languageId: SupportedEditorLanguageId;
    sourceText: string;
  },
): EditorWorkspaceState {
  const existing = workspace.tabsById[input.id];
  if (existing?.role === 'sidecar') return workspace;
  if (existing) return workspace;
  const sidecar: EditorWorkspaceTab = {
    id: input.id,
    role: 'sidecar',
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

export function removeDetachedSidecarTab(workspace: EditorWorkspaceState, tabId: string): EditorWorkspaceState {
  const current = workspace.tabsById[tabId];
  if (!current || current.role !== 'sidecar') return workspace;
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
    current.role === 'sidecar' ||
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
    languageId: transition.next.languageId,
    sourceText: transition.next.sourceText,
    revision: transition.next.revision,
    graphAppliedRevision: Math.min(current.graphAppliedRevision, Math.max(0, transition.next.revision - 1)),
    snapshotId: null,
  };
  const snapshotBindingsByDocumentKey = { ...workspace.snapshotBindingsByDocumentKey };
  delete snapshotBindingsByDocumentKey[current.documentKey];

  return {
    ...workspace,
    tabsById: {
      ...workspace.tabsById,
      [current.id]: nextTab,
    },
    snapshotBindingsByDocumentKey,
  };
}

export function syncSidecarLanguageFromPrimary(workspace: EditorWorkspaceState): EditorWorkspaceState {
  const primary = workspace.tabsById[workspace.primaryTabId];
  if (!primary) return workspace;
  let nextWorkspace = workspace;
  for (const [tabId, tab] of Object.entries(workspace.tabsById)) {
    if (tab.role !== 'sidecar' || tab.languageId === primary.languageId) continue;
    nextWorkspace = updateWorkspaceTab(nextWorkspace, tabId, { languageId: primary.languageId });
  }
  return nextWorkspace;
}
