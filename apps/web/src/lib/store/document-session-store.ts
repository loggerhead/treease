import { derived, get, writable, type Readable, type Writable } from 'svelte/store';

import { editorLanguageFallback, type SupportedEditorLanguageId } from '../monaco/language-support';
import {
  activeDocumentAuthorityStore,
  getAuthorityDocumentSessionState,
  patchAuthorityActiveDocument,
  resetActiveDocumentAuthority,
  setAuthorityCompareEditToken,
  setAuthorityDocumentSession,
  setAuthorityEditorIO,
} from './active-document-authority';
import type { PathSeg } from './tree-path';
import type { DocumentSessionState, EditorIO, EditorMutation, EditorMutationEnvelope } from './editor-store-types';

function clonePathSegs(path: PathSeg[]) {
  return path.map((segment) => ({ ...segment }));
}

function cloneEditorMutationForWrite(mutation: EditorMutation): EditorMutation {
  if (mutation.type === 'changeLanguage') {
    return { type: mutation.type, payload: { ...mutation.payload } };
  }
  return {
    ...mutation,
    payload: {
      ...mutation.payload,
      graphEditFallback: mutation.payload.graphEditFallback
        ? { ...mutation.payload.graphEditFallback, path: clonePathSegs(mutation.payload.graphEditFallback.path) }
        : undefined,
    },
  };
}

function deepFreezeForRead<T>(value: T): T {
  if (!value || typeof value !== 'object' || Object.isFrozen(value)) return value;
  Object.freeze(value);
  for (const nestedValue of Object.values(value as Record<string, unknown>)) deepFreezeForRead(nestedValue);
  return value;
}

function cloneEditorMutationForRead(editorMutation: EditorMutationEnvelope | null): EditorMutationEnvelope | null {
  if (!editorMutation) return null;
  return deepFreezeForRead({
    ...editorMutation,
    mutation: cloneEditorMutationForWrite(editorMutation.mutation),
  });
}

export const initialDocumentSessionState: DocumentSessionState = {
  sourceText: '',
  previousSourceText: '',
  documentKey: '',
  languageId: editorLanguageFallback,
  compareEditToken: 0,
  editorRevision: 0,
  graphAppliedRevision: 0,
  editorIO: null,
};

let editorMutationId = 0;
export const editorMutationRawStore = writable<EditorMutationEnvelope | null>(null);

const authoritySessionStore = derived(activeDocumentAuthorityStore, () => getAuthorityDocumentSessionState());

/** Document Session is an Adapter: writes are routed to Active Document authority. */
export const documentSessionStore: Writable<DocumentSessionState> = {
  subscribe: authoritySessionStore.subscribe,
  set: setAuthorityDocumentSession,
  update: (updater) => setAuthorityDocumentSession(updater(getAuthorityDocumentSessionStore())),
};

function getAuthorityDocumentSessionStore(): DocumentSessionState {
  return get(authoritySessionStore);
}

export function getDocumentSessionState(): DocumentSessionState {
  return getAuthorityDocumentSessionStore();
}

export function getEditorMutationState(): EditorMutationEnvelope | null {
  return cloneEditorMutationForRead(get(editorMutationRawStore));
}

export function getEditorMutationRawState(): EditorMutationEnvelope | null {
  return get(editorMutationRawStore);
}

export function setDocumentSessionState(state: DocumentSessionState): void {
  setAuthorityDocumentSession(state);
}

export function setEditorMutationState(state: EditorMutationEnvelope | null): void {
  editorMutationRawStore.set(state);
}

export function setSourceText(sourceText: string): void {
  patchAuthorityActiveDocument({ sourceText });
}

export function setDocumentKey(documentKey: string): void {
  patchAuthorityActiveDocument({ documentKey });
}

export function setLanguageId(languageId: SupportedEditorLanguageId): void {
  patchAuthorityActiveDocument({ languageId });
}

export function incrementEditorRevision(): void {
  patchAuthorityActiveDocument({ revision: getDocumentSessionState().editorRevision + 1 });
}

export function setEditorRevision(editorRevision: number): void {
  patchAuthorityActiveDocument({ revision: editorRevision });
}

export function setCompareEditToken(compareEditToken: number): void {
  setAuthorityCompareEditToken(compareEditToken);
}

export function updateCompareEditToken(updater: (value: number) => number): void {
  setAuthorityCompareEditToken(updater(getDocumentSessionState().compareEditToken));
}

export function setGraphAppliedRevision(graphAppliedRevision: number): void {
  patchAuthorityActiveDocument({ graphAppliedRevision });
}

export function setEditorIO(editorIO: EditorIO | null): void {
  setAuthorityEditorIO(editorIO);
}

export function emitEditorMutation(mutation: EditorMutation): void {
  editorMutationId += 1;
  editorMutationRawStore.set({ id: editorMutationId, mutation: cloneEditorMutationForWrite(mutation) });
}

export function clearEditorMutation(): void {
  editorMutationRawStore.set(null);
}

export function resetDocumentSession(): void {
  resetActiveDocumentAuthority();
  editorMutationId = 0;
  editorMutationRawStore.set(null);
}

function createDocumentSessionFieldStore<K extends keyof DocumentSessionState>(
  key: K,
  setter: (value: DocumentSessionState[K]) => void,
): Writable<DocumentSessionState[K]> {
  return {
    subscribe: (run) => derived(authoritySessionStore, ($state) => $state[key]).subscribe(run),
    set: setter,
    update: (updater) => setter(updater(getDocumentSessionState()[key])),
  };
}

export const sourceText = createDocumentSessionFieldStore('sourceText', setSourceText);
export const previousSourceText: Readable<string> = { subscribe: (run) => derived(authoritySessionStore, ($state) => $state.previousSourceText).subscribe(run) };
export const documentKey = createDocumentSessionFieldStore('documentKey', setDocumentKey);
export const languageId = createDocumentSessionFieldStore('languageId', setLanguageId);
export const compareEditToken = createDocumentSessionFieldStore('compareEditToken', setCompareEditToken);
export const editorRevision = createDocumentSessionFieldStore('editorRevision', setEditorRevision);
export const graphAppliedRevision = createDocumentSessionFieldStore('graphAppliedRevision', setGraphAppliedRevision);
export const editorIO = createDocumentSessionFieldStore('editorIO', setEditorIO);
export const editorMutation: Readable<EditorMutationEnvelope | null> = {
  subscribe: (run) => derived(editorMutationRawStore, ($state) => cloneEditorMutationForRead($state)).subscribe(run),
};

export type { EditorIO, EditorIoContext, EditorMutation, EditorMutationEnvelope } from './editor-store-types';
