import type { GraphCell, GraphCellKind } from '../../../graph/graph-viewer-render';
import type { CellBoxEntry, LeaferBox, ScrollableBox } from '../model';

export function toGraphClickTarget(kind: GraphCellKind): 'key' | 'value' | 'node' {
  if (kind === 'key') return 'key';
  if (kind === 'value') return 'value';
  return 'node';
}

export function createGraphRenderBindings(
  graphRenderState: {
    upsertCellEntry: (cell: GraphCell, updater: (entry: CellBoxEntry) => void) => void;
    updateCellEntry: (cell: GraphCell, updater: (entry: CellBoxEntry) => void) => void;
    registerCellBox: (cell: GraphCell, kind: GraphCellKind, box: LeaferBox) => void;
    unregisterCellBox: (cell: GraphCell, kind: GraphCellKind, box: LeaferBox) => void;
    registerRowBox: (
      cell: GraphCell,
      rowBox: LeaferBox,
      scrollOwner?: ScrollableBox,
      bodyHeight?: number,
      contentHeight?: number,
    ) => void;
    unregisterRowBox: (cell: GraphCell, rowBox: LeaferBox) => void;
  },
) {
  return {
    upsertCellEntry: (_map: Map<string, CellBoxEntry>, cell: GraphCell, updater: (entry: CellBoxEntry) => void) =>
      graphRenderState.upsertCellEntry(cell, updater),
    updateCellEntry: (_map: Map<string, CellBoxEntry>, cell: GraphCell, updater: (entry: CellBoxEntry) => void) =>
      graphRenderState.updateCellEntry(cell, updater),
    registerCellBox: (cell: GraphCell, kind: GraphCellKind, box: LeaferBox) =>
      graphRenderState.registerCellBox(cell, kind, box),
    unregisterCellBox: (cell: GraphCell, kind: GraphCellKind, box: LeaferBox) =>
      graphRenderState.unregisterCellBox(cell, kind, box),
    registerRowBox: (
      cell: GraphCell,
      rowBox: LeaferBox,
      scrollOwner?: ScrollableBox,
      bodyHeight?: number,
      contentHeight?: number,
    ) => graphRenderState.registerRowBox(cell, rowBox, scrollOwner, bodyHeight, contentHeight),
    unregisterRowBox: (cell: GraphCell, rowBox: LeaferBox) => graphRenderState.unregisterRowBox(cell, rowBox),
  };
}
