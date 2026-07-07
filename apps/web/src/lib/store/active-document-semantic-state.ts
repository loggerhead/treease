import type { SnapshotId } from '@core-wasm/index';
import { get, writable } from 'svelte/store';

export type ActiveDocumentSemanticStatus =
  | 'pendingWholeDocument'
  | 'pendingJsonBlockEligible'
  | 'valid'
  | 'invalidWholeDocument'
  | 'invalidJsonBlockEligible'
  | 'rejected'
  | 'noSnapshot';

export type ActiveDocumentSemanticState = {
  documentKey: string;
  language: string;
  revision: number;
  status: ActiveDocumentSemanticStatus;
  snapshotId: SnapshotId | null;
};

type ActiveDocumentSemanticStateByKey = Record<string, ActiveDocumentSemanticState>;

export const activeDocumentSemanticStateByKey = writable<ActiveDocumentSemanticStateByKey>({});

function isStale(current: ActiveDocumentSemanticState | undefined, revision: number): boolean {
  return Boolean(current && revision < current.revision);
}

// Once a document key establishes whole-document semantics, later parse failures
// stay on that authority path instead of degrading into transient block mode.
function isWholeDocumentSemanticStatus(status: ActiveDocumentSemanticStatus | undefined): boolean {
  return status === 'pendingWholeDocument' || status === 'valid' || status === 'invalidWholeDocument';
}

function setActiveDocumentSemanticState(next: ActiveDocumentSemanticState): void {
  if (!next.documentKey) return;
  activeDocumentSemanticStateByKey.update((current) => {
    const previous = current[next.documentKey];
    if (isStale(previous, next.revision)) return current;
    return {
      ...current,
      [next.documentKey]: next,
    };
  });
}

export function markActiveDocumentSemanticPending(payload: {
  documentKey: string;
  language: string;
  revision: number;
}): void {
  const previous = getActiveDocumentSemanticState(payload.documentKey);
  setActiveDocumentSemanticState({
    ...payload,
    status: isWholeDocumentSemanticStatus(previous?.status) ? 'pendingWholeDocument' : 'pendingJsonBlockEligible',
    snapshotId: null,
  });
}

export function markActiveDocumentSemanticValid(payload: {
  documentKey: string;
  language: string;
  revision: number;
  snapshotId: SnapshotId;
}): void {
  setActiveDocumentSemanticState({
    ...payload,
    status: 'valid',
    snapshotId: payload.snapshotId,
  });
}

export function markActiveDocumentSemanticInvalid(payload: {
  documentKey: string;
  language: string;
  revision: number;
  snapshotId: SnapshotId | null;
}): void {
  const previous = getActiveDocumentSemanticState(payload.documentKey);
  setActiveDocumentSemanticState({
    ...payload,
    status: isWholeDocumentSemanticStatus(previous?.status) ? 'invalidWholeDocument' : 'invalidJsonBlockEligible',
    snapshotId: payload.snapshotId,
  });
}

export function markActiveDocumentSemanticTerminal(payload: {
  documentKey: string;
  language: string;
  revision: number;
  status: Extract<ActiveDocumentSemanticStatus, 'rejected' | 'noSnapshot'>;
}): void {
  setActiveDocumentSemanticState({
    ...payload,
    snapshotId: null,
  });
}

export function clearActiveDocumentSemanticState(documentKey?: string): void {
  if (!documentKey) {
    activeDocumentSemanticStateByKey.set({});
    return;
  }
  activeDocumentSemanticStateByKey.update((current) => {
    if (!current[documentKey]) return current;
    const next = { ...current };
    delete next[documentKey];
    return next;
  });
}

export function getActiveDocumentSemanticState(documentKey: string): ActiveDocumentSemanticState | null {
  if (!documentKey) return null;
  return get(activeDocumentSemanticStateByKey)[documentKey] ?? null;
}

export function isActiveDocumentSemanticValid(documentKey: string, revision?: number): boolean {
  const state = getActiveDocumentSemanticState(documentKey);
  if (!state || state.status !== 'valid') return false;
  return revision == null || state.revision === revision;
}

export function isActiveDocumentSemanticPending(documentKey: string, revision?: number): boolean {
  const state = getActiveDocumentSemanticState(documentKey);
  if (!state) return false;
  if (revision != null && state.revision !== revision) return false;
  return state.status === 'pendingWholeDocument' || state.status === 'pendingJsonBlockEligible';
}

export function shouldSuppressJsonBlockFallback(documentKey: string, revision?: number): boolean {
  const state = getActiveDocumentSemanticState(documentKey);
  if (!state) return false;
  if (revision != null && state.revision !== revision) return false;
  return state.status === 'valid' || state.status === 'invalidWholeDocument';
}

export function getActiveDocumentCommitBaseSnapshotId(documentKey: string): SnapshotId | null {
  const state = getActiveDocumentSemanticState(documentKey);
  if (!state) return null;
  if (
    state.status !== 'valid' &&
    state.status !== 'invalidWholeDocument' &&
    state.status !== 'invalidJsonBlockEligible'
  ) {
    return null;
  }
  return state.snapshotId;
}

export function getActiveDocumentSuccessfulSnapshotId(documentKey: string, revision?: number): SnapshotId | null {
  if (!isActiveDocumentSemanticValid(documentKey, revision)) return null;
  return getActiveDocumentSemanticState(documentKey)?.snapshotId ?? null;
}
