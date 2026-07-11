import type * as Monaco from 'monaco-editor';
import type { SnapshotReadResult } from '@core-wasm/index';
import type { GraphHighlightTarget } from '../../store/graph-selection-store';
import type { PathSeg } from '../../store/tree-path';
import type { SupportedEditorLanguageId } from '../../monaco/language-support';
import { resolvePathSpanResult, toByteColumn } from '../../services/TreePathService';
import { getWorkspaceSnapshotId } from '../../store/workspace-store';

export async function resolveEditorPositionTargetResult(
  model: Monaco.editor.ITextModel,
  position: Monaco.IPosition,
  path: PathSeg[],
  documentKey: string,
  languageId: SupportedEditorLanguageId,
  nest: boolean,
): Promise<SnapshotReadResult<GraphHighlightTarget | undefined>> {
  if (!path.length) return { status: 'ready', data: undefined };
  const last = path[path.length - 1];
  if (last && typeof last.index === 'number' && last.tag === 1) {
    return { status: 'ready', data: 'value' };
  }
  const lineCount = model.getLineCount();
  if (position.lineNumber < 1 || position.lineNumber > lineCount) return { status: 'ready', data: undefined };
  const row = Math.max(0, position.lineNumber - 1);
  const columnIndex = Math.max(0, position.column - 1);
  const lineText = model.getLineContent(position.lineNumber);
  const column = toByteColumn(lineText, columnIndex);
  const snapshotId = getWorkspaceSnapshotId(documentKey);
  const [keySpanResult, valueSpanResult] = await Promise.all([
    resolvePathSpanResult(model, path, documentKey, languageId, 'key', nest, snapshotId),
    resolvePathSpanResult(model, path, documentKey, languageId, 'value', nest, snapshotId),
  ]);
  if (keySpanResult.status !== 'ready' || valueSpanResult.status !== 'ready') {
    return { status: 'snapshotNotReady' };
  }
  const keySpan = keySpanResult.data;
  const valueSpan = valueSpanResult.data;
  const keyPosition = keySpan ? { row: keySpan.row, column: keySpan.column } : null;
  const valuePosition = valueSpan ? { row: valueSpan.row, column: valueSpan.column } : null;
  if (
    keyPosition &&
    valuePosition &&
    keyPosition.row === valuePosition.row &&
    keyPosition.column === valuePosition.column
  ) {
    return { status: 'ready', data: 'node' };
  }
  const candidates: Array<{ target: GraphHighlightTarget; distance: number }> = [];
  if (keyPosition) {
    candidates.push({
      target: 'key',
      distance: Math.abs(keyPosition.row - row) * 10_000 + Math.abs(keyPosition.column - column),
    });
  }
  if (valuePosition) {
    candidates.push({
      target: 'value',
      distance: Math.abs(valuePosition.row - row) * 10_000 + Math.abs(valuePosition.column - column),
    });
  }
  if (!candidates.length) return { status: 'ready', data: undefined };
  candidates.sort((left, right) => left.distance - right.distance);
  return { status: 'ready', data: candidates[0]?.target };
}

export async function resolveEditorPositionTarget(
  model: Monaco.editor.ITextModel,
  position: Monaco.IPosition,
  path: PathSeg[],
  documentKey: string,
  languageId: SupportedEditorLanguageId,
  nest: boolean,
): Promise<GraphHighlightTarget | undefined> {
  const result = await resolveEditorPositionTargetResult(model, position, path, documentKey, languageId, nest);
  return result.status === 'ready' ? result.data : undefined;
}
