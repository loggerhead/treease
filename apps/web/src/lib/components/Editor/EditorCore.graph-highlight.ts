import type { GraphHighlightTarget, TempModel } from '../../store/graph-selection-store';
import type { PathSeg } from '../../store/tree-path';

export const editorDrivenCursorReasons = {
  explicit: 3,
  paste: 4,
  undo: 5,
  redo: 6,
} as const;

export function shouldSyncGraphHighlightFromCursorReason(reason: number): boolean {
  return (
    reason === editorDrivenCursorReasons.explicit ||
    reason === editorDrivenCursorReasons.paste ||
    reason === editorDrivenCursorReasons.undo ||
    reason === editorDrivenCursorReasons.redo
  );
}

export function applyResolvedTreePath(
  current: TempModel,
  options: {
    treePath: PathSeg[];
    target?: GraphHighlightTarget;
    revision: number;
    syncGraphHighlight: boolean;
  },
): TempModel {
  const { treePath, target, revision, syncGraphHighlight } = options;
  if (!syncGraphHighlight) {
    return {
      ...current,
      treePath,
    };
  }
  return {
    ...current,
    treePath,
    graphHighlight: treePath.length
      ? {
          path: treePath,
          target,
          revision,
          source: 'editor',
        }
      : null,
  };
}

export function applyFailedTreePath(current: TempModel, syncGraphHighlight: boolean): TempModel {
  if (!syncGraphHighlight) {
    return {
      ...current,
      treePath: [],
    };
  }
  return {
    ...current,
    treePath: [],
    graphHighlight: null,
  };
}
