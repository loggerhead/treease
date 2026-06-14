// 职责：共享的 RawGraphDelta 规范化纯函数：节点/边/表格补丁的 normalize、edgeKey、canonicalPathKey
import { GraphKind, PathSegTag, SemType, type PathSeg } from '@core-wasm/index'
import { toWasmPathSeg } from '../brand-bridge';
import type { RawGraphDelta } from './protocol';
import { readWasmString } from '../tree-node-value';

export function semTypeToValueType(semType: SemType): 'string' | 'number' | 'boolean' | 'null' | 'object' | 'array' {
  switch (semType) {
    case SemType.MAP:
      return 'object';
    case SemType.SEQ:
      return 'array';
    case SemType.STR:
      return 'string';
    case SemType.INT:
    case SemType.FLOAT:
      return 'number';
    case SemType.BOOLEAN:
      return 'boolean';
    case SemType.NIL:
      return 'null';
    default:
      return 'string';
  }
}

export function graphKindToString(kind: GraphKind) {
  switch (kind) {
    case GraphKind.SCALAR:
      return 'scalar';
    case GraphKind.TABLE:
      return 'table';
    case GraphKind.OBJECT:
      return 'object';
    default:
      throw new Error(`Unknown graph kind: ${kind}`);
  }
}

export type NormalizedGraphNodeKey = {
  kind: string;
  path: PathSeg[];
  pathKey: string;
  stableId: string;
};

function textAlignToString(value: number): 'center' | 'right' | 'left' {
  switch (value) {
    case 1:
      return 'center';
    case 2:
      return 'right';
    default:
      return 'left';
  }
}

function verticalAlignToString(value: number): 'top' | 'middle' | 'bottom' {
  switch (value) {
    case 0:
      return 'top';
    case 2:
      return 'bottom';
    default:
      return 'middle';
  }
}

function normalizeEditableFlag(value: unknown): boolean {
  return value === true || value === 1;
}

export function isRawGraphDelta(value: unknown): value is RawGraphDelta {
  if (!value || typeof value !== 'object') return false;
  const delta = value as RawGraphDelta;
  const nodesRemoved = delta.nodesRemoved as unknown;
  const tableCellPatches = delta.tableCellPatches as unknown;
  return (
    Array.isArray(delta.nodesAdded) &&
    Array.isArray(delta.nodesUpdated) &&
    (Array.isArray(nodesRemoved) || ArrayBuffer.isView(nodesRemoved)) &&
    Array.isArray(delta.edgesAdded) &&
    Array.isArray(delta.edgesRemoved) &&
    (tableCellPatches == null || Array.isArray(tableCellPatches))
  );
}

function normalizeDeltaPath(path: any): PathSeg[] {
  if (!Array.isArray(path)) return [];
  return path.map((seg) => {
    if (seg?.tag === PathSegTag.KEY) {
      return toWasmPathSeg({
        tag: PathSegTag.KEY,
        key: readWasmString(seg.key),
        index: seg.index ?? 0,
      });
    }
    return toWasmPathSeg({
      tag: PathSegTag.INDEX,
      key: '',
      index: seg?.index ?? 0,
    });
  });
}

export function buildCanonicalPathKey(path: PathSeg[]): string {
  return path
    .map((seg) => (seg.tag === PathSegTag.KEY ? `k:${readWasmString(seg.key)}` : `i:${seg.index ?? 0}`))
    .join('|');
}

function normalizeRawNodeKey(key: any, fallbackKind: GraphKind, fallbackPath: PathSeg[]): NormalizedGraphNodeKey {
  const path = normalizeDeltaPath(key?.path);
  const normalizedPath = path.length > 0 || fallbackPath.length === 0 ? path : fallbackPath;
  return {
    kind: graphKindToString(key?.kind ?? fallbackKind),
    path: normalizedPath,
    pathKey: readWasmString(key?.pathKey) || buildCanonicalPathKey(normalizedPath),
    stableId: readWasmString(key?.stableId),
  };
}
function emptyCellValue() {
  return {
    text: '',
    value: '',
    valueType: 'string' as const,
    isIndex: false,
    path: [],
    editable: false,
    boxArgs: { x: 0, y: 0, width: 0, height: 0, cornerRadius: 0 },
    textArgs: { x: 0, y: 0, width: 0, height: 0, text: '', textAlign: 'left' as const, verticalAlign: 'middle' as const, editable: false },
  };
}

export function normalizeRawCell(cell: any) {
  if (!cell || typeof cell !== 'object' || Array.isArray(cell)) {
    return emptyCellValue();
  }
  const boxArgs = cell.boxArgs ?? { x: 0, y: 0, width: 0, height: 0, cornerRadius: 0 };
  const valueType = semTypeToValueType(cell.semType);
  const displayText = cell.text ?? cell.value ?? '';
  const rawValue = cell.value ?? '';
  const value =
    rawValue === '' && (valueType === 'object' || valueType === 'array') ? displayText : rawValue;
  const textArgs = cell.textArgs ?? {
    x: boxArgs.x,
    y: boxArgs.y,
    width: boxArgs.width,
    height: boxArgs.height,
    text: displayText,
    textAlign: 0,
    textVerticalAlign: 1,
    editable: 0,
  };
  const displayTextArgsText = textArgs.text === '' || textArgs.text == null ? displayText : textArgs.text;
  const textAlign = textAlignToString(textArgs.textAlign);
  const verticalAlign = verticalAlignToString(textArgs.textVerticalAlign);
  const editable = normalizeEditableFlag(textArgs.editable ?? cell.editable);
  return {
    text: displayText,
    value,
    valueType,
    isIndex: cell.isIndex === true,
    path: normalizeDeltaPath(cell.path),
    editable,
    boxArgs,
    textArgs: {
      x: textArgs.x,
      y: textArgs.y,
      width: textArgs.width,
      height: textArgs.height,
      text: displayTextArgsText,
      textAlign,
      verticalAlign,
      editable,
    },
  };
}

export function normalizeRawTableCellPatch(patch: any) {
  return {
    tableRenderHandle: patch.tableRenderHandle,
    rowIndex: patch.rowIndex,
    columnIndex: patch.columnIndex,
    cell: normalizeRawCell(patch.cell),
  };
}

export function normalizeRawRow(row: any) {
  const boxArgs = row.boxArgs ?? { x: 0, y: 0, width: 0, height: 0, cornerRadius: 0 };
  const cellBoxArgs = row.cellBoxArgs ?? { x: 0, y: 0, width: 0, height: 0, cornerRadius: 0 };
  return {
    boxArgs,
    cellBoxArgs,
    cells: Array.isArray(row.cells) ? row.cells.map(normalizeRawCell) : [],
  };
}

export function normalizeRawTable(table: any) {
  return {
    key: table.key ? readWasmString(table.key) : '',
    columns: Array.isArray(table.columns) ? table.columns.map(normalizeRawCell) : [],
    rows: Array.isArray(table.rows) ? table.rows.map(normalizeRawRow) : [],
    headerHeight: table.headerHeight ?? 0,
    totalHeight: table.totalHeight ?? 0,
    viewHeight: table.viewHeight ?? 0,
    rowHeight: table.rowHeight ?? 0,
  };
}

export function normalizeRawNode(node: any) {
  const boxArgs = node.boxArgs ?? { x: 0, y: 0, width: 0, height: 0, cornerRadius: 0 };
  const path = normalizeDeltaPath(node.path);
  const nodeKind = node.kind ?? node.key?.kind ?? 0;
  const keyObj = node.key ?? { kind: nodeKind, path };
  const key = normalizeRawNodeKey(keyObj, nodeKind, path);
  return {
    renderHandle: node.renderHandle,
    key,
    kind: graphKindToString(nodeKind),
    depth: node.depth,
    boxArgs,
    path,
    meta: normalizeRawCell(node.meta),
    rows: Array.isArray(node.rows) ? node.rows.map(normalizeRawRow) : [],
    table: node.table ? normalizeRawTable(node.table) : undefined,
  };
}

export function normalizeRawEdge(edge: any) {
  // New format: fromKind/fromPath directly on edge. Old format: from.key.path.
  const fromPath = normalizeDeltaPath(edge.from?.path ?? edge.fromPath);
  const toPath = normalizeDeltaPath(edge.to?.path ?? edge.toPath);
  const fromInfo = edge.from ?? { kind: edge.fromKind ?? 1, path: fromPath };
  const toInfo = edge.to ?? { kind: edge.toKind ?? 1, path: toPath };
  const rawBezierArgs = edge.bezierArgs;
  const bezierArgs = rawBezierArgs && typeof rawBezierArgs === 'object' && !Array.isArray(rawBezierArgs)
    ? {
        fromX: rawBezierArgs.fromX ?? rawBezierArgs.from_x ?? 0,
        fromY: rawBezierArgs.fromY ?? rawBezierArgs.from_y ?? 0,
        c1x: rawBezierArgs.c1x ?? 0,
        c1y: rawBezierArgs.c1y ?? 0,
        c2x: rawBezierArgs.c2x ?? 0,
        c2y: rawBezierArgs.c2y ?? 0,
        toX: rawBezierArgs.toX ?? rawBezierArgs.to_x ?? 0,
        toY: rawBezierArgs.toY ?? rawBezierArgs.to_y ?? 0,
      }
    : {
        fromX: edge.bezierFromX ?? edge.bezier_from_x ?? 0,
        fromY: edge.bezierFromY ?? edge.bezier_from_y ?? 0,
        c1x: edge.bezierC1x ?? edge.bezier_c1x ?? 0,
        c1y: edge.bezierC1y ?? edge.bezier_c1y ?? 0,
        c2x: edge.bezierC2x ?? edge.bezier_c2x ?? 0,
        c2y: edge.bezierC2y ?? edge.bezier_c2y ?? 0,
        toX: edge.bezierToX ?? edge.bezier_to_x ?? 0,
        toY: edge.bezierToY ?? edge.bezier_to_y ?? 0,
      };
  return {
    fromRenderHandle: edge.fromRenderHandle,
    from: normalizeRawNodeKey(fromInfo, GraphKind.SCALAR, fromPath),
    fromRow: edge.fromRow,
    toRenderHandle: edge.toRenderHandle,
    to: normalizeRawNodeKey(toInfo, GraphKind.SCALAR, toPath),
    toRow: edge.toRow,
    bezierArgs,
  };
}

export function edgeKey(edge: {
  from?: NormalizedGraphNodeKey;
  fromRenderHandle: number;
  fromRow: number;
  to?: NormalizedGraphNodeKey;
  toRenderHandle: number;
  toRow: number;
}) {
  const from = edge.from?.stableId || edge.from?.pathKey || String(edge.fromRenderHandle);
  const to = edge.to?.stableId || edge.to?.pathKey || String(edge.toRenderHandle);
  return `${from}:${edge.fromRow}:${to}:${edge.toRow}`;
}
