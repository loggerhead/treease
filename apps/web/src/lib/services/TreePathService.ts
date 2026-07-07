import type * as Monaco from 'monaco-editor';
import type { QueryResult, QueryTargetKind, SnapshotId, SnapshotReadResult } from '@core-wasm/index';
import { callSharedWasmWorker } from '../wasm/wasm-worker-singleton';
import type { PathSeg } from '../store/tree-path';
import type { SupportedEditorLanguageId } from '../monaco/language-support';
import { PathSegTag } from '@core-wasm/index';
import { serializePath, byteOffsetToRowColumn, byteOffsetToUtf16Offset } from '../../shared/document-anchor-utils';
import { toWasmPathSeg } from '../../shared/brand-bridge';

const textEncoder = new TextEncoder();
type TreePathColumnMode = 'char' | 'byte' | 'auto';
type PathSelectionRange = { start: Monaco.IPosition; end: Monaco.IPosition };
export type PathSpan = {
  startByte: number;
  endByte: number;
  row: number;
  column: number;
};

export type TreePathReadResult = SnapshotReadResult<PathSeg[]>;
export type PathSpanReadResult = SnapshotReadResult<PathSpan | null>;


/**
 * 将编辑器的列索引转换为字节列，用于 treePath 计算。
 * @param lineText 当前行文本
 * @param columnIndex 列索引（字符）
 * @returns 字节列索引
 */
export function toByteColumn(lineText: string, columnIndex: number): number {
  return textEncoder.encode(lineText.slice(0, columnIndex)).length;
}

function resolveTreePathColumn(text: string, row: number, column: number, mode: TreePathColumnMode): number {
  if (mode === 'byte') return Math.max(0, column);
  const lines = text.split('\n');
  const lineText = lines[row]?.replace(/\r$/, '') ?? '';
  return toByteColumn(lineText, Math.max(0, column));
}

function snapshotNotReady<T>(): SnapshotReadResult<T> {
  return { status: 'snapshotNotReady' };
}

function readyTreePath(data: PathSeg[]): TreePathReadResult {
  return { status: 'ready', data };
}

async function callTreePathResult(
  text: string,
  row: number,
  column: number,
  documentKey: string,
  snapshotId: SnapshotId | null,
  _languageId: SupportedEditorLanguageId,
  _nest: boolean,
): Promise<TreePathReadResult> {
  if (snapshotId == null) return snapshotNotReady();
  const byteOffset = byteOffsetFromRowColumn(text, row, column);
  const result = await callSharedWasmWorker<SnapshotReadResult<QueryResult>>('querySnapshot', {
    documentKey,
    snapshotId,
    queryKind: 'resolvePath',
    spanStart: byteOffset,
    spanEnd: byteOffset,
  });
  if (result.status !== 'ready') return snapshotNotReady();
  return readyTreePath(parseAnchorPath(result.data.anchors[0]?.path));
}

function byteOffsetFromRowColumn(text: string, row: number, column: number): number {
  const lines = text.split('\n');
  let byteOffset = 0;
  for (let index = 0; index < row; index += 1) {
    byteOffset += textEncoder.encode(lines[index] ?? '').length + 1;
  }
  return byteOffset + Math.max(0, column);
}



function parseAnchorPath(path?: string | null): PathSeg[] {
  if (!path || path === '$') return [];
  const bytes = path;
  const segments: PathSeg[] = [];
  let index = bytes.startsWith('$') ? 1 : 0;
  while (index < bytes.length) {
    if (bytes[index] === '.') {
      index += 1;
      const start = index;
      while (index < bytes.length && /[A-Za-z0-9_$]/.test(bytes[index] ?? '')) {
        index += 1;
      }
      if (start === index) return [];
      segments.push(toWasmPathSeg({ tag: PathSegTag.KEY, key: bytes.slice(start, index), index: 0 }));
      continue;
    }
    if (bytes[index] === '[') {
      const end = bytes.indexOf(']', index + 1);
      if (end < 0) return [];
      const inner = bytes.slice(index + 1, end).trim();
      if (inner.startsWith('"')) {
        segments.push(toWasmPathSeg({ tag: PathSegTag.KEY, key: JSON.parse(inner), index: 0 }));
      } else {
        segments.push(toWasmPathSeg({ tag: PathSegTag.INDEX, key: '', index: Number.parseInt(inner, 10) || 0 }));
      }
      index = end + 1;
      continue;
    }
    return [];
  }
  return segments;
}

async function callPathSpanResult(
  text: string,
  path: PathSeg[],
  documentKey: string,
  snapshotId: SnapshotId | null,
  _languageId: SupportedEditorLanguageId,
  target: QueryTargetKind,
  _nest: boolean,
): Promise<PathSpanReadResult> {
  if (snapshotId == null) return snapshotNotReady();
  const result = await callSharedWasmWorker<SnapshotReadResult<QueryResult>>('querySnapshot', {
    documentKey,
    snapshotId,
    queryKind: 'findAnchors',
    pathPattern: serializePath(path),
    target,
  });
  if (result.status !== 'ready') return snapshotNotReady();
  const anchor = result.data.anchors[0];
  if (!anchor || anchor.spanEnd < anchor.spanStart) return { status: 'ready', data: null };
  const start = byteOffsetToRowColumn(text, anchor.spanStart);
  return {
    status: 'ready',
    data: {
      startByte: anchor.spanStart,
      endByte: anchor.spanEnd,
      row: start.row,
      column: start.column,
    },
  };
}

export async function resolveTreePathFromTextResult(
  text: string,
  row: number,
  column: number,
  documentKey: string,
  languageId: SupportedEditorLanguageId,
  nest: boolean,
  columnMode: TreePathColumnMode = 'char',
  snapshotId: SnapshotId | null,
): Promise<TreePathReadResult> {
  const resolvedColumn = resolveTreePathColumn(text, row, column, columnMode);
  return callTreePathResult(text, row, resolvedColumn, documentKey, snapshotId, languageId, nest);
}

export async function resolveTreePathResult(
  model: Monaco.editor.ITextModel,
  position: Monaco.IPosition,
  documentKey: string,
  languageId: SupportedEditorLanguageId,
  nest: boolean,
  snapshotId: SnapshotId | null,
): Promise<TreePathReadResult> {
  const row = Math.max(0, position.lineNumber - 1);
  const columnIndex = Math.max(0, position.column - 1);
  return resolveTreePathFromTextResult(model.getValue(), row, columnIndex, documentKey, languageId, nest, 'char', snapshotId);
}

// Snapshot-bound source lookup. Graph payloads provide paths only; byte spans
// come from the active snapshot so formatted close output and editor text stay aligned.
export async function resolvePathSpanResult(
  model: Monaco.editor.ITextModel,
  path: PathSeg[],
  documentKey: string,
  languageId: SupportedEditorLanguageId,
  target: 'key' | 'value',
  nest: boolean,
  snapshotId: SnapshotId | null,
): Promise<PathSpanReadResult> {
  if (!path || path.length === 0) return { status: 'ready', data: null };
  const result = await callPathSpanResult(
    model.getValue(),
    path,
    documentKey,
    snapshotId,
    languageId,
    target === 'key' ? 'key' : 'value',
    nest,
  );
  if (result.status !== 'ready') return result;
  const span = result.data;
  if (!span || span.row < 0 || span.column < 0 || span.endByte < span.startByte) return { status: 'ready', data: null };
  return { status: 'ready', data: span };
}

function getPathSpanCandidates(target: 'key' | 'value' | 'node'): Array<'key' | 'value'> {
  return target === 'key' ? ['key', 'value'] : ['value', 'key'];
}

// Internal graph/text linkage anchor — row/column stay 0-based byte coordinates here.
// Use resolvePathSelectionRangeResult when a Monaco 1-based editor range is required.
function toPathAnchor(span: PathSpan | null): { row: number; column: number } | null {
  if (!span) return null;
  return { row: span.row, column: span.column };
}


function byteOffsetToPosition(model: Monaco.editor.ITextModel, byteOffset: number): Monaco.IPosition {
  return model.getPositionAt(byteOffsetToUtf16Offset(model.getValue(), byteOffset));
}

function toPathSelectionRange(model: Monaco.editor.ITextModel, span: PathSpan | null): PathSelectionRange | null {
  if (!span) return null;
  return {
    start: byteOffsetToPosition(model, span.startByte),
    end: byteOffsetToPosition(model, span.endByte),
  };
}

async function resolveFirstPathSpanCandidate<T>(
  model: Monaco.editor.ITextModel,
  path: PathSeg[],
  documentKey: string,
  languageId: SupportedEditorLanguageId,
  target: 'key' | 'value' | 'node',
  nest: boolean,
  mapSpan: (span: PathSpan | null) => T | null,
  snapshotId: SnapshotId | null,
): Promise<SnapshotReadResult<T | null>> {
  if (!path || path.length === 0) return { status: 'ready', data: null };
  for (const candidate of getPathSpanCandidates(target)) {
    const spanResult = await resolvePathSpanResult(model, path, documentKey, languageId, candidate, nest, snapshotId);
    if (spanResult.status !== 'ready') return spanResult;
    const result = mapSpan(spanResult.data);
    if (result) return { status: 'ready', data: result };
  }
  return { status: 'ready', data: null };
}

export async function resolvePathAnchorResult(
  model: Monaco.editor.ITextModel,
  path: PathSeg[],
  documentKey: string,
  languageId: SupportedEditorLanguageId,
  target: 'key' | 'value' | 'node',
  nest: boolean,
  snapshotId: SnapshotId | null,
): Promise<SnapshotReadResult<{ row: number; column: number } | null>> {
  return resolveFirstPathSpanCandidate(model, path, documentKey, languageId, target, nest, toPathAnchor, snapshotId);
}

export async function resolvePathSelectionRangeResult(
  model: Monaco.editor.ITextModel,
  path: PathSeg[],
  documentKey: string,
  languageId: SupportedEditorLanguageId,
  target: 'key' | 'value' | 'node',
  nest: boolean,
  snapshotId: SnapshotId | null,
): Promise<SnapshotReadResult<PathSelectionRange | null>> {
  return resolveFirstPathSpanCandidate(
    model,
    path,
    documentKey,
    languageId,
    target,
    nest,
    (span) => toPathSelectionRange(model, span),
    snapshotId,
  );
}
