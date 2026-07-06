import { get, writable, type Writable } from 'svelte/store';

import type {
  GraphHighlightState,
  GraphHighlightTarget,
  TempModel,
  TreeSelectionSource,
  TreeSyncSource,
  TreeSyncState,
} from './editor-store-types';
import type { PathSeg } from './tree-path';

function clonePathSegs(path: PathSeg[]) {
  return path.map((segment) => ({ ...segment }));
}

function cloneGraphHighlight(graphHighlight: GraphHighlightState | null): GraphHighlightState | null {
  if (!graphHighlight) return null;
  return {
    ...graphHighlight,
    path: clonePathSegs(graphHighlight.path),
  };
}

function cloneDiagnostics(diagnostics: TempModel['diagnostics']): TempModel['diagnostics'] {
  return diagnostics.map((diagnostic) => ({
    ...diagnostic,
    context: diagnostic.context.map((line) => ({ ...line })),
  }));
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

function cloneTempModelForRead(tempModel: TempModel): TempModel {
  return deepFreezeForRead({
    ...tempModel,
    treePath: clonePathSegs(tempModel.treePath),
    graphHighlight: cloneGraphHighlight(tempModel.graphHighlight),
    diagnostics: cloneDiagnostics(tempModel.diagnostics),
  });
}

function cloneTempModelForWrite(tempModel: TempModel): TempModel {
  return {
    ...tempModel,
    treePath: clonePathSegs(tempModel.treePath),
    graphHighlight: cloneGraphHighlight(tempModel.graphHighlight),
    diagnostics: cloneDiagnostics(tempModel.diagnostics),
  };
}

function cloneTreeStateForRead(treeState: TreeSyncState): TreeSyncState {
  return deepFreezeForRead({
    ...treeState,
    tree: structuredClone(treeState.tree),
    value: structuredClone(treeState.value),
  });
}

function cloneTreeStateForWrite(treeState: TreeSyncState): TreeSyncState {
  return {
    ...treeState,
    tree: structuredClone(treeState.tree),
    value: structuredClone(treeState.value),
  };
}

export const initialTempModel: TempModel = {
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

export const initialTreeState: TreeSyncState = {
  tree: null,
  value: null,
  revision: 0,
  source: 'editor',
};

type GraphSelectionCoordinator = {
  onTempModelChange?: (next: TempModel, previous: TempModel) => void;
};

let graphSelectionCoordinator: GraphSelectionCoordinator | null = null;

let memoizedRawTempModel: TempModel | null = null;
let memoizedReadTempModel: TempModel | null = null;
let memoizedRawTreeState: TreeSyncState | null = null;
let memoizedReadTreeState: TreeSyncState | null = null;

export const tempModelStore = writable<TempModel>(initialTempModel);
export const treeSyncStore = writable<TreeSyncState>(initialTreeState);

export function getActiveTempModelSnapshot(): TempModel {
  const raw = get(tempModelStore);
  if (memoizedRawTempModel === raw && memoizedReadTempModel) return memoizedReadTempModel;
  const snapshot = cloneTempModelForRead(raw);
  memoizedRawTempModel = raw;
  memoizedReadTempModel = snapshot;
  return snapshot;
}

export function getTreeStateSnapshot(): TreeSyncState {
  const raw = get(treeSyncStore);
  if (memoizedRawTreeState === raw && memoizedReadTreeState) return memoizedReadTreeState;
  const snapshot = cloneTreeStateForRead(raw);
  memoizedRawTreeState = raw;
  memoizedReadTreeState = snapshot;
  return snapshot;
}

export function getActiveTempModelRaw(): TempModel {
  return get(tempModelStore);
}

export function getTreeStateRaw(): TreeSyncState {
  return get(treeSyncStore);
}

export function setTempModelState(model: TempModel): void {
  const previous = get(tempModelStore);
  const next = cloneTempModelForWrite(model);
  tempModelStore.set(next);
  graphSelectionCoordinator?.onTempModelChange?.(next, previous);
}

export function setTreeStateState(treeState: TreeSyncState): void {
  treeSyncStore.set(cloneTreeStateForWrite(treeState));
}

export function resetGraphSelectionState(): void {
  setTempModelState(initialTempModel);
  treeSyncStore.set(initialTreeState);
}

export function registerGraphSelectionCoordinator(coordinator: GraphSelectionCoordinator | null): void {
  graphSelectionCoordinator = coordinator;
}

export const activeTempModel: Writable<TempModel> = {
  subscribe: (run) => {
    let initialized = false;
    let currentRaw: TempModel | undefined;
    return tempModelStore.subscribe(($state) => {
      if (initialized && Object.is($state, currentRaw)) return;
      initialized = true;
      currentRaw = $state;
      run(getActiveTempModelSnapshot());
    });
  },
  set: setTempModelState,
  update: (fn) => setTempModelState(fn(cloneTempModelForWrite(get(tempModelStore)))),
};

export const treeState: Writable<TreeSyncState> = {
  subscribe: (run) => {
    let initialized = false;
    let currentRaw: TreeSyncState | undefined;
    return treeSyncStore.subscribe(($state) => {
      if (initialized && Object.is($state, currentRaw)) return;
      initialized = true;
      currentRaw = $state;
      run(getTreeStateSnapshot());
    });
  },
  set: setTreeStateState,
  update: (fn) => setTreeStateState(fn(cloneTreeStateForWrite(get(treeSyncStore)))),
};

export type {
  GraphHighlightState,
  GraphHighlightTarget,
  TempModel,
  TreeSelectionSource,
  TreeSyncSource,
  TreeSyncState,
} from './editor-store-types';
