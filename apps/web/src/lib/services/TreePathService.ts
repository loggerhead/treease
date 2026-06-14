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
type PathSpan = {
  startByte: number;
  endByte: number;
  row: number;
  column: number;
};


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

async function callTreePath(
  text: string,
  row: number,
  column: number,
  documentKey: string,
  snapshotId: SnapshotId | null,
  languageId: SupportedEditorLanguageId,
  nest: boolean,
): Promise<PathSeg[]> {
  void languageId;
  void nest;
  if (snapshotId == null) return [];
  const byteOffset = byteOffsetFromRowColumn(text, row, column);
  const result = await callSharedWasmWorker<SnapshotReadResult<QueryResult>>('querySnapshot', {
    documentKey,
    snapshotId,
    queryKind: 'resolvePath',
    spanStart: byteOffset,
    spanEnd: byteOffset,
  });
  if (result.status !== 'ready') return [];
  return parseAnchorPath(result.data.anchors[0]?.path);
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

async function callPathSpan(
  text: string,
  path: PathSeg[],
  documentKey: string,
  snapshotId: SnapshotId | null,
  languageId: SupportedEditorLanguageId,
  target: QueryTargetKind,
  nest: boolean,
): Promise<PathSpan | null> {
  void languageId;
  void nest;
  if (snapshotId == null) return null;
  const result = await callSharedWasmWorker<SnapshotReadResult<QueryResult>>('querySnapshot', {
    documentKey,
    snapshotId,
    queryKind: 'findAnchors',
    pathPattern: serializePath(path),
    target,
  });
  if (result.status !== 'ready') return null;
  const anchor = result.data.anchors[0];
  if (!anchor || anchor.spanEnd < anchor.spanStart) return null;
  const start = byteOffsetToRowColumn(text, anchor.spanStart);
  return {
    startByte: anchor.spanStart,
    endByte: anchor.spanEnd,
    row: start.row,
    column: start.column,
  };
}

export async function resolveTreePathFromText(
  text: string,
  row: number,
  column: number,
  documentKey: string,
  languageId: SupportedEditorLanguageId,
  nest: boolean,
  columnMode: TreePathColumnMode = 'char',
  snapshotId: SnapshotId | null,
): Promise<PathSeg[]> {
  const resolvedColumn = resolveTreePathColumn(text, row, column, columnMode);
  return callTreePath(text, row, resolvedColumn, documentKey, snapshotId, languageId, nest);
}

/**
 * 解析指定位置的树路径。
 * @param model 文本模型
 * @param position 位置信息
 * @param documentKey 缓存键
 * @param languageId 语言 ID
 * @param nest 是否启用嵌套解析
 * @returns 路径段数组
 */
export async function resolveTreePath(
  model: Monaco.editor.ITextModel,
  position: Monaco.IPosition,
  documentKey: string,
  languageId: SupportedEditorLanguageId,
  nest: boolean,
  snapshotId: SnapshotId | null,
): Promise<PathSeg[]> {
  const row = Math.max(0, position.lineNumber - 1);
  const columnIndex = Math.max(0, position.column - 1);
  return resolveTreePathFromText(model.getValue(), row, columnIndex, documentKey, languageId, nest, 'char', snapshotId);
}

/**
 * 获取指定位置的树路径。
 * @param model 文本模型
 * @param position 位置信息
 * @param documentKey 缓存键
 * @param languageId 语言 ID
 * @param nest 是否启用嵌套解析
 * @returns 路径段数组
 */
export async function getTreePathAtPosition(
  model: Monaco.editor.ITextModel,
  position: Monaco.IPosition,
  documentKey: string,
  languageId: SupportedEditorLanguageId,
  nest: boolean,
  snapshotId: SnapshotId | null,
): Promise<PathSeg[]> {
  if (!position) return [];
  try {
    return await resolveTreePath(model, position, documentKey, languageId, nest, snapshotId);
  } catch (error) {
    console.error('[TreePathService] resolveTreePath failed', { documentKey, languageId, row: position.lineNumber, column: position.column }, error);
    return [];
  }
}

export async function resolveTreePathSafe(
  model: Monaco.editor.ITextModel,
  position: Monaco.IPosition | null,
  documentKey: string,
  languageId: SupportedEditorLanguageId,
  nest: boolean,
  snapshotId: SnapshotId | null,
): Promise<PathSeg[]> {
  if (!position) return [];
  return getTreePathAtPosition(model, position, documentKey, languageId, nest, snapshotId);
}

// Snapshot-bound source lookup. Graph payloads provide paths only; byte spans
// come from the active snapshot so formatted close output and editor text stay aligned.
export async function resolvePathSpan(
  model: Monaco.editor.ITextModel,
  path: PathSeg[],
  documentKey: string,
  languageId: SupportedEditorLanguageId,
  target: 'key' | 'value',
  nest: boolean,
  snapshotId: SnapshotId | null,
): Promise<PathSpan | null> {
  if (!path || path.length === 0) return null;
  try {
    const span = await callPathSpan(
      model.getValue(),
      path,
      documentKey,
      snapshotId,
      languageId,
      target === 'key' ? 'key' : 'value',
      nest,
    );
    if (!span || span.row < 0 || span.column < 0 || span.endByte < span.startByte) return null;
    return span;
  } catch (error) {
    console.debug('[TreePathService] resolvePathSpan failed', { documentKey, languageId, path, target, nest }, error);
    return null;
  }
}

function getPathSpanCandidates(target: 'key' | 'value' | 'node'): Array<'key' | 'value'> {
  return target === 'key' ? ['key', 'value'] : ['value', 'key'];
}

// Internal graph/text linkage anchor — row/column stay 0-based byte coordinates here.
// Use resolvePathSelectionRangeSafe when a Monaco 1-based editor range is required.
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
): Promise<T | null> {
  if (!path || path.length === 0) return null;
  for (const candidate of getPathSpanCandidates(target)) {
    const span = await resolvePathSpan(model, path, documentKey, languageId, candidate, nest, snapshotId);
    const result = mapSpan(span);
    if (result) return result;
  }
  return null;
}

export async function resolvePathAnchorSafe(
  model: Monaco.editor.ITextModel,
  path: PathSeg[],
  documentKey: string,
  languageId: SupportedEditorLanguageId,
  target: 'key' | 'value' | 'node',
  nest: boolean,
  snapshotId: SnapshotId | null,
): Promise<{ row: number; column: number } | null> {
  return resolveFirstPathSpanCandidate(model, path, documentKey, languageId, target, nest, toPathAnchor, snapshotId);
}

export async function resolvePathSelectionRangeSafe(
  model: Monaco.editor.ITextModel,
  path: PathSeg[],
  documentKey: string,
  languageId: SupportedEditorLanguageId,
  target: 'key' | 'value' | 'node',
  nest: boolean,
  snapshotId: SnapshotId | null,
): Promise<PathSelectionRange | null> {
  return resolveFirstPathSpanCandidate(model, path, documentKey, languageId, target, nest, (span) =>
    toPathSelectionRange(model, span),
    snapshotId,
  );
}
