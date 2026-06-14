import type { PathSeg } from '@core-wasm/index';
import { PathSegTag } from '@core-wasm/index';
import { toWasmPathSeg } from '../../src/shared/brand-bridge';
import { callSharedWasmWorker } from '../../src/lib/wasm/wasm-worker-singleton';

export type PathAnchor = { row: number; column: number };
export type PathSpan = { startByte: number; endByte: number; row: number; column: number };

export type GraphCellLike = {
  text?: string;
  path?: PathSeg[];
};

export type GraphRowLike = {
  cells?: GraphCellLike[];
};

export type GraphNodeLike = {
  renderHandle?: number;
  kind?: 'scalar' | 'object' | 'table';
  path?: PathSeg[];
  meta?: GraphCellLike;
  rows?: GraphRowLike[];
  table?: { columns?: GraphCellLike[]; rows?: GraphRowLike[] };
};

function readWasmLikeString(value: unknown): string {
  if (typeof value !== 'string') {
    throw new Error(`Expected path key string, received ${Object.prototype.toString.call(value)}`);
  }
  return value;
}

export function isValidAnchor(anchor: PathAnchor | null | undefined): anchor is PathAnchor {
  return Boolean(anchor && anchor.row >= 0 && anchor.column >= 0);
}

export async function resolveAnchor(
  documentKey: string,
  language: string,
  text: string,
  path: PathSeg[],
  target: 'key' | 'value' | 'node',
): Promise<PathAnchor | null> {
  const candidates: Array<'key' | 'value'> = target === 'key' ? ['key', 'value'] : ['value', 'key'];
  for (const candidate of candidates) {
    const span = await callSharedWasmWorker<PathSpan>('pathSpan', {
      documentKey,
      language,
      text,
      path,
      target: candidate,
      nest: true,
    });
    if (isValidAnchor(span)) return { row: span.row, column: span.column };
  }
  return null;
}

export function resolveAnyAnchor(documentKey: string, language: string, text: string, path: PathSeg[]): Promise<PathAnchor | null> {
  return resolveAnchor(documentKey, language, text, path, 'node');
}

export function formatPath(path: PathSeg[]): string {
  return [
    '$',
    ...path.map((segment) => (segment.tag === PathSegTag.INDEX ? `[${segment.index}]` : String(segment.key ?? ''))),
  ]
    .filter((segment) => segment.length > 0)
    .join('.');
}

function normalizePath(path: unknown): PathSeg[] {
  if (!Array.isArray(path)) return [];
  return path.map((seg) => {
    const candidate = seg as { tag?: number; key?: unknown; index?: number };
    if (candidate?.tag === PathSegTag.KEY) {
      return toWasmPathSeg({ tag: PathSegTag.KEY, key: readWasmLikeString(candidate.key), index: candidate.index ?? 0 });
    }
    return toWasmPathSeg({ tag: PathSegTag.INDEX, key: '', index: candidate?.index ?? 0 });
  });
}

function normalizeCell(cell: unknown): GraphCellLike {
  const candidate = (cell ?? {}) as { text?: string; path?: unknown };
  return {
    text: candidate.text,
    path: normalizePath(candidate.path),
  };
}

function normalizeRow(row: unknown): GraphRowLike {
  const candidate = (row ?? {}) as { cells?: unknown[] };
  return {
    cells: Array.isArray(candidate.cells) ? candidate.cells.map(normalizeCell) : [],
  };
}

export function snapshotGraphNode(node: unknown): GraphNodeLike {
  const candidate = (node ?? {}) as {
    renderHandle?: number;
    kind?: 'scalar' | 'object' | 'table' | number;
    path?: unknown;
    meta?: unknown;
    rows?: unknown[];
    table?: { columns?: unknown[]; rows?: unknown[] };
  };
  return {
    renderHandle: candidate.renderHandle,
    kind:
      candidate.kind === 'table' || candidate.kind === 'object' || candidate.kind === 'scalar'
        ? candidate.kind
        : candidate.kind === 2
          ? 'table'
          : candidate.kind === 1
            ? 'object'
            : candidate.kind === 0
              ? 'scalar'
              : undefined,
    path: normalizePath(candidate.path),
    meta: normalizeCell(candidate.meta),
    rows: Array.isArray(candidate.rows) ? candidate.rows.map(normalizeRow) : [],
    table: candidate.table
      ? {
          columns: Array.isArray(candidate.table.columns) ? candidate.table.columns.map(normalizeCell) : [],
          rows: Array.isArray(candidate.table.rows) ? candidate.table.rows.map(normalizeRow) : [],
        }
      : undefined,
  };
}

export function collectGraphNodesFromDeltas(graphDeltas: unknown[]): GraphNodeLike[] {
  const nodes = new Map<number, GraphNodeLike>();
  for (const delta of graphDeltas) {
    const candidate = delta as {
      clear?: number;
      nodesRemoved?: number[];
      nodesAdded?: unknown[];
      nodesUpdated?: unknown[];
    };
    if (candidate.clear === 1) nodes.clear();
    for (const nodeId of candidate.nodesRemoved ?? []) nodes.delete(nodeId);
    for (const node of candidate.nodesAdded ?? []) {
      const normalized = snapshotGraphNode(node);
      if (typeof normalized.renderHandle === 'number') nodes.set(normalized.renderHandle, normalized);
    }
    for (const node of candidate.nodesUpdated ?? []) {
      const normalized = snapshotGraphNode(node);
      if (typeof normalized.renderHandle === 'number') nodes.set(normalized.renderHandle, normalized);
    }
  }
  return [...nodes.values()];
}

export function collectUniqueCellPaths(nodes: GraphNodeLike[]): PathSeg[][] {
  const seen = new Set<string>();
  const paths: PathSeg[][] = [];
  const pushPath = (path: PathSeg[] | undefined) => {
    if (!path?.length) return;
    const key = JSON.stringify(path);
    if (seen.has(key)) return;
    seen.add(key);
    paths.push(path);
  };
  for (const node of nodes) {
    pushPath(node.meta?.path);
    for (const row of node.rows ?? []) {
      for (const cell of row.cells ?? []) pushPath(cell.path);
    }
    for (const row of node.table?.rows ?? []) {
      for (const cell of row.cells ?? []) pushPath(cell.path);
    }
  }
  return paths;
}
