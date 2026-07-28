import type { GraphRow, TableRowRenderEntry } from '../types';

export type TableVisibleRange = {
  start: number;
  end: number;
};

type UpdateTableVirtualWindowInput = {
  rows: GraphRow[];
  rowHeight: number;
  rowStartY: number;
  renderCount: number;
  scrollY: number;
  renderedSlots: Map<number, TableRowRenderEntry>;
  acquireRowSlot: (row: GraphRow) => TableRowRenderEntry;
  bindRowSlot: (entry: TableRowRenderEntry, row: GraphRow, rowIndex: number, options?: { force?: boolean }) => void;
  releaseRow: (rowIndex: number) => void;
  forceRebindVisibleRows?: boolean;
};

export function computeTableRenderCount(params: {
  rowCount: number;
  rowHeight: number;
  viewportHeight: number;
  overscan: number;
}): number {
  const rowsPerViewport =
    params.rowHeight > 0 && params.viewportHeight > 0
      ? Math.ceil(params.viewportHeight / params.rowHeight)
      : params.rowCount;
  return Math.min(params.rowCount, Math.max(rowsPerViewport + params.overscan + 1, rowsPerViewport, 1));
}

export function computeTableVisibleRange(params: {
  rowCount: number;
  rowHeight: number;
  renderCount: number;
  scrollY: number;
}): TableVisibleRange {
  const scrollTop = Math.max(0, params.scrollY);
  const start = params.rowHeight > 0 ? Math.floor(scrollTop / params.rowHeight) : 0;
  const end = Math.min(params.rowCount, start + params.renderCount);
  return { start, end };
}

export function updateTableVirtualWindow(input: UpdateTableVirtualWindowInput): TableVisibleRange {
  const visibleRange = computeTableVisibleRange({
    rowCount: input.rows.length,
    rowHeight: input.rowHeight,
    renderCount: input.renderCount,
    scrollY: input.scrollY,
  });

  for (const rowIndex of input.renderedSlots.keys()) {
    if (rowIndex < visibleRange.start || rowIndex >= visibleRange.end) {
      input.releaseRow(rowIndex);
    }
  }

  for (let rowIndex = visibleRange.start; rowIndex < visibleRange.end; rowIndex += 1) {
    const row = input.rows[rowIndex];
    const existing = input.renderedSlots.get(rowIndex);
    if (existing) {
      if (input.forceRebindVisibleRows) {
        input.bindRowSlot(existing, row, rowIndex, { force: true });
      }
      continue;
    }
    const entry = input.acquireRowSlot(row);
    input.bindRowSlot(entry, row, rowIndex);
  }

  return visibleRange;
}
