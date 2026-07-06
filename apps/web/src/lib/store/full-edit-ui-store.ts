import { derived, get, writable, type Writable } from 'svelte/store';

import type { SupportedEditorLanguageId } from '../monaco/language-support';
import type {
  FullEditSessionKind,
  FullEditTransportKind,
  FullEditUiPhase,
  FullEditUiState,
  JsonBlockSelection,
} from './editor-store-types';

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

function cloneFullEditUiStateForRead(state: FullEditUiState): FullEditUiState {
  return deepFreezeForRead({ ...state });
}

function cloneJsonBlockSelectionForRead(selection: JsonBlockSelection | null): JsonBlockSelection | null {
  return selection ? deepFreezeForRead({ ...selection }) : null;
}

function cloneJsonBlockSelectionForWrite(selection: JsonBlockSelection | null): JsonBlockSelection | null {
  return selection ? { ...selection } : null;
}

export const initialFullEditUiState: FullEditUiState = {
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

type FullEditUiCoordinator = {
  onFullEditUiStateChange?: (next: FullEditUiState, previous: FullEditUiState) => void;
};

let fullEditUiCoordinator: FullEditUiCoordinator | null = null;

type FullEditOwnerPayload = {
  sessionId: string;
  ownerKey: string;
};

export const fullEditUiStateStore = writable<FullEditUiState>(initialFullEditUiState);
export const jsonBlockSelectionStore = writable<JsonBlockSelection | null>(null);

function matchesFullEditOwner(current: FullEditUiState, payload: FullEditOwnerPayload): boolean {
  return current.active && current.sessionId === payload.sessionId && current.ownerKey === payload.ownerKey;
}

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

function updateOwnedFullEditUiState(
  payload: FullEditOwnerPayload,
  updater: (current: FullEditUiState) => FullEditUiState | null,
): void {
  fullEditUiStateStore.update((current) => {
    if (!matchesFullEditOwner(current, payload)) return current;
    const next = updater(current);
    return next ?? current;
  });
}

export function getFullEditUiStateSnapshot(): FullEditUiState {
  return cloneFullEditUiStateForRead(get(fullEditUiStateStore));
}

export function getJsonBlockSelectionSnapshot(): JsonBlockSelection | null {
  return cloneJsonBlockSelectionForRead(get(jsonBlockSelectionStore));
}

export function getFullEditUiStateRaw(): FullEditUiState {
  return get(fullEditUiStateStore);
}

export function getJsonBlockSelectionRaw(): JsonBlockSelection | null {
  return get(jsonBlockSelectionStore);
}

export function setFullEditUiState(value: FullEditUiState): void {
  const previous = get(fullEditUiStateStore);
  const next = { ...value };
  fullEditUiStateStore.set(next);
  fullEditUiCoordinator?.onFullEditUiStateChange?.(next, previous);
}

export function setJsonBlockSelection(value: JsonBlockSelection | null): void {
  jsonBlockSelectionStore.set(cloneJsonBlockSelectionForWrite(value));
}

export function prepareFullEditStream(payload: {
  documentKey: string;
  revision: number;
  language: SupportedEditorLanguageId | '';
  transportKind: FullEditTransportKind;
  reason: FullEditUiState['reason'];
}): void {
  setFullEditUiState(
    buildActiveFullEditUiState({
      sessionId: null,
      ownerKey: null,
      documentKey: payload.documentKey,
      revision: payload.revision,
      language: payload.language,
      transportKind: payload.transportKind,
      reason: payload.reason,
      phase: 'preparing',
    }),
  );
}

export function cancelPreparedFullEditStream(payload: {
  documentKey: string;
  revision: number;
  reason: FullEditUiState['reason'];
}): void {
  const current = get(fullEditUiStateStore);
  if (
    !current.active ||
    current.sessionId !== null ||
    current.phase !== 'preparing' ||
    current.documentKey !== payload.documentKey ||
    current.revision !== payload.revision ||
    current.reason !== payload.reason
  ) {
    return;
  }
  setFullEditUiState(initialFullEditUiState);
}

export function beginFullEditStream(payload: {
  sessionId: string;
  ownerKey: string;
  documentKey: string;
  revision: number;
  language: SupportedEditorLanguageId | '';
  transportKind: FullEditTransportKind;
  reason: FullEditUiState['reason'];
}): void {
  setFullEditUiState(
    buildActiveFullEditUiState({
      sessionId: payload.sessionId,
      ownerKey: payload.ownerKey,
      documentKey: payload.documentKey,
      revision: payload.revision,
      language: payload.language,
      transportKind: payload.transportKind,
      reason: payload.reason,
      phase: 'streaming',
    }),
  );
}

export function appendFullEditStreamChunkMeta(payload: {
  sessionId: string;
  ownerKey: string;
  streamSeq: number;
  inputByteLength: number;
  modelVersionId?: number | null;
}): void {
  updateOwnedFullEditUiState(payload, (current) => {
    if (current.phase !== 'streaming') return null;
    if (payload.streamSeq <= current.streamSeq) return null;
    if (payload.inputByteLength < current.inputByteLength) return null;
    return {
      ...current,
      streamSeq: payload.streamSeq,
      inputByteLength: payload.inputByteLength,
      byteLength: payload.inputByteLength,
      modelVersionId: typeof payload.modelVersionId === 'number' ? payload.modelVersionId : current.modelVersionId,
    };
  });
}

export function markFullEditStreamFinalizing(payload: FullEditOwnerPayload): void {
  updateOwnedFullEditUiState(payload, (current) =>
    current.phase === 'streaming' ? { ...current, phase: 'finalizing' } : null,
  );
}

export function markFullEditStreamSettled(payload: FullEditOwnerPayload): void {
  updateOwnedFullEditUiState(payload, (current) =>
    current.phase === 'finalizing' ? { ...current, phase: 'settled' } : null,
  );
}

export function completeFullEditStreamUi(payload: FullEditOwnerPayload): void {
  updateOwnedFullEditUiState(payload, (current) => ({ ...current, phase: 'idle' }));
}

export function finishFullEditStream(payload: FullEditOwnerPayload): void {
  updateOwnedFullEditUiState(payload, () => initialFullEditUiState);
}

export function cancelFullEditStream(payload: FullEditOwnerPayload): void {
  updateOwnedFullEditUiState(payload, () => initialFullEditUiState);
}

export function clearJsonBlockSelectionForDocument(documentKey: string): void {
  jsonBlockSelectionStore.update((current) =>
    current?.sourceDocumentKey === documentKey ? null : current,
  );
}

export function resetFullEditUiState(): void {
  setFullEditUiState(initialFullEditUiState);
  jsonBlockSelectionStore.set(null);
}

export function registerFullEditUiCoordinator(coordinator: FullEditUiCoordinator | null): void {
  fullEditUiCoordinator = coordinator;
}

export const fullEditUiState: Writable<FullEditUiState> = {
  subscribe: (run) => derived(fullEditUiStateStore, ($state) => cloneFullEditUiStateForRead($state)).subscribe(run),
  set: setFullEditUiState,
  update: (fn) => setFullEditUiState(fn(get(fullEditUiStateStore))),
};

export const jsonBlockSelection: Writable<JsonBlockSelection | null> = {
  subscribe: (run) =>
    derived(jsonBlockSelectionStore, ($state) => cloneJsonBlockSelectionForRead($state)).subscribe(run),
  set: setJsonBlockSelection,
  update: (fn) => setJsonBlockSelection(fn(get(jsonBlockSelectionStore))),
};

export type {
  FullEditSessionKind,
  FullEditTransportKind,
  FullEditUiPhase,
  FullEditUiState,
  JsonBlockSelection,
} from './editor-store-types';
