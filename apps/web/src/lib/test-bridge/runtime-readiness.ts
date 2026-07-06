import type { FullEditUiState } from '../store/full-edit-ui-store';

export type RuntimeReadinessGraphMode = 'committed' | 'streaming' | 'json-block';

type RuntimeReadinessSidecar = {
  requestId: number;
  hookId: string | null;
  documentKey: string | null;
  revision: number;
  settled: boolean;
};

type RuntimeReadinessCursorPath = {
  requestId: number;
  documentKey: string | null;
  revision: number;
  lineNumber: number;
  column: number;
  syncGraphHighlight: boolean;
  settled: boolean;
};

export type TreeaseRuntimeReadiness = {
  documentKey: string;
  editorRevision: number;
  cursorPath: RuntimeReadinessCursorPath;
  sidecar: RuntimeReadinessSidecar;
  import: {
    sessionId: string | null;
    phase: FullEditUiState['phase'];
    requestedRevision: number;
    completedRevision: number;
    settled: boolean;
  };
  graph: {
    mode: RuntimeReadinessGraphMode;
    requestedRevision: number;
    appliedRevision: number;
    flushedRevision: number;
    interactiveRevision: number;
    settledRevision: number;
    pendingRenderWork: boolean;
    settled: boolean;
  };
  preview: {
    requestId: number;
    sourceRevision: number;
    completedRevision: number;
    settled: boolean;
  };
  subgraph: {
    requestId: number;
    pathKey: string | null;
    sourceRevision: number;
    materializedRevision: number;
    interactiveRevision: number;
    settled: boolean;
  };
};

export type RuntimeReadinessSnapshot = TreeaseRuntimeReadiness;

type GraphMilestonePayload = {
  documentKey: string;
  revision: number;
  mode: RuntimeReadinessGraphMode;
};

type GraphInteractionPayload = GraphMilestonePayload & {
  hasGraphData: boolean;
  nodeCount: number;
  pendingRenderWork: boolean;
  interactiveReady: boolean;
};

type PreviewRequestPayload = {
  requestId: number;
  sourceRevision: number;
};

type PreviewCompletionPayload = PreviewRequestPayload & {
  completedRevision: number;
};

type SidecarRequestPayload = {
  requestId: number;
  hookId: string;
  documentKey: string;
};

type SidecarSettledPayload = SidecarRequestPayload & {
  revision: number;
};

type CursorPathRequestPayload = {
  requestId: number;
  documentKey: string;
  revision: number;
  lineNumber: number;
  column: number;
  syncGraphHighlight: boolean;
};

type CursorPathSettledPayload = Omit<CursorPathRequestPayload, 'syncGraphHighlight'>;

type SubgraphRequestPayload = {
  requestId: number;
  pathKey: string;
  sourceRevision: number;
};

type SubgraphMaterializedPayload = SubgraphRequestPayload & {
  materializedRevision: number;
};

type SubgraphInteractivePayload = SubgraphRequestPayload & {
  interactiveRevision: number;
  interactiveReady: boolean;
};

function createInitialSnapshot(): RuntimeReadinessSnapshot {
  return {
    documentKey: '',
    editorRevision: 0,
    cursorPath: createCursorPathSnapshot(),
    sidecar: createSidecarSnapshot(),
    import: {
      sessionId: null,
      phase: 'idle',
      requestedRevision: 0,
      completedRevision: 0,
      settled: true,
    },
    graph: {
      mode: 'committed',
      requestedRevision: 0,
      appliedRevision: 0,
      flushedRevision: 0,
      interactiveRevision: 0,
      settledRevision: 0,
      pendingRenderWork: false,
      settled: true,
    },
    preview: {
      requestId: 0,
      sourceRevision: 0,
      completedRevision: 0,
      settled: true,
    },
    subgraph: {
      requestId: 0,
      pathKey: null,
      sourceRevision: 0,
      materializedRevision: 0,
      interactiveRevision: 0,
      settled: true,
    },
  };
}

let snapshot = createInitialSnapshot();
let lastImportSessionId: string | null = null;
let lastImportRevision = 0;
let lastImportActive = false;

function resetImportTracking(): void {
  lastImportSessionId = null;
  lastImportRevision = 0;
  lastImportActive = false;
}

function resetForDocument(documentKey: string, editorRevision: number): void {
  snapshot = {
    ...createInitialSnapshot(),
    documentKey,
    editorRevision,
  };
  resetImportTracking();
}

function isIdleAtRevision(fullEditUiState: FullEditUiState, revision: number): boolean {
  return !fullEditUiState.active && fullEditUiState.phase === 'idle' && snapshot.editorRevision >= revision;
}

function isImportPending(fullEditUiState: FullEditUiState): boolean {
  return fullEditUiState.phase === 'streaming' || fullEditUiState.phase === 'preparing';
}

function completedImportRevision(fullEditUiState: FullEditUiState, requestedRevision: number, completedRevision: number): number {
  return isIdleAtRevision(fullEditUiState, requestedRevision)
    ? Math.max(completedRevision, requestedRevision)
    : completedRevision;
}

function createSidecarSnapshot(
  overrides: Partial<RuntimeReadinessSidecar> = {},
): RuntimeReadinessSidecar {
  return {
    requestId: 0,
    hookId: null,
    documentKey: null,
    revision: 0,
    settled: true,
    ...overrides,
  };
}

function createCursorPathSnapshot(
  overrides: Partial<RuntimeReadinessCursorPath> = {},
): RuntimeReadinessCursorPath {
  return {
    requestId: 0,
    documentKey: null,
    revision: 0,
    lineNumber: 0,
    column: 0,
    syncGraphHighlight: false,
    settled: true,
    ...overrides,
  };
}

function syncImportState(fullEditUiState: FullEditUiState): void {
  const nextRequestedRevision = fullEditUiState.active
    ? Math.max(snapshot.import.requestedRevision, fullEditUiState.revision)
    : snapshot.import.requestedRevision;
  const sessionChanged =
    Boolean(fullEditUiState.sessionId) &&
    fullEditUiState.sessionId !== lastImportSessionId &&
    fullEditUiState.revision >= lastImportRevision;
  if (sessionChanged) {
    lastImportSessionId = fullEditUiState.sessionId;
    lastImportRevision = fullEditUiState.revision;
  }
  const completedBySettled = fullEditUiState.phase === 'settled'
    ? Math.max(snapshot.import.completedRevision, fullEditUiState.revision)
    : snapshot.import.completedRevision;
  const completedRevision =
    lastImportActive &&
    fullEditUiState.phase === 'idle' &&
    lastImportRevision > 0 &&
    snapshot.editorRevision >= lastImportRevision
      ? Math.max(completedBySettled, lastImportRevision)
      : completedBySettled;
  const nextCompletedRevision = completedImportRevision(
    fullEditUiState,
    nextRequestedRevision,
    completedRevision,
  );
  const settled =
    nextRequestedRevision === 0
      || (nextCompletedRevision >= nextRequestedRevision && !isImportPending(fullEditUiState));
  snapshot = {
    ...snapshot,
    import: {
      sessionId: fullEditUiState.sessionId,
      phase: fullEditUiState.phase,
      requestedRevision: nextRequestedRevision,
      completedRevision: nextCompletedRevision,
      settled,
    },
  };
  lastImportActive = fullEditUiState.active;
}

function isCurrentDocument(payload: { documentKey: string; revision: number }): boolean {
  return payload.documentKey === snapshot.documentKey && payload.revision <= snapshot.editorRevision;
}

function advanceGraphSettlement(): void {
  const nextInteractiveRevision =
    snapshot.graph.interactiveRevision >= snapshot.graph.requestedRevision && !snapshot.graph.pendingRenderWork
      ? snapshot.graph.interactiveRevision
      : snapshot.graph.settledRevision;
  snapshot = {
    ...snapshot,
    graph: {
      ...snapshot.graph,
      settledRevision: Math.max(snapshot.graph.settledRevision, nextInteractiveRevision),
      settled:
        snapshot.graph.requestedRevision === 0
          ? true
          : Math.max(snapshot.graph.settledRevision, nextInteractiveRevision) >= snapshot.graph.requestedRevision,
    },
  };
}

export function resetRuntimeReadiness(): void {
  snapshot = createInitialSnapshot();
  resetImportTracking();
}

export function syncRuntimeReadinessFromEditorState(state: {
  documentKey: string;
  editorRevision: number;
  fullEditUiState: FullEditUiState;
}): void {
  const documentChanged = state.documentKey !== snapshot.documentKey;
  const revisionRewound = state.editorRevision < snapshot.editorRevision;
  if (documentChanged || revisionRewound) {
    resetForDocument(state.documentKey, state.editorRevision);
  } else if (state.editorRevision > snapshot.editorRevision) {
    snapshot = {
      ...snapshot,
      documentKey: state.documentKey,
      editorRevision: state.editorRevision,
    };
  } else if (state.documentKey !== snapshot.documentKey) {
    snapshot = { ...snapshot, documentKey: state.documentKey };
  }
  syncImportState(state.fullEditUiState);
}

export function markGraphRequested(payload: GraphMilestonePayload): void {
  if (payload.documentKey !== snapshot.documentKey) return;
  snapshot = {
    ...snapshot,
    graph: {
      ...snapshot.graph,
      mode: payload.mode,
      requestedRevision: Math.max(snapshot.graph.requestedRevision, payload.revision),
      pendingRenderWork: true,
      settled: false,
    },
  };
}

export function markGraphApplied(payload: GraphMilestonePayload): void {
  if (!isCurrentDocument(payload)) return;
  if (payload.revision < snapshot.graph.requestedRevision) return;
  snapshot = {
    ...snapshot,
    graph: {
      ...snapshot.graph,
      mode: payload.mode,
      appliedRevision: Math.max(snapshot.graph.appliedRevision, payload.revision),
    },
  };
}

export function markGraphFlushed(payload: GraphMilestonePayload): void {
  if (!isCurrentDocument(payload)) return;
  if (payload.revision < snapshot.graph.requestedRevision) return;
  snapshot = {
    ...snapshot,
    graph: {
      ...snapshot.graph,
      mode: payload.mode,
      flushedRevision: Math.max(snapshot.graph.flushedRevision, payload.revision),
      pendingRenderWork: false,
    },
  };
  advanceGraphSettlement();
}

export function syncGraphInteractionReadiness(payload: GraphInteractionPayload): void {
  if (!isCurrentDocument(payload)) return;
  const interactiveRevision = payload.interactiveReady || !payload.hasGraphData || payload.nodeCount === 0
    ? Math.max(snapshot.graph.interactiveRevision, payload.revision)
    : snapshot.graph.interactiveRevision;
  snapshot = {
    ...snapshot,
    graph: {
      ...snapshot.graph,
      mode: payload.mode,
      interactiveRevision,
      pendingRenderWork: payload.pendingRenderWork,
    },
  };
  advanceGraphSettlement();
}

export function markPreviewRequested(payload: PreviewRequestPayload): void {
  if (payload.sourceRevision < snapshot.preview.sourceRevision) return;
  snapshot = {
    ...snapshot,
    preview: {
      requestId: payload.requestId,
      sourceRevision: payload.sourceRevision,
      completedRevision: payload.requestId === snapshot.preview.requestId ? snapshot.preview.completedRevision : 0,
      settled: false,
    },
  };
}

export function markPreviewCompleted(payload: PreviewCompletionPayload): void {
  if (payload.requestId !== snapshot.preview.requestId) return;
  snapshot = {
    ...snapshot,
    preview: {
      requestId: payload.requestId,
      sourceRevision: payload.sourceRevision,
      completedRevision: Math.max(snapshot.preview.completedRevision, payload.completedRevision),
      settled: payload.completedRevision >= payload.sourceRevision,
    },
  };
}

export function markSidecarRequested(payload: SidecarRequestPayload): void {
  snapshot = {
    ...snapshot,
    sidecar: createSidecarSnapshot({
      requestId: payload.requestId,
      hookId: payload.hookId,
      documentKey: payload.documentKey,
      revision: payload.requestId === snapshot.sidecar.requestId ? snapshot.sidecar.revision : 0,
      settled: false,
    }),
  };
}

export function markCursorPathRequested(payload: CursorPathRequestPayload): void {
  if (!isCurrentDocument(payload)) return;
  snapshot = {
    ...snapshot,
    cursorPath: createCursorPathSnapshot({
      requestId: payload.requestId,
      documentKey: payload.documentKey,
      revision: payload.revision,
      lineNumber: payload.lineNumber,
      column: payload.column,
      syncGraphHighlight: payload.syncGraphHighlight,
      settled: false,
    }),
  };
}

export function markCursorPathSettled(payload: CursorPathSettledPayload): void {
  if (!isCurrentDocument(payload)) return;
  if (payload.requestId !== snapshot.cursorPath.requestId) return;
  if (payload.documentKey !== snapshot.cursorPath.documentKey) return;
  if (payload.revision !== snapshot.cursorPath.revision) return;
  if (payload.lineNumber !== snapshot.cursorPath.lineNumber) return;
  if (payload.column !== snapshot.cursorPath.column) return;
  snapshot = {
    ...snapshot,
    cursorPath: {
      ...snapshot.cursorPath,
      settled: true,
    },
  };
}

export function markSidecarSettled(payload: SidecarSettledPayload): void {
  if (payload.requestId !== snapshot.sidecar.requestId) return;
  if (payload.hookId !== snapshot.sidecar.hookId) return;
  snapshot = {
    ...snapshot,
    sidecar: createSidecarSnapshot({
      requestId: payload.requestId,
      hookId: payload.hookId,
      documentKey: payload.documentKey,
      revision: Math.max(snapshot.sidecar.revision, payload.revision),
      settled: true,
    }),
  };
}

export function markSubgraphRequested(payload: SubgraphRequestPayload): void {
  if (payload.sourceRevision < snapshot.subgraph.sourceRevision) return;
  snapshot = {
    ...snapshot,
    subgraph: {
      requestId: payload.requestId,
      pathKey: payload.pathKey,
      sourceRevision: payload.sourceRevision,
      materializedRevision: 0,
      interactiveRevision: 0,
      settled: false,
    },
  };
}

export function markSubgraphMaterialized(payload: SubgraphMaterializedPayload): void {
  if (payload.requestId !== snapshot.subgraph.requestId || payload.pathKey !== snapshot.subgraph.pathKey) return;
  snapshot = {
    ...snapshot,
    subgraph: {
      ...snapshot.subgraph,
      materializedRevision: Math.max(snapshot.subgraph.materializedRevision, payload.materializedRevision),
    },
  };
}

export function syncSubgraphInteractionReadiness(payload: SubgraphInteractivePayload): void {
  if (payload.requestId !== snapshot.subgraph.requestId || payload.pathKey !== snapshot.subgraph.pathKey) return;
  const interactiveRevision = payload.interactiveReady
    ? Math.max(snapshot.subgraph.interactiveRevision, payload.interactiveRevision)
    : snapshot.subgraph.interactiveRevision;
  snapshot = {
    ...snapshot,
    subgraph: {
      ...snapshot.subgraph,
      interactiveRevision,
      settled: interactiveRevision >= snapshot.subgraph.sourceRevision,
    },
  };
}

export function readRuntimeReadiness(): RuntimeReadinessSnapshot {
  return {
    ...snapshot,
    cursorPath: { ...snapshot.cursorPath },
    sidecar: { ...snapshot.sidecar },
    import: { ...snapshot.import },
    graph: { ...snapshot.graph },
    preview: { ...snapshot.preview },
    subgraph: { ...snapshot.subgraph },
  };
}
