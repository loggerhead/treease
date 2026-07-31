import type { GraphHighlightState } from '../../store/editor-store-types';
import type { LeaferBox } from './model';

type GraphSelectionDecorationDeps = {
  resolveDecorations: (highlight: GraphHighlightState) => LeaferBox[];
};

/** Projects the one shared graph selection into renderer-owned decoration slots. */
export function createGraphSelectionDecorationController(deps: GraphSelectionDecorationDeps) {
  let activeDecorations: LeaferBox[] = [];

  function clear(): void {
    activeDecorations.forEach((decoration) => {
      decoration.visible = false;
    });
    activeDecorations = [];
  }

  function sync(highlight: GraphHighlightState | null): void {
    clear();
    if (!highlight?.path.length) return;
    activeDecorations = [...new Set(deps.resolveDecorations(highlight))];
    activeDecorations.forEach((decoration) => {
      decoration.visible = true;
    });
  }

  return { clear, sync };
}
