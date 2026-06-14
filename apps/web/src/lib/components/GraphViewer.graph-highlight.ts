import type { TempModel } from '../store/editor-store';
import type { PathSeg } from '../store/tree-path';

export function clearGraphSelectionAfterEdit(current: TempModel, editPath: PathSeg[]): TempModel {
  void editPath;
  const highlight = current.graphHighlight;
  if (!highlight?.path?.length) return current;
  if (highlight.source !== 'graph') return current;
  return {
    ...current,
    treePath: [],
    graphHighlight: null,
  };
}
