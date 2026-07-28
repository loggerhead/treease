import { buildPathKey } from '../../graph/graph-viewer-path';
import type { GraphCell, GraphCellKind } from '@treease/graph-viewer-runtime';
import type { PathSeg } from '../../store/tree-path';
import type { CellBoxEntry, LeaferBox, ScrollableBox } from './model';

export type GraphAnchorTarget = 'key' | 'value' | 'node';

export type ResolveTreePathByPosition = (row: number, column: number) => Promise<PathSeg[]>;

function buildCellEntryPathKey(cell: GraphCell | null | undefined): string {
  if (!cell) return '';
  return buildPathKey(cell.path ?? []);
}

export function getCellEntry(map: Map<string, CellBoxEntry>, path: PathSeg[] | null | undefined): CellBoxEntry | null {
  const pathKey = buildPathKey(path ?? []);
  if (!pathKey) return null;
  return map.get(pathKey) ?? null;
}

export function upsertCellEntry(map: Map<string, CellBoxEntry>, cell: GraphCell, updater: (entry: CellBoxEntry) => void): void {
  const pathKey = buildCellEntryPathKey(cell);
  if (!pathKey) return;
  const entry = map.get(pathKey) ?? {};
  updater(entry);
  map.set(pathKey, entry);
}

export function updateCellEntry(map: Map<string, CellBoxEntry>, cell: GraphCell, updater: (entry: CellBoxEntry) => void): void {
  const pathKey = buildCellEntryPathKey(cell);
  if (!pathKey) return;
  const entry = map.get(pathKey);
  if (!entry) return;
  updater(entry);
  const hasRenderableTarget = !!entry.key || !!entry.value || !!entry.row;
  if (!hasRenderableTarget) {
    entry.cell = undefined;
    entry.scrollOwner = undefined;
    entry.bodyHeight = undefined;
    entry.contentHeight = undefined;
  }
  const current = map.get(pathKey);
  if (current !== entry) return;
  if (hasRenderableTarget) {
    map.set(pathKey, entry);
  } else {
    map.delete(pathKey);
  }
}

export function registerCellBox(
  map: Map<string, CellBoxEntry>,
  cell: GraphCell,
  kind: GraphCellKind,
  box: LeaferBox,
): void {
  if (kind !== 'key' && kind !== 'value') return;
  upsertCellEntry(map, cell, (entry) => {
    if (kind === 'key') entry.key = box;
    if (kind === 'value') entry.value = box;
  });
}

export function unregisterCellBox(
  map: Map<string, CellBoxEntry>,
  cell: GraphCell,
  kind: GraphCellKind,
  box: LeaferBox,
): void {
  if (kind !== 'key' && kind !== 'value') return;
  updateCellEntry(map, cell, (entry) => {
    if (kind === 'key' && entry.key === box) entry.key = undefined;
    if (kind === 'value' && entry.value === box) entry.value = undefined;
  });
}

export function registerRowBox(
  map: Map<string, CellBoxEntry>,
  cell: GraphCell,
  rowBox: LeaferBox,
  scrollOwner?: ScrollableBox,
  bodyHeight?: number,
  contentHeight?: number,
): void {
  upsertCellEntry(map, cell, (entry) => {
    entry.row = rowBox;
    entry.cell = cell;
    if (scrollOwner) entry.scrollOwner = scrollOwner;
    if (bodyHeight) entry.bodyHeight = bodyHeight;
    if (contentHeight) entry.contentHeight = contentHeight;
  });
}

export function unregisterRowBox(map: Map<string, CellBoxEntry>, cell: GraphCell, rowBox: LeaferBox): void {
  updateCellEntry(map, cell, (entry) => {
    if (entry.row !== rowBox) return;
    entry.row = undefined;
    entry.scrollOwner = undefined;
    entry.bodyHeight = undefined;
    entry.contentHeight = undefined;
  });
}

export function createCellEntryBindings(map: Map<string, CellBoxEntry>) {
  return {
    registerCellBox: (cell: GraphCell, kind: GraphCellKind, box: LeaferBox): void => {
      registerCellBox(map, cell, kind, box);
    },
    unregisterCellBox: (cell: GraphCell, kind: GraphCellKind, box: LeaferBox): void => {
      unregisterCellBox(map, cell, kind, box);
    },
    registerRowBox: (
      cell: GraphCell,
      rowBox: LeaferBox,
      scrollOwner?: ScrollableBox,
      bodyHeight?: number,
      contentHeight?: number,
    ): void => {
      registerRowBox(map, cell, rowBox, scrollOwner, bodyHeight, contentHeight);
    },
    unregisterRowBox: (cell: GraphCell, rowBox: LeaferBox): void => {
      unregisterRowBox(map, cell, rowBox);
    },
  };
}

export function getAnchor(entry: CellBoxEntry | null | undefined, target: GraphAnchorTarget): LeaferBox | null {
  if (!entry) return null;
  if (target === 'key') return entry.key ?? entry.value ?? entry.row ?? null;
  if (target === 'value') return entry.value ?? entry.key ?? entry.row ?? null;
  return entry.row ?? entry.value ?? entry.key ?? null;
}

export function getScrollContext(
  entry: CellBoxEntry | null | undefined,
): { row: LeaferBox; scrollOwner: ScrollableBox; bodyHeight: number; contentHeight: number } | null {
  if (!entry?.row || !entry.scrollOwner || !entry.bodyHeight || !entry.contentHeight) return null;
  return {
    row: entry.row,
    scrollOwner: entry.scrollOwner,
    bodyHeight: entry.bodyHeight,
    contentHeight: entry.contentHeight,
  };
}

export function getHighlightTarget(
  entry: CellBoxEntry | null | undefined,
  preferredTarget?: GraphAnchorTarget,
): { target: GraphAnchorTarget; box: LeaferBox | null } {
  if (preferredTarget === 'key' && entry?.key) {
    return { target: 'key', box: entry.key };
  }
  if (preferredTarget === 'value' && entry?.value) {
    return { target: 'value', box: entry.value };
  }
  if (entry?.value) {
    return { target: 'value', box: entry.value };
  }
  if (entry?.key) {
    return { target: 'key', box: entry.key };
  }
  if (entry?.row) {
    return { target: 'node', box: entry.row };
  }
  return { target: preferredTarget ?? 'node', box: null };
}

export async function resolveCellPath(
  cell: GraphCell,
  _resolveTreePathByPosition: ResolveTreePathByPosition,
  fallbackPath: PathSeg[] = cell.path ?? [],
): Promise<PathSeg[]> {
  if (fallbackPath.length) return fallbackPath;
  return cell.path ?? [];
}

export async function resolveInteractiveCellPath(
  cell: GraphCell,
  fallbackPath: PathSeg[],
  resolveTreePathByPosition: ResolveTreePathByPosition,
): Promise<PathSeg[]> {
  if (!cell?.isTableCell || cell.isHeader) return fallbackPath;
  return resolveCellPath(cell, resolveTreePathByPosition, fallbackPath);
}
