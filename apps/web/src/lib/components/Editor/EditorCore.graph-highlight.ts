import type { GraphHighlightTarget, TempModel } from '../../store/graph-selection-store';
import type { PathSeg } from '../../store/tree-path';

export const editorDrivenCursorReasons = {
  explicit: 3,
  paste: 4,
  undo: 5,
  redo: 6,
} as const;

export function shouldSyncGraphHighlightFromCursorReason(reason: number): boolean {
  // Monaco reports paste/undo/redo as cursor movements. They are document
  // edits, not navigation facts, so only an explicit cursor gesture qualifies.
  return reason === editorDrivenCursorReasons.explicit;
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
  return {
    ...current,
    treePath,
    ...(syncGraphHighlight
      ? {
          graphHighlight: treePath.length
            ? {
                path: treePath,
                target,
                revision,
                source: 'editor',
              }
            : null,
        }
      : {}),
  };
}

export function applyFailedTreePath(
  current: TempModel,
  syncGraphHighlight: boolean,
): TempModel {
  return {
    ...current,
    treePath: [],
    ...(syncGraphHighlight ? { graphHighlight: null } : {}),
  };
}
