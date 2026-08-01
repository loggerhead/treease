import type { SnapshotId } from '@core-wasm/index';
import type * as Monaco from 'monaco-editor';
import { derived, get, writable, type Readable } from 'svelte/store';

import { editorLanguageFallback, type SupportedEditorLanguageId } from '../monaco/language-support';
import { initialFullEditUiState } from './full-edit-ui-state';
import { initialTempModel } from './graph-selection-store';
import {
  createEditorWorkspaceState,
  syncSidecarLanguageFromPrimary,
  type EditorWorkspaceState,
  type EditorWorkspaceTab,
  type EditorWorkspaceTabPatch,
} from './editor-workspace';
import type { DocumentSessionState, EditorIO } from './editor-store-types';

/**
 * The only Web-owned authority for an active Document. Document Runtime remains
 * authoritative for DocumentSnapshot creation and semantic classification.
 * Workspace Store and Document Session are adapters over this state.
 */
export type ActiveDocumentSemanticStatus =
  | 'pendingWholeDocument'
  | 'pendingJsonBlockEligible'
  | 'valid'
  | 'invalidWholeDocument'
  | 'invalidJsonBlockEligible'
  | 'rejected'
  | 'noSnapshot'
  | 'jobFailed';

export type ActiveDocumentSemanticState = {
  documentKey: string;
  language: string;
  revision: number;
  status: ActiveDocumentSemanticStatus;
  snapshotId: SnapshotId | null;
};

type ActiveDocumentSemanticStateByKey = Record<string, ActiveDocumentSemanticState>;

export type ActiveDocumentAuthorityState = {
  workspace: EditorWorkspaceState;
  semanticByDocumentKey: ActiveDocumentSemanticStateByKey;
  editorIO: EditorIO | null;
  previousSourceText: string;
  compareEditToken: number;
};

function createPrimaryTab(): EditorWorkspaceTab {
  return {
    id: 'primary',
    role: 'primary',
    name: 'Primary',
    documentKey: '',
    languageId: editorLanguageFallback,
    sourceText: '',
    origin: 'example',
    revision: 0,
    graphAppliedRevision: 0,
    snapshotId: null,
    tempModel: initialTempModel,
    fullEditUiState: initialFullEditUiState,
  };
}

export const initialActiveDocumentAuthorityState: ActiveDocumentAuthorityState = {
  workspace: createEditorWorkspaceState(createPrimaryTab()),
  semanticByDocumentKey: {},
  editorIO: null,
  previousSourceText: '',
  compareEditToken: 0,
};

export const activeDocumentAuthorityStore = writable<ActiveDocumentAuthorityState>(initialActiveDocumentAuthorityState);

export const activeDocumentSemanticStateByKey: Readable<ActiveDocumentSemanticStateByKey> = derived(
  activeDocumentAuthorityStore,
  ($authority) => $authority.semanticByDocumentKey,
);

function getActiveTab(state: ActiveDocumentAuthorityState): EditorWorkspaceTab {
  return state.workspace.tabsById[state.workspace.activeTabId] ?? state.workspace.tabsById[state.workspace.primaryTabId];
}

function patchWorkspace(state: ActiveDocumentAuthorityState, workspace: EditorWorkspaceState): ActiveDocumentAuthorityState {
  return workspace === state.workspace ? state : { ...state, workspace };
}

type ActiveDocumentPatch = EditorWorkspaceTabPatch & { documentKey?: string };

function patchActiveTab(state: ActiveDocumentAuthorityState, patch: ActiveDocumentPatch): ActiveDocumentAuthorityState {
  const tab = getActiveTab(state);
  if (!tab) return state;
  const nextTab = { ...tab, ...patch };
  const workspace = {
    ...state.workspace,
    tabsById: { ...state.workspace.tabsById, [tab.id]: nextTab },
  };
  return patchWorkspace(state, workspace);
}

function updateAuthority(updater: (current: ActiveDocumentAuthorityState) => ActiveDocumentAuthorityState): void {
  activeDocumentAuthorityStore.update((current) => updater(current));
}

export function getActiveDocumentAuthorityState(): ActiveDocumentAuthorityState {
  return get(activeDocumentAuthorityStore);
}

export function getAuthorityWorkspaceState(): EditorWorkspaceState {
  return getActiveDocumentAuthorityState().workspace;
}

export function setAuthorityWorkspaceState(workspace: EditorWorkspaceState): void {
  updateAuthority((current) => patchWorkspace(current, workspace));
}

function bindSnapshotInWorkspace(
  workspace: EditorWorkspaceState,
  payload: { documentKey: string; revision: number; snapshotId: SnapshotId | null | undefined },
): EditorWorkspaceState {
  if (!payload.documentKey || payload.snapshotId == null) return workspace;
  const current = workspace.snapshotBindingsByDocumentKey[payload.documentKey];
  const newestTabRevision = Object.values(workspace.tabsById).reduce(
    (newest, tab) => (tab.documentKey === payload.documentKey ? Math.max(newest, tab.revision) : newest),
    -1,
  );
  if ((current && payload.revision < current.revision) || (newestTabRevision >= 0 && payload.revision < newestTabRevision)) {
    return workspace;
  }
  let tabsById = workspace.tabsById;
  for (const [tabId, tab] of Object.entries(workspace.tabsById)) {
    if (tab.documentKey !== payload.documentKey || payload.revision < tab.revision) continue;
    if (tabsById === workspace.tabsById) tabsById = { ...tabsById };
    tabsById[tabId] = { ...tab, revision: Math.max(tab.revision, payload.revision), snapshotId: payload.snapshotId };
  }
  return {
    ...workspace,
    tabsById,
    snapshotBindingsByDocumentKey: {
      ...workspace.snapshotBindingsByDocumentKey,
      [payload.documentKey]: { documentKey: payload.documentKey, revision: payload.revision, snapshotId: payload.snapshotId },
    },
  };
}

export function bindAuthoritySnapshot(payload: { documentKey: string; revision: number; snapshotId: SnapshotId | null | undefined }): void {
  updateAuthority((current) => patchWorkspace(current, bindSnapshotInWorkspace(current.workspace, payload)));
}

export function clearAuthoritySnapshot(documentKey: string, snapshotId?: SnapshotId | null): void {
  updateAuthority((current) => {
    const workspace = current.workspace;
    const binding = workspace.snapshotBindingsByDocumentKey[documentKey];
    if (!documentKey || !binding || (snapshotId != null && binding.snapshotId !== snapshotId)) return current;
    const snapshotBindingsByDocumentKey = { ...workspace.snapshotBindingsByDocumentKey };
    delete snapshotBindingsByDocumentKey[documentKey];
    const tabsById = Object.fromEntries(
      Object.entries(workspace.tabsById).map(([tabId, tab]) => [
        tabId,
        tab.documentKey === documentKey && (snapshotId == null || tab.snapshotId === snapshotId)
          ? { ...tab, snapshotId: null }
          : tab,
      ]),
    );
    return patchWorkspace(current, { ...workspace, tabsById, snapshotBindingsByDocumentKey });
  });
}

export function patchAuthorityActiveDocument(patch: ActiveDocumentPatch): void {
  updateAuthority((current) => {
    const active = getActiveTab(current);
    const next = patchActiveTab(current, patch);
    const withSidecarLanguage = patch.languageId !== undefined
      ? { ...next, workspace: syncSidecarLanguageFromPrimary(next.workspace) }
      : next;
    return 'sourceText' in patch && patch.sourceText !== undefined && patch.sourceText !== active?.sourceText
      ? { ...withSidecarLanguage, previousSourceText: active?.sourceText ?? current.previousSourceText }
      : withSidecarLanguage;
  });
}

export function setAuthorityEditorIO(editorIO: EditorIO | null): void {
  updateAuthority((current) => (current.editorIO === editorIO ? current : { ...current, editorIO }));
}

export function setAuthorityCompareEditToken(compareEditToken: number): void {
  updateAuthority((current) => (current.compareEditToken === compareEditToken ? current : { ...current, compareEditToken }));
}

export function setAuthorityDocumentSession(state: DocumentSessionState): void {
  updateAuthority((current) => {
    const active = getActiveTab(current);
    const workspace = active
      ? patchActiveTab(current, {
          sourceText: state.sourceText,
          documentKey: state.documentKey,
          languageId: state.languageId,
          revision: state.editorRevision,
          graphAppliedRevision: state.graphAppliedRevision,
        }).workspace
      : current.workspace;
    return {
      ...current,
      workspace,
      previousSourceText: state.previousSourceText,
      compareEditToken: state.compareEditToken,
      editorIO: state.editorIO,
    };
  });
}

export function getAuthorityDocumentSessionState(): DocumentSessionState {
  const authority = getActiveDocumentAuthorityState();
  const active = getActiveTab(authority);
  return {
    sourceText: active?.sourceText ?? '',
    previousSourceText: authority.previousSourceText,
    documentKey: active?.documentKey ?? '',
    languageId: active?.languageId ?? editorLanguageFallback,
    compareEditToken: authority.compareEditToken,
    editorRevision: active?.revision ?? 0,
    graphAppliedRevision: active?.graphAppliedRevision ?? 0,
    editorIO: authority.editorIO,
  };
}

export function resetActiveDocumentAuthority(): void {
  activeDocumentAuthorityStore.set(initialActiveDocumentAuthorityState);
}

function isStale(current: ActiveDocumentSemanticState | undefined, revision: number): boolean {
  return Boolean(current && revision < current.revision);
}

function isWholeDocumentSemanticStatus(status: ActiveDocumentSemanticStatus | undefined): boolean {
  return status === 'pendingWholeDocument' || status === 'valid' || status === 'invalidWholeDocument';
}

function setSemanticState(next: ActiveDocumentSemanticState): void {
  if (!next.documentKey) return;
  updateAuthority((current) => {
    const previous = current.semanticByDocumentKey[next.documentKey];
    if (isStale(previous, next.revision)) return current;
    return {
      ...current,
      semanticByDocumentKey: { ...current.semanticByDocumentKey, [next.documentKey]: next },
    };
  });
}

export function markActiveDocumentSemanticPending(payload: { documentKey: string; language: string; revision: number }): void {
  const previous = getActiveDocumentSemanticState(payload.documentKey);
  setSemanticState({
    ...payload,
    status: isWholeDocumentSemanticStatus(previous?.status) ? 'pendingWholeDocument' : 'pendingJsonBlockEligible',
    snapshotId: null,
  });
}

export type ActiveDocumentJobOutcome = {
  documentKey: string;
  language: string;
  revision: number;
  status: 'snapshotReady' | 'parseFailed' | 'rejected' | 'noSnapshot' | 'jobFailed';
  snapshotId: SnapshotId | null;
};

export function beginActiveDocumentJob(payload: { documentKey: string; language: string; revision: number }): boolean {
  const current = getActiveDocumentSemanticState(payload.documentKey);
  if (current && current.revision > payload.revision) return false;
  markActiveDocumentSemanticPending(payload);
  return true;
}

/** Apply a terminal Document Runtime outcome exactly once at the authority. */
export function applyActiveDocumentJobOutcome(outcome: ActiveDocumentJobOutcome): boolean {
  const current = getActiveDocumentSemanticState(outcome.documentKey);
  if (current && current.revision > outcome.revision) return false;
  if (outcome.status === 'snapshotReady' && outcome.snapshotId != null) {
    markActiveDocumentSemanticValid({ ...outcome, snapshotId: outcome.snapshotId });
  } else if (outcome.status === 'parseFailed') {
    markActiveDocumentSemanticInvalid({ ...outcome, snapshotId: outcome.snapshotId });
  } else {
    markActiveDocumentSemanticTerminal({ ...outcome, status: outcome.status as 'rejected' | 'noSnapshot' | 'jobFailed' });
  }
  return true;
}

export function markActiveDocumentSemanticValid(payload: {
  documentKey: string;
  language: string;
  revision: number;
  snapshotId: SnapshotId;
}): void {
  updateAuthority((current) => {
    const previous = current.semanticByDocumentKey[payload.documentKey];
    if (!payload.documentKey || isStale(previous, payload.revision)) return current;
    const semanticByDocumentKey = {
      ...current.semanticByDocumentKey,
      [payload.documentKey]: { ...payload, status: 'valid' as const },
    };
    return {
      ...patchWorkspace(current, bindSnapshotInWorkspace(current.workspace, payload)),
      semanticByDocumentKey,
    };
  });
}

export function markActiveDocumentSemanticInvalid(payload: {
  documentKey: string;
  language: string;
  revision: number;
  snapshotId: SnapshotId | null;
}): void {
  const previous = getActiveDocumentSemanticState(payload.documentKey);
  setSemanticState({
    ...payload,
    status: isWholeDocumentSemanticStatus(previous?.status) ? 'invalidWholeDocument' : 'invalidJsonBlockEligible',
  });
}

export function markActiveDocumentSemanticTerminal(payload: {
  documentKey: string;
  language: string;
  revision: number;
  status: Extract<ActiveDocumentSemanticStatus, 'rejected' | 'noSnapshot' | 'jobFailed'>;
}): void {
  setSemanticState({ ...payload, snapshotId: null });
}

export function clearActiveDocumentSemanticState(documentKey?: string): void {
  updateAuthority((current) => {
    if (!documentKey) return { ...current, semanticByDocumentKey: {} };
    if (!current.semanticByDocumentKey[documentKey]) return current;
    const semanticByDocumentKey = { ...current.semanticByDocumentKey };
    delete semanticByDocumentKey[documentKey];
    return { ...current, semanticByDocumentKey };
  });
}

export function getActiveDocumentSemanticState(documentKey: string): ActiveDocumentSemanticState | null {
  return documentKey ? getActiveDocumentAuthorityState().semanticByDocumentKey[documentKey] ?? null : null;
}

export function isActiveDocumentSemanticValid(documentKey: string, revision?: number): boolean {
  const state = getActiveDocumentSemanticState(documentKey);
  return Boolean(state?.status === 'valid' && (revision == null || state.revision === revision));
}

export function isActiveDocumentSemanticPending(documentKey: string, revision?: number): boolean {
  const state = getActiveDocumentSemanticState(documentKey);
  if (!state || (revision != null && state.revision !== revision)) return false;
  return state.status === 'pendingWholeDocument' || state.status === 'pendingJsonBlockEligible';
}

export function shouldSuppressJsonBlockFallback(documentKey: string, revision?: number): boolean {
  const state = getActiveDocumentSemanticState(documentKey);
  if (!state || (revision != null && state.revision !== revision)) return false;
  return state.status === 'valid' || state.status === 'invalidWholeDocument';
}

export function getActiveDocumentCommitBaseSnapshotId(documentKey: string): SnapshotId | null {
  const state = getActiveDocumentSemanticState(documentKey);
  return state && ['valid', 'invalidWholeDocument', 'invalidJsonBlockEligible'].includes(state.status) ? state.snapshotId : null;
}

export function getActiveDocumentSuccessfulSnapshotId(documentKey: string, revision?: number): SnapshotId | null {
  return isActiveDocumentSemanticValid(documentKey, revision)
    ? getActiveDocumentSemanticState(documentKey)?.snapshotId ?? null
    : null;
}

export function getAuthorityWorkspaceSnapshotId(documentKey: string): SnapshotId | null {
  return documentKey ? getAuthorityWorkspaceState().snapshotBindingsByDocumentKey[documentKey]?.snapshotId ?? null : null;
}

export type ActiveDocumentTextSource = 'monacoModel' | 'editorIO' | 'workspaceTab';

export type ActiveDocumentContext = {
  documentKey: string;
  languageId: SupportedEditorLanguageId;
  revision: number;
  snapshotId: SnapshotId | null;
  text: string;
  textSource: ActiveDocumentTextSource;
  model: Monaco.editor.ITextModel | null;
};

export function resolveReadableSnapshotId(documentKey: string, revision: number, fallbackSnapshotId?: SnapshotId | null): SnapshotId | null {
  return getActiveDocumentSuccessfulSnapshotId(documentKey, revision) ?? fallbackSnapshotId ?? getAuthorityWorkspaceSnapshotId(documentKey);
}

export function resolveCommitBaseSnapshotId(documentKey: string, fallbackSnapshotId?: SnapshotId | null): SnapshotId | null {
  return getActiveDocumentCommitBaseSnapshotId(documentKey) ?? fallbackSnapshotId ?? getAuthorityWorkspaceSnapshotId(documentKey);
}

export function getActiveDocumentContext(): ActiveDocumentContext {
  const authority = getActiveDocumentAuthorityState();
  const active = getActiveTab(authority);
  const documentKey = active?.documentKey ?? '';
  const revision = active?.revision ?? 0;
  const snapshotId = resolveReadableSnapshotId(documentKey, revision, active?.snapshotId ?? null);
  const model = authority.editorIO?.getModel() ?? null;
  const modelText = model?.getValue();
  if (modelText != null) {
    return { documentKey, languageId: active?.languageId ?? editorLanguageFallback, revision, snapshotId, text: modelText, textSource: 'monacoModel', model };
  }
  const ioText = authority.editorIO?.getText?.();
  if (ioText != null) {
    return { documentKey, languageId: authority.editorIO?.getLanguage?.() ?? active?.languageId ?? editorLanguageFallback, revision, snapshotId, text: ioText, textSource: 'editorIO', model: null };
  }
  return { documentKey, languageId: active?.languageId ?? editorLanguageFallback, revision, snapshotId, text: active?.sourceText ?? '', textSource: 'workspaceTab', model: null };
}

export function getActiveDocumentText(): string {
  return getActiveDocumentContext().text;
}
