import { derived, get, writable, type Readable, type Writable } from 'svelte/store';

import { editorLanguageFallback, type SupportedEditorLanguageId } from '../monaco/language-support';
import type { PathSeg } from './tree-path';
import type {
  DocumentSessionState,
  EditorIO,
  EditorIoContext,
  EditorMutation,
  EditorMutationEnvelope,
} from './editor-store-types';

function clonePathSegs(path: PathSeg[]) {
  return path.map((segment) => ({ ...segment }));
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

function cloneEditorMutationForRead(editorMutation: EditorMutationEnvelope | null): EditorMutationEnvelope | null {
  if (!editorMutation) return null;
  const mutation = editorMutation.mutation;
  return deepFreezeForRead({
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

type DocumentSessionCoordinator = {
  onStateChange?: (next: DocumentSessionState, previous: DocumentSessionState) => void;
};

let documentSessionCoordinator: DocumentSessionCoordinator | null = null;

export const documentSessionStore = writable<DocumentSessionState>(initialDocumentSessionState);
export const editorMutationRawStore = writable<EditorMutationEnvelope | null>(null);

export function getDocumentSessionState(): DocumentSessionState {
  return get(documentSessionStore);
}

export function getEditorMutationState(): EditorMutationEnvelope | null {
  return cloneEditorMutationForRead(get(editorMutationRawStore));
}

export function getEditorMutationRawState(): EditorMutationEnvelope | null {
  return get(editorMutationRawStore);
}

export function setDocumentSessionState(state: DocumentSessionState): void {
  const previous = get(documentSessionStore);
  documentSessionStore.set(state);
  documentSessionCoordinator?.onStateChange?.(state, previous);
}

export function setEditorMutationState(state: EditorMutationEnvelope | null): void {
  editorMutationRawStore.set(state);
}

export function setSourceText(value: string): void {
  const previous = get(documentSessionStore);
  const next = {
    ...previous,
    previousSourceText: previous.sourceText,
    sourceText: value,
  };
  documentSessionStore.set(next);
  documentSessionCoordinator?.onStateChange?.(next, previous);
}

export function setDocumentKey(value: string): void {
  const previous = get(documentSessionStore);
  const next = { ...previous, documentKey: value };
  documentSessionStore.set(next);
  documentSessionCoordinator?.onStateChange?.(next, previous);
}

export function setLanguageId(value: SupportedEditorLanguageId): void {
  const previous = get(documentSessionStore);
  const next = { ...previous, languageId: value };
  documentSessionStore.set(next);
  documentSessionCoordinator?.onStateChange?.(next, previous);
}

export function incrementEditorRevision(): void {
  const previous = get(documentSessionStore);
  const next = { ...previous, editorRevision: previous.editorRevision + 1 };
  documentSessionStore.set(next);
  documentSessionCoordinator?.onStateChange?.(next, previous);
}

export function setEditorRevision(value: number): void {
  const previous = get(documentSessionStore);
  const next = { ...previous, editorRevision: value };
  documentSessionStore.set(next);
  documentSessionCoordinator?.onStateChange?.(next, previous);
}

export function setCompareEditToken(value: number): void {
  const previous = get(documentSessionStore);
  const next = { ...previous, compareEditToken: value };
  documentSessionStore.set(next);
  documentSessionCoordinator?.onStateChange?.(next, previous);
}

export function updateCompareEditToken(fn: (value: number) => number): void {
  const previous = get(documentSessionStore);
  const next = { ...previous, compareEditToken: fn(previous.compareEditToken) };
  documentSessionStore.set(next);
  documentSessionCoordinator?.onStateChange?.(next, previous);
}

export function setGraphAppliedRevision(value: number): void {
  const previous = get(documentSessionStore);
  const next = { ...previous, graphAppliedRevision: value };
  documentSessionStore.set(next);
  documentSessionCoordinator?.onStateChange?.(next, previous);
}

export function setEditorIO(value: EditorIO | null): void {
  const previous = get(documentSessionStore);
  const next = { ...previous, editorIO: value };
  documentSessionStore.set(next);
  documentSessionCoordinator?.onStateChange?.(next, previous);
}

export function emitEditorMutation(value: EditorMutation): void {
  editorMutationId += 1;
  editorMutationRawStore.set({
    id: editorMutationId,
    mutation: cloneEditorMutationForWrite(value),
  });
}

export function clearEditorMutation(): void {
  editorMutationRawStore.set(null);
}

export function resetDocumentSession(): void {
  const previous = get(documentSessionStore);
  documentSessionStore.set(initialDocumentSessionState);
  documentSessionCoordinator?.onStateChange?.(initialDocumentSessionState, previous);
  editorMutationId = 0;
  editorMutationRawStore.set(null);
}

export function registerDocumentSessionCoordinator(coordinator: DocumentSessionCoordinator | null): void {
  documentSessionCoordinator = coordinator;
}

function createDocumentSessionFieldStore<K extends keyof DocumentSessionState>(
  key: K,
  setter: (value: DocumentSessionState[K]) => void,
): Writable<DocumentSessionState[K]> {
  return {
    subscribe: (run) => derived(documentSessionStore, ($state) => $state[key]).subscribe(run),
    set: setter,
    update: (fn) => setter(fn(get(documentSessionStore)[key])),
  };
}

export const sourceText = createDocumentSessionFieldStore('sourceText', setSourceText);
export const previousSourceText: Readable<string> = {
  subscribe: (run) => derived(documentSessionStore, ($state) => $state.previousSourceText).subscribe(run),
};
export const documentKey = createDocumentSessionFieldStore('documentKey', setDocumentKey);
export const languageId = createDocumentSessionFieldStore('languageId', setLanguageId);
export const compareEditToken: Writable<number> = {
  subscribe: (run) => derived(documentSessionStore, ($state) => $state.compareEditToken).subscribe(run),
  set: setCompareEditToken,
  update: updateCompareEditToken,
};
export const editorRevision: Writable<number> = {
  subscribe: (run) => derived(documentSessionStore, ($state) => $state.editorRevision).subscribe(run),
  set: setEditorRevision,
  update: (fn) => setEditorRevision(fn(get(documentSessionStore).editorRevision)),
};
export const graphAppliedRevision = createDocumentSessionFieldStore('graphAppliedRevision', setGraphAppliedRevision);
export const editorIO = createDocumentSessionFieldStore('editorIO', setEditorIO);
export const editorMutation: Readable<EditorMutationEnvelope | null> = {
  subscribe: (run) => derived(editorMutationRawStore, ($state) => cloneEditorMutationForRead($state)).subscribe(run),
};

export type {
  EditorIO,
  EditorIoContext,
  EditorMutation,
  EditorMutationEnvelope,
} from './editor-store-types';
