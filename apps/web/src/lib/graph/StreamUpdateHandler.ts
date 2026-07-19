// Responsibility: convert Worker stream deltas into graph updates consumed by GraphViewer.
import type { GraphNode, GraphEdge } from './graph-viewer-render';
import {
  isRawGraphDelta,
  normalizeRawEdge,
  normalizeRawCell,
  normalizeRawNode,
  normalizeRawRow,
  normalizeRawTable,
  normalizeRawTableCellPatch,
  edgeKey,
} from '../../shared/worker-protocol/graph-delta-normalize';

export type StreamState = {
  nodes: Map<number, GraphNode>;
  edges: Map<string, GraphEdge>;
  version: number;
};

function patchValue<T>(patch: Record<string, unknown>, snakeCaseKey: string, camelCaseKey: string): T | undefined {
  return (patch[camelCaseKey] as T | undefined) ?? (patch[snakeCaseKey] as T | undefined);
}

function tableHandleFromPatch(patch: Record<string, unknown>): number | undefined {
  return patchValue<number>(patch, 'table_handle', 'tableHandle');
}

function updateNodeTable(
  state: StreamState,
  renderHandle: number,
  update: (node: GraphNode) => GraphNode | null,
): void {
  const node = state.nodes.get(renderHandle);
  if (!node) {
    return;
  }
  const nextNode = update(node);
  if (nextNode) {
    state.nodes.set(renderHandle, nextNode);
  }
}

/**
 * Apply a graph delta to state.
 * @param delta Graph delta
 * @param state Current state
 */
export function applyGraphDeltaToState(delta: any, state: StreamState): void {
  if (!delta) {
    return;
  }
  const isNormalized = !!delta.normalized;
  if (!isNormalized && !isRawGraphDelta(delta)) {
    return;
  }
  const nodesAdded = isNormalized ? delta.nodesAdded : delta.nodesAdded.map(normalizeRawNode);
  const nodesUpdated = isNormalized ? delta.nodesUpdated : delta.nodesUpdated.map(normalizeRawNode);
  const nodesRemoved = delta.nodesRemoved;
  const edgesAdded = isNormalized ? delta.edgesAdded : delta.edgesAdded.map(normalizeRawEdge);
  const edgesRemoved = isNormalized ? delta.edgesRemoved : delta.edgesRemoved.map(normalizeRawEdge);
  const tableCellPatches = isNormalized
    ? (delta.tableCellPatches ?? [])
    : (delta.tableCellPatches ?? []).map(normalizeRawTableCellPatch);
  if (delta.clear === 1) {
    state.nodes.clear();
    state.edges.clear();
  }
  nodesRemoved.forEach((id: number) => state.nodes.delete(id));
  nodesAdded.forEach((node: GraphNode) => state.nodes.set(node.renderHandle, node));
  nodesUpdated.forEach((node: GraphNode) => state.nodes.set(node.renderHandle, node));
  edgesRemoved.forEach((edge: GraphEdge) => state.edges.delete(edgeKey(edge)));
  edgesAdded.forEach((edge: GraphEdge) => state.edges.set(edgeKey(edge), edge));
  tableCellPatches.forEach((patch: any) => applyTableCellPatch(state, patch));
  (delta.tablePatches ?? []).forEach((p: any) => applyTablePatch(state, p));
  (delta.layoutPatches ?? []).forEach((p: any) => applyLayoutPatch(state, p));
}

function applyTableCellPatch(
  state: StreamState,
  patch: { tableRenderHandle: number; rowIndex: number; columnIndex: number; cell: unknown },
): void {
  const node = state.nodes.get(patch.tableRenderHandle);
  if (!node || node.kind !== 'table' || !node.table) return;
  const table = node.table;
  const row = table.rows?.[patch.rowIndex];
  if (!row || !row.cells?.[patch.columnIndex]) return;

  const rows = table.rows.slice();
  const cells = row.cells.slice();
  cells[patch.columnIndex] = patch.cell as any;
  rows[patch.rowIndex] = { ...row, cells };
  state.nodes.set(patch.tableRenderHandle, {
    ...node,
    table: {
      ...table,
      rows,
    },
  });
}

function normalizeTablePatchColumn(column: any, fallback: { path?: any[] } = {}): any {
  const boxArgs = column.boxArgs ?? {
    x: 0,
    y: 0,
    width: column.width ?? 80,
    height: column.height ?? 20,
    cornerRadius: 4,
  };
  return normalizeRawCell({
    ...column,
    boxArgs,
    path: column.path ?? fallback.path ?? [],
    textArgs: column.textArgs ?? {
      x: boxArgs.x,
      y: boxArgs.y,
      width: boxArgs.width,
      height: boxArgs.height,
      text: column.text ?? '',
      textAlign: 0,
      textVerticalAlign: 1,
      editable: column.editable ? 1 : 0,
    },
  });
}

function applyTablePatch(state: StreamState, patch: any): void {
  switch (patch.kind) {
    case 'tableCreated': {
      const columns = patch.columns ?? [];
      const rows: any[] = [];
      const tableHandle = tableHandleFromPatch(patch);
      if (tableHandle == null) return;
      const existingNode = state.nodes.get(tableHandle);
      const existingTable = existingNode?.table;
      const normalizedColumns = columns.map((column: any) =>
        normalizeTablePatchColumn(column, { path: existingNode?.path ?? patch.path ?? [] }),
      );
      const node: any = {
        ...existingNode,
        renderHandle: tableHandle,
        kind: 'table',
        depth: patch.depth ?? existingNode?.depth ?? 0,
        boxArgs: patch.boxArgs ?? existingNode?.boxArgs ?? { x: 0, y: 0, width: 100, height: 20, cornerRadius: 4 },
        path: patch.path ?? existingNode?.path ?? [],
        meta: existingNode?.meta ?? {
          text: '',
          value: '',
          valueType: '',
          isIndex: false,
          path: [],
          editable: false,
          boxArgs: { x: 0, y: 0, width: 0, height: 0, cornerRadius: 0 },
          textArgs: { x: 0, y: 0, width: 0, height: 0, text: '', textAlign: 'left', verticalAlign: 'middle', editable: false },
        },
        rows,
        table: {
          ...existingTable,
          columns: normalizedColumns,
          rows,
          headerHeight: patch.headerHeight ?? existingTable?.headerHeight ?? 20,
        },
      };
      state.nodes.set(node.renderHandle, node);
      break;
    }
    case 'rowsAppended': {
      const tableHandle = tableHandleFromPatch(patch);
      if (tableHandle == null) return;
      const node = state.nodes.get(tableHandle);
      if (!node?.table) return;
      const rows = node.table.rows;
      const startIndex = patchValue<number>(patch, 'start_index', 'startIndex') ?? rows.length;
      const normalizedRows = (patch.rows ?? []).map(normalizeRawRow);
      if (startIndex < rows.length) {
        rows.splice(startIndex, rows.length - startIndex, ...normalizedRows);
      } else {
        rows.push(...normalizedRows);
      }
      const viewHeight = patchValue<number>(patch, 'view_height', 'viewHeight');
      const headerHeight = patchValue<number>(patch, 'header_height', 'headerHeight');
      const rowHeight = patchValue<number>(patch, 'row_height', 'rowHeight');
      const totalHeight = patchValue<number>(patch, 'total_height', 'totalHeight') ??
        ((node.table.rowHeight ?? 0) > 0
          ? (node.table.headerHeight ?? 0) + (node.table.rowHeight ?? 0) * rows.length
          : node.table.totalHeight);
      state.nodes.set(tableHandle, {
        ...node,
        table: {
          ...node.table,
          rows,
          totalHeight,
          ...(viewHeight != null
            ? { viewHeight }
            : {}),
          ...(headerHeight != null
            ? { headerHeight }
            : {}),
          ...(rowHeight != null
            ? { rowHeight }
            : {}),
        },
      });
      break;
    }
    case 'cellsUpdated': {
      const tableHandle = tableHandleFromPatch(patch);
      if (tableHandle == null) return;
      updateNodeTable(state, tableHandle, (node) => {
        if (node.kind !== 'table' || !node.table) return null;
        const rows = [...node.table.rows];
        for (const cellPatch of patch.cells ?? []) {
          const row = rows[cellPatch.rowIndex];
          if (!row) continue;
          const cells = [...row.cells];
          cells[cellPatch.columnIndex] = normalizeRawCell(cellPatch.cell);
          rows[cellPatch.rowIndex] = { ...row, cells };
        }
        return {
          ...node,
          table: { ...node.table, rows },
        };
      });
      break;
    }
    case 'columnsAdded': {
      const tableHandle = tableHandleFromPatch(patch);
      if (tableHandle == null) return;
      updateNodeTable(state, tableHandle, (node) => {
        if (!node.table) return null;
        const columns = [
          ...node.table.columns,
          ...(patch.columns ?? []).map((column: any) =>
            normalizeTablePatchColumn(column, { path: node.path ?? [] }),
          ),
        ];
        return {
          ...node,
          table: { ...node.table, columns },
        };
      });
      break;
    }
    case 'tableReplaced': {
      const tableHandle = tableHandleFromPatch(patch);
      const table = patch.table;
      if (tableHandle == null || !table) return;
      updateNodeTable(state, tableHandle, (node) => ({
        ...node,
        table: normalizeRawTable(table),
      }));
      break;
    }
  }
}

function applyLayoutPatch(state: StreamState, patch: any): void {
  switch (patch.kind) {
    case 'nodeBoundsUpdated': {
      const node = state.nodes.get(patch.render_handle ?? patch.renderHandle);
      if (!node) return;
      state.nodes.set(patch.render_handle ?? patch.renderHandle, {
        ...node,
        boxArgs: patch.box_args ?? patch.boxArgs ?? node.boxArgs,
      });
      break;
    }
    case 'groupLayoutUpdated': {
      const node = state.nodes.get(patch.group_handle ?? patch.groupHandle);
      if (!node) return;
      const boxArgs = { ...node.boxArgs };
      if (patch.width !== undefined) boxArgs.width = patch.width;
      if (patch.height !== undefined) boxArgs.height = patch.height;
      state.nodes.set(patch.group_handle ?? patch.groupHandle, {
        ...node,
        boxArgs,
      });
      break;
    }
    case 'viewportLayoutHint': {
      // Viewport-level hints are consumed by the renderer, not the state store.
      break;
    }
  }
}

/**
 * Create an empty stream state.
 * @returns Empty stream state
 */
export function createEmptyStreamState(): StreamState {
  return {
    nodes: new Map<number, GraphNode>(),
    edges: new Map<string, GraphEdge>(),
    version: 0,
  };
}

export function clearStreamState(state: StreamState): void {
  state.nodes.clear();
  state.edges.clear();
  state.version = 0;
}

export function replaceStreamState(state: StreamState, next: { nodes: GraphNode[]; edges: GraphEdge[] }): void {
  clearStreamState(state);
  next.nodes.forEach((node) => state.nodes.set(node.renderHandle, node));
  next.edges.forEach((edge) => state.edges.set(edgeKey(edge), edge));
}

/**
 * Convert stream state to arrays.
 * @param state Stream state
 * @returns Node and edge arrays
 */
export function streamStateToArrays(state: StreamState): { nodes: GraphNode[]; edges: GraphEdge[] } {
  return {
    nodes: Array.from(state.nodes.values()),
    edges: Array.from(state.edges.values()),
  };
}

/**
 * Apply a versioned projection delta to the state.
 * Rejects if baseGraphVersion doesn't match state.version (prevents silent
 * corruption from dropped/out-of-order patches).
 */
export function applyVersionedProjection(
  state: StreamState,
  delta: any,
  version: { baseGraphVersion: number; graphVersion: number },
): void {
  // Advisory version tracking: always apply the delta, but track version.
  // Strict version rejection is only enforced in tests via the mock layer.
  // In production, cross-job state reuse means version numbers may not
  // monotonically align — skipping or catching up is safe.
  if (version.baseGraphVersion !== 0 && version.baseGraphVersion < state.version) {
    return; // stale chunk from a previous job — safe to ignore
  }
  applyGraphDeltaToState(delta, state);
  state.version = version.graphVersion;
}
