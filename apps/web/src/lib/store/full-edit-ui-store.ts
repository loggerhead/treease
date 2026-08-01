import { derived, get, writable, type Writable } from 'svelte/store';

import type { JsonBlockSelection } from './editor-store-types';

function deepFreezeForRead<T>(value: T): T {
  if (!value || typeof value !== 'object') return value;
  if (Object.isFrozen(value)) return value;
  Object.freeze(value);
  for (const nestedValue of Object.values(value as Record<string, unknown>)) {
    deepFreezeForRead(nestedValue);
  }
  return value;
}

function cloneJsonBlockSelectionForRead(selection: JsonBlockSelection | null): JsonBlockSelection | null {
  return selection ? deepFreezeForRead({ ...selection }) : null;
}

function cloneJsonBlockSelectionForWrite(selection: JsonBlockSelection | null): JsonBlockSelection | null {
  return selection ? { ...selection } : null;
}

export { initialFullEditUiState } from './full-edit-ui-state';
export type {
  FullEditSessionKind,
  FullEditTransportKind,
  FullEditUiPhase,
  FullEditUiState,
  JsonBlockSelection,
} from './editor-store-types';

export const jsonBlockSelectionStore = writable<JsonBlockSelection | null>(null);

export function getJsonBlockSelectionSnapshot(): JsonBlockSelection | null {
  return cloneJsonBlockSelectionForRead(get(jsonBlockSelectionStore));
}

export function getJsonBlockSelectionRaw(): JsonBlockSelection | null {
  return get(jsonBlockSelectionStore);
}

export function setJsonBlockSelection(value: JsonBlockSelection | null): void {
  jsonBlockSelectionStore.set(cloneJsonBlockSelectionForWrite(value));
}

export function clearJsonBlockSelectionForDocument(documentKey: string): void {
  jsonBlockSelectionStore.update((current) =>
    current?.sourceDocumentKey === documentKey ? null : current,
  );
}

export const jsonBlockSelection: Writable<JsonBlockSelection | null> = {
  subscribe: (run) =>
    derived(jsonBlockSelectionStore, ($state) => cloneJsonBlockSelectionForRead($state)).subscribe(run),
  set: setJsonBlockSelection,
  update: (fn) => setJsonBlockSelection(fn(get(jsonBlockSelectionStore))),
};
