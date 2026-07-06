import type { TempModel } from '../store/graph-selection-store';
import type { PathSeg } from '../store/tree-path';

export function clearGraphSelectionAfterEdit(current: TempModel, _editPath: PathSeg[]): TempModel {
  const highlight = current.graphHighlight;
  if (!highlight?.path?.length) return current;
  if (highlight.source !== 'graph') return current;
  return {
    ...current,
    treePath: [],
    graphHighlight: null,
  };
}

export function clearGraphSelectionForFullEdit(current: TempModel): TempModel {
  if (!current.treePath.length && !current.graphHighlight) return current;
  return {
    ...current,
    treePath: [],
    graphHighlight: null,
  };
}
