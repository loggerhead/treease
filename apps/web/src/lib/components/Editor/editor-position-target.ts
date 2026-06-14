import type * as Monaco from 'monaco-editor';
import type { GraphHighlightTarget } from '../../store/editor-store';
import type { PathSeg } from '../../store/tree-path';
import type { SupportedEditorLanguageId } from '../../monaco/language-support';
import { resolvePathSpan, toByteColumn } from '../../services/TreePathService';
import { getActiveDocumentSnapshotId } from '../../services/DocumentSessionService';

export async function resolveEditorPositionTarget(
  model: Monaco.editor.ITextModel,
  position: Monaco.IPosition,
  path: PathSeg[],
  documentKey: string,
  languageId: SupportedEditorLanguageId,
  nest: boolean,
): Promise<GraphHighlightTarget | undefined> {
  if (!path.length) return undefined;
  const last = path[path.length - 1];
  if (last && typeof last.index === 'number' && last.tag === 1) {
    return 'value';
  }
  const lineCount = model.getLineCount();
  if (position.lineNumber < 1 || position.lineNumber > lineCount) return undefined;
  const row = Math.max(0, position.lineNumber - 1);
  const columnIndex = Math.max(0, position.column - 1);
  const lineText = model.getLineContent(position.lineNumber);
  const column = toByteColumn(lineText, columnIndex);
  const snapshotId = getActiveDocumentSnapshotId(documentKey);
  const [keySpan, valueSpan] = await Promise.all([
    resolvePathSpan(model, path, documentKey, languageId, 'key', nest, snapshotId),
    resolvePathSpan(model, path, documentKey, languageId, 'value', nest, snapshotId),
  ]);
  const keyPosition = keySpan ? { row: keySpan.row, column: keySpan.column } : null;
  const valuePosition = valueSpan ? { row: valueSpan.row, column: valueSpan.column } : null;
  if (
    keyPosition &&
    valuePosition &&
    keyPosition.row === valuePosition.row &&
    keyPosition.column === valuePosition.column
  ) {
    return 'node';
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
  if (!candidates.length) return undefined;
  candidates.sort((left, right) => left.distance - right.distance);
  return candidates[0]?.target;
}
