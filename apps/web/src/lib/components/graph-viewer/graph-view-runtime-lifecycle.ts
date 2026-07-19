import type { JsonBlockSelection } from '../../store/editor-store-types';
import type { FullEditUiState } from '../../store/editor-store-types';

type FullEditRenderSnapshot = {
  hasRenderRuntime: boolean;
  documentKey: string;
  language: string;
  sourceText: string;
};

type IncrementalRenderSnapshot = {
  hasRenderRuntime: boolean;
  isBlocked: boolean;
  documentKey: string;
  language: string;
  sourceText: string;
  editorRevision: number;
  graphAppliedRevision: number;
};

export type GraphViewRuntimeRenderInput = {
  fullEditUiState: FullEditUiState | null | undefined;
  jsonBlockSelection: JsonBlockSelection | null;
  renderRuntimeReady: boolean;
  documentKey: string;
  language: string;
  sourceText: string;
  editorRevision: number;
  graphAppliedRevision: number;
  lastAutoOffset: { x: number; y: number } | null;
};

export type GraphViewRuntimeRenderGuard = {
  documentKey: string;
  revision: number;
  mode?: 'committed' | 'streaming' | 'json-block';
};

export function isGraphViewRuntimeRenderCurrent(
  guard: GraphViewRuntimeRenderGuard | null,
  input: {
    documentKey: string;
    revision: number;
    jsonBlockSelection: JsonBlockSelection | null;
  },
): boolean {
  if (!guard) return false;
  if (guard.mode === 'json-block') {
    return (
      input.jsonBlockSelection?.blockDocumentKey === guard.documentKey &&
      input.jsonBlockSelection.revision === guard.revision
    );
  }
  return guard.documentKey === input.documentKey && guard.revision === input.revision;
}

type GraphViewRuntimeLifecycleDeps = {
  fullBuildReasons: ReadonlySet<string | null>;
  setLastAutoOffset: (value: { x: number; y: number } | null) => void;
  isFullEditProgressActive: () => boolean;
  completeStreamProgress: () => void;
  attachFullEditSession: (state: FullEditUiState | null | undefined, snapshot: FullEditRenderSnapshot) => void;
  renderJsonBlock: (selection: JsonBlockSelection | null, hasRenderRuntime: boolean) => void;
  renderIncremental: (snapshot: IncrementalRenderSnapshot) => void;
  scheduleFullEditCleanup: (kind: 'settled' | 'idle', task: () => void) => void;
  updateMinimap: () => void;
};

/**
 * Graph View Runtime rendering lifecycle: keep one UI commit order across full-edit,
 * JSON-block, and incremental visibility modes. Document Runtime remains the authority
 * for snapshots and freshness; this layer only decides whether stale or handled results
 * may still update View Runtime.
 */
export function createGraphViewRuntimeLifecycle(deps: GraphViewRuntimeLifecycleDeps) {
  let lastFullEditProgressActive = false;
  let lastFullEditIncrementalActive = false;
  let lastActiveFullEditDocumentKey = '';
  let lastActiveFullEditRevision = -1;
  let lastFullEditHandledDocumentKey = '';
  let lastFullEditHandledRevision = -1;

  function syncRender(input: GraphViewRuntimeRenderInput): void {
    const state = input.fullEditUiState;
    const fullEditProgressActive = deps.isFullEditProgressActive();

    if (lastFullEditProgressActive && !fullEditProgressActive) {
      deps.completeStreamProgress();
    }
    if (fullEditProgressActive) {
      lastActiveFullEditDocumentKey = state?.documentKey ?? input.documentKey;
      lastActiveFullEditRevision = state?.revision ?? input.editorRevision;
    }
    lastFullEditProgressActive = fullEditProgressActive;

    if (state?.active && state.phase === 'streaming' && input.lastAutoOffset != null && deps.fullBuildReasons.has(state.reason)) {
      deps.setLastAutoOffset(null);
    }
    deps.attachFullEditSession(state, {
      hasRenderRuntime: input.renderRuntimeReady,
      documentKey: input.documentKey,
      language: input.language,
      sourceText: input.sourceText,
    });

    if (lastFullEditIncrementalActive && !fullEditProgressActive) {
      lastFullEditHandledDocumentKey = lastActiveFullEditDocumentKey || input.documentKey;
      lastFullEditHandledRevision = lastActiveFullEditRevision >= 0 ? lastActiveFullEditRevision : input.editorRevision;
    }
    lastFullEditIncrementalActive = fullEditProgressActive;

    const fullEditHandled =
      lastFullEditHandledDocumentKey === input.documentKey && lastFullEditHandledRevision === input.editorRevision;
    deps.renderJsonBlock(input.jsonBlockSelection, input.renderRuntimeReady);
    deps.renderIncremental({
      hasRenderRuntime: input.renderRuntimeReady,
      isBlocked: fullEditProgressActive || fullEditHandled || Boolean(input.jsonBlockSelection),
      documentKey: input.documentKey,
      language: input.language,
      sourceText: input.sourceText,
      editorRevision: input.editorRevision,
      graphAppliedRevision: input.graphAppliedRevision,
    });
  }

  function settle(state: FullEditUiState | null | undefined, documentKey: string): void {
    if (!state?.active) return;
    if (state.phase === 'settled') {
      lastFullEditHandledDocumentKey = state.documentKey ?? documentKey;
      lastFullEditHandledRevision = state.revision;
      deps.scheduleFullEditCleanup('settled', () => {
        deps.completeStreamProgress();
        deps.updateMinimap();
      });
      return;
    }
    if (state.phase === 'idle') {
      deps.scheduleFullEditCleanup('idle', () => {
        deps.completeStreamProgress();
      });
    }
  }

  function reset(): void {
    lastFullEditProgressActive = false;
    lastFullEditIncrementalActive = false;
    lastActiveFullEditDocumentKey = '';
    lastActiveFullEditRevision = -1;
    lastFullEditHandledDocumentKey = '';
    lastFullEditHandledRevision = -1;
  }

  return { syncRender, settle, reset };
}

export function disposeGraphViewRuntime(deps: {
  cleanupHandles: { settled: number | null; idle: number | null };
  cancelFrame: (handle: number) => void;
  resetCanvasHint: () => void;
  disposeMeasurement: () => void;
  disposeRenderEffects: () => void;
  disposeRenderCoordinator: () => Promise<void>;
  disposeScene: () => void;
  resetActiveEditState: () => void;
  disposeSubgraphWorkspace: () => void;
  unsubscribeStreamProgress: () => void;
  disposeStreamProgress: () => void;
  resetLifecycle: () => void;
  clearGraphBridge: () => void;
  resetGraphStreamState: () => void;
}): void {
  if (deps.cleanupHandles.settled != null) deps.cancelFrame(deps.cleanupHandles.settled);
  if (deps.cleanupHandles.idle != null) deps.cancelFrame(deps.cleanupHandles.idle);
  deps.resetCanvasHint();
  deps.disposeMeasurement();
  deps.disposeRenderEffects();
  void deps.disposeRenderCoordinator();
  deps.disposeScene();
  deps.resetActiveEditState();
  deps.disposeSubgraphWorkspace();
  deps.unsubscribeStreamProgress();
  deps.disposeStreamProgress();
  deps.resetLifecycle();
  deps.clearGraphBridge();
  deps.resetGraphStreamState();
}
