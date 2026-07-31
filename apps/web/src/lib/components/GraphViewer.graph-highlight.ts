import type { TempModel } from '../store/graph-selection-store';
import type { PathSeg } from '../store/tree-path';

export function clearGraphSelectionAfterEdit(current: TempModel, _editPath: PathSeg[]): TempModel {
  // A value edit preserves its path. Scene replacement owns only the old box;
  // the committed snapshot decides whether this logical selection still exists.
  return current;
}

export function clearGraphSelectionForFullEdit(current: TempModel): TempModel {
  if (!current.treePath.length && !current.graphHighlight) return current;
  return {
    ...current,
    treePath: [],
    graphHighlight: null,
  };
}
