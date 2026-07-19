// Responsibility: Worker-side tree-path handler; query tree paths, spans, and reveals through the active snapshot.
import { querySnapshot, type PathSeg, type QueryResult, type SnapshotId, type SnapshotReadResult } from '@core-wasm/index';
import { pathSegKeyValue } from '../../shared/path';
import { PathSegTag, type PathSpan } from '@core-wasm/index';
import type { GraphSearchTarget, WorkerRequest } from './protocol';

const textEncoder = new TextEncoder();

export type LazyPathTarget = {
  path?: PathSeg[];
  meta?: { path?: PathSeg[] };
};

export type PathResolverOptions = {
  documentKey: string;
  language: string;
  text: string;
  snapshotId?: SnapshotId | null;
  memo?: Map<string, PathSeg[]>;
  updateTarget?: boolean;
};

export function toPathSeg(tag: PathSegTag, key: string, index: number): PathSeg {
  return { tag, key: key as unknown as PathSeg['key'], index } as PathSeg;
}

export function normalizePathSegs(path: PathSeg[]): PathSeg[] {
  if (!Array.isArray(path)) return [];
  return path.map((seg) => {
    if (seg.tag === PathSegTag.KEY) {
      return toPathSeg(PathSegTag.KEY, seg.key, seg.index ?? 0);
    }
    return toPathSeg(PathSegTag.INDEX, '', seg.index ?? 0);
  });
}

// NOTE: A namesake `buildPathKey` also exists in
// `apps/web/src/lib/graph/graph-viewer-path.ts` (identical logic).
// It uses `isPathSegKey` helper from store/tree-path; here we use
// `PathSegTag.KEY` directly. Keep in sync if changing the format.
export function buildPathKey(path: PathSeg[]): string {
  if (!path || path.length === 0) return '';
  return path.map((seg) => (seg.tag === PathSegTag.KEY ? `k:${pathSegKeyValue(seg)}` : `i:${seg.index}`)).join('|');
}

export function buildPathText(path: PathSeg[]): string {
  if (!path || path.length === 0) return '$';
  const text = path.map((seg) => (seg.tag === PathSegTag.KEY ? `.${pathSegKeyValue(seg)}` : `[${seg.index}]`)).join('');
  if (text.startsWith('[')) return `$${text}`;
  return text.replace(/^\./, '$.');
}

export function createPathResolver(options: PathResolverOptions): (target: LazyPathTarget) => Promise<PathSeg[]> {
  const { updateTarget } = options;
  return async (target: LazyPathTarget): Promise<PathSeg[]> => {
    const directPath = Array.isArray(target?.path) ? target.path : undefined;
    if (directPath && directPath.length > 0) return directPath;
    const metaPath = Array.isArray(target?.meta?.path) ? target.meta.path : undefined;
    if (metaPath && metaPath.length > 0) {
      if (updateTarget && Array.isArray(target?.path) && target.path.length === 0) {
        target.path = metaPath;
      }
      return metaPath;
    }
    return [];
  };
}

function byteOffsetFromRowColumn(text: string, row: number, column: number): number {
  const lines = text.split('\n');
  let byteOffset = 0;
  for (let index = 0; index < row; index += 1) {
    byteOffset += textEncoder.encode(lines[index] ?? '').length + 1;
  }
  return byteOffset + Math.max(0, column);
}

function byteOffsetToRowColumn(text: string, byteOffset: number): { row: number; column: number } {
  const safeByteOffset = Math.max(0, byteOffset);
  const lines = text.split('\n');
  let offset = 0;
  for (let row = 0; row < lines.length; row += 1) {
    const lineBytes = textEncoder.encode(lines[row] ?? '').length;
    if (safeByteOffset <= offset + lineBytes) {
      return { row, column: Math.max(0, safeByteOffset - offset) };
    }
    offset += lineBytes + 1;
  }
  return { row: Math.max(0, lines.length - 1), column: Math.max(0, safeByteOffset - offset) };
}

export function serializePath(path: PathSeg[]): string {
  if (!path.length) return '$';
  return path.reduce((acc, segment) => {
    if (segment.tag === PathSegTag.INDEX) {
      return `${acc}[${segment.index ?? 0}]`;
    }
    const key = pathSegKeyValue(segment);
    return /^[A-Za-z0-9_$]+$/.test(key) ? `${acc}.${key}` : `${acc}[${JSON.stringify(key)}]`;
  }, '$');
}

export function parseAnchorPath(path?: string | null): PathSeg[] {
  if (!path || path === '$') return [];
  const segments: PathSeg[] = [];
  let index = path.startsWith('$') ? 1 : 0;
  while (index < path.length) {
    if (path[index] === '.') {
      index += 1;
      const start = index;
      while (index < path.length && /[A-Za-z0-9_$]/.test(path[index] ?? '')) {
        index += 1;
      }
      if (start === index) return [];
      segments.push(toPathSeg(PathSegTag.KEY, path.slice(start, index), 0));
      continue;
    }
    if (path[index] === '[') {
      const end = path.indexOf(']', index + 1);
      if (end < 0) return [];
      const inner = path.slice(index + 1, end).trim();
      if (inner.startsWith('"')) {
        segments.push(toPathSeg(PathSegTag.KEY, JSON.parse(inner), 0));
      } else {
        segments.push(toPathSeg(PathSegTag.INDEX, '', Number.parseInt(inner, 10) || 0));
      }
      index = end + 1;
      continue;
    }
    return [];
  }
  return segments;
}


export async function resolveLazyPath(
  documentKey: string,
  language: string,
  text: string,
  target: LazyPathTarget,
  snapshotId: SnapshotId | null,
): Promise<PathSeg[]> {
  const resolve = createPathResolver({ documentKey, language, text, snapshotId });
  return resolve(target);
}

export async function resolveSearchRevealTarget(
  documentKey: string,
  _language: string,
  _text: string,
  path: PathSeg[],
  target: GraphSearchTarget,
  _nest: boolean,
  snapshotId: SnapshotId | null,
): Promise<'key' | 'value' | null> {
  if (snapshotId == null || path.length === 0) return null;
  const candidates: Array<'key' | 'value'> = target === 'key' ? ['key', 'value'] : ['value', 'key'];
  for (const candidate of candidates) {
    try {
      const result: SnapshotReadResult<QueryResult> = await querySnapshot({
        documentKey,
        snapshotId,
        queryKind: 'findAnchors',
        pathPattern: serializePath(path),
        target: candidate,
      });
      if (result.status !== 'ready') continue;
      const anchor = result.data.anchors[0];
      if (anchor && anchor.spanEnd >= anchor.spanStart) return candidate;
    } catch (error) {
      console.debug('[tree-path] querySnapshot reveal target failed', { candidate }, error);
    }
  }
  return null;
}

function readMessageText(message: { text?: string | null }): string {
  return message.text ?? '';
}

export async function handleTreePath(
  message: Extract<WorkerRequest, { type: 'treePath' }>,
): Promise<PathSeg[] | { status: 'snapshotNotReady' }> {
  const text = readMessageText(message);
  const snapshotId = message.snapshotId;
  if (snapshotId == null) return { status: 'snapshotNotReady' };
  const byteOffset = byteOffsetFromRowColumn(text, message.row, message.column);
  const data: SnapshotReadResult<QueryResult> = await querySnapshot({
    documentKey: message.documentKey,
    snapshotId,
    queryKind: 'resolvePath',
    spanStart: byteOffset,
    spanEnd: byteOffset,
  });
  return data.status === 'ready' ? parseAnchorPath(data.data.anchors[0]?.path) : { status: 'snapshotNotReady' };
}

export async function handlePathSpan(
  message: Extract<WorkerRequest, { type: 'pathSpan' }>,
): Promise<PathSpan | { status: 'snapshotNotReady' }> {
  const text = readMessageText(message);
  const snapshotId = message.snapshotId;
  if (snapshotId == null) return { status: 'snapshotNotReady' };
  const result: SnapshotReadResult<QueryResult> = await querySnapshot({
    documentKey: message.documentKey,
    snapshotId,
    queryKind: 'findAnchors',
    pathPattern: serializePath(message.path ?? []),
    target: message.target === 'key' ? 'key' : 'value',
  });
  if (result.status !== 'ready') {
    return { status: 'snapshotNotReady' };
  }
  const anchor = result.data.anchors[0];
  if (!anchor || anchor.spanEnd < anchor.spanStart) {
    return { startByte: -1, endByte: -1, row: -1, column: -1 } as PathSpan;
  }
  const start = byteOffsetToRowColumn(text, anchor.spanStart);
  return {
    startByte: anchor.spanStart,
    endByte: anchor.spanEnd,
    row: start.row,
    column: start.column,
  } as PathSpan;
}
