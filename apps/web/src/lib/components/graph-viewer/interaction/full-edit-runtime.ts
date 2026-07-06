import type { FullEditUiState } from '../../../store/editor-store-types';
import type { LeaferAppLike } from '../model';
import type { GraphSceneViewData } from '../rendering';

export function isFullEditProgressActiveState(fullEditUiState: FullEditUiState | null | undefined): boolean {
  return fullEditUiState?.active === true && fullEditUiState.phase !== 'idle';
}

export function closeRuntimeInnerEditor(leafer: LeaferAppLike | null): void {
  const editor = (leafer as ({ editor?: { closeInnerEditor?: (skipUpdate?: boolean) => void } } & object) | null)
    ?.editor;
  editor?.closeInnerEditor?.(true);
}

export function syncReadonlyRuntimeState(
  leafer: LeaferAppLike | null,
  getGraphData: () => GraphSceneViewData | null,
  replaceAll: (graphData: GraphSceneViewData) => void,
  updateMinimap: () => void,
  resetActiveEditState: () => void,
  isInteractionBlocked: () => boolean,
): void {
  resetActiveEditState();
  closeRuntimeInnerEditor(leafer);
  const graphData = getGraphData();
  if (!graphData) return;
  replaceAll(graphData);
  if (!isInteractionBlocked()) updateMinimap();
}

export function scheduleAnimationCleanup(
  kind: 'settled' | 'idle',
  handles: { settled: number | null; idle: number | null },
  setHandles: (handles: { settled: number | null; idle: number | null }) => void,
  requestFrame: typeof requestAnimationFrame,
  task: () => void,
): void {
  if (kind === 'settled') {
    if (handles.settled != null) return;
    const nextSettled = requestFrame(() => {
      setHandles({ settled: null, idle: handles.idle });
      task();
    });
    setHandles({ settled: nextSettled, idle: handles.idle });
    return;
  }
  if (handles.idle != null) return;
  const nextIdle = requestFrame(() => {
    setHandles({ settled: handles.settled, idle: null });
    task();
  });
  setHandles({ settled: handles.settled, idle: nextIdle });
}

export function createGraphFullEditRuntime(options: {
  getFullEditUiState: () => FullEditUiState | null | undefined;
  getLeafer: () => LeaferAppLike | null;
  getGraphData: () => GraphSceneViewData | null;
  replaceAll: (graphData: GraphSceneViewData) => void;
  updateMinimap: () => void;
  resetActiveEditState: () => void;
  isInteractionBlocked: () => boolean;
  requestFrame: typeof requestAnimationFrame;
  completeStreamProgress: () => void;
  getCleanupHandles: () => { settled: number | null; idle: number | null };
  setCleanupHandles: (handles: { settled: number | null; idle: number | null }) => void;
}) {
  return {
    isProgressActive(): boolean {
      return isFullEditProgressActiveState(options.getFullEditUiState());
    },
    completeStreamProgress(): void {
      options.completeStreamProgress();
    },
    scheduleCleanup(kind: 'settled' | 'idle', task: () => void): void {
      scheduleAnimationCleanup(
        kind,
        options.getCleanupHandles(),
        options.setCleanupHandles,
        options.requestFrame,
        task,
      );
    },
    closeInnerEditor(): void {
      closeRuntimeInnerEditor(options.getLeafer());
    },
    syncReadonlyEditability(): void {
      syncReadonlyRuntimeState(
        options.getLeafer(),
        options.getGraphData,
        options.replaceAll,
        options.updateMinimap,
        options.resetActiveEditState,
        options.isInteractionBlocked,
      );
    },
  };
}
