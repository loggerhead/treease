import type { GraphEdge, GraphNode } from '@treease/graph-viewer-runtime';

type ProjectionState = { nodes: Map<number, GraphNode>; edges: Map<string, GraphEdge> };
const text = (value: unknown): string => typeof value === 'string' ? value : String(value ?? '');
const path = (value: unknown): any[] => Array.isArray(value) ? value : [];
const cell = (value: any) => {
  const boxArgs = value?.boxArgs ?? { x: 0, y: 0, width: 0, height: 0, cornerRadius: 0 };
  const semType = typeof value?.semType === 'number' ? value.semType : 2;
  const valueType = semType === 0 ? 'object' : semType === 1 ? 'array' : semType === 3 || semType === 4 ? 'number' : semType === 5 ? 'boolean' : semType === 6 ? 'null' : 'string';
  const display = text(value?.text ?? value?.value);
  return { text: display, value: text(value?.value ?? display), semType, valueType, path: path(value?.path), editable: value?.editable === true || value?.editable === 1, boxArgs, textArgs: { x: value?.textArgs?.x ?? boxArgs.x, y: value?.textArgs?.y ?? boxArgs.y, width: value?.textArgs?.width ?? boxArgs.width, height: value?.textArgs?.height ?? boxArgs.height, text: text(value?.textArgs?.text ?? display), textAlign: value?.textArgs?.textAlign === 2 ? 'right' : value?.textArgs?.textAlign === 1 ? 'center' : 'left', verticalAlign: value?.textArgs?.textVerticalAlign === 0 ? 'top' : value?.textArgs?.textVerticalAlign === 2 ? 'bottom' : 'middle', editable: value?.textArgs?.editable === true || value?.textArgs?.editable === 1 } } as const;
};
const row = (value: any) => ({ boxArgs: value?.boxArgs ?? { x: 0, y: 0, width: 0, height: 0, cornerRadius: 0 }, cellBoxArgs: value?.cellBoxArgs ?? { x: 0, y: 0, width: 0, height: 0, cornerRadius: 0 }, cells: Array.isArray(value?.cells) ? value.cells.map(cell) : [] });
const node = (value: any): GraphNode => {
  const kind = value?.kind === 2 ? 'table' : value?.kind === 1 ? 'object' : value?.kind === 'table' || value?.kind === 'object' ? value.kind : 'scalar';
  const nodePath = path(value?.path);
  return { renderHandle: Number(value?.renderHandle), kind, depth: Number(value?.depth ?? 0), boxArgs: value?.boxArgs ?? { x: 0, y: 0, width: 0, height: 0, cornerRadius: 0 }, path: nodePath, meta: cell(value?.meta), rows: Array.isArray(value?.rows) ? value.rows.map(row) : [], table: value?.table ? { key: text(value.table.key), columns: Array.isArray(value.table.columns) ? value.table.columns.map(cell) : [], rows: Array.isArray(value.table.rows) ? value.table.rows.map(row) : [], headerHeight: Number(value.table.headerHeight ?? value.table.header_height ?? 0), totalHeight: Number(value.table.totalHeight ?? value.table.total_height ?? 0), viewHeight: Number(value.table.viewHeight ?? value.table.view_height ?? 0), rowHeight: Number(value.table.rowHeight ?? value.table.row_height ?? 0) } : undefined };
};
const edge = (value: any): GraphEdge => ({ fromRenderHandle: Number(value?.fromRenderHandle), fromRow: Number(value?.fromRow ?? 0), toRenderHandle: Number(value?.toRenderHandle), toRow: Number(value?.toRow ?? 0), bezierArgs: value?.bezierArgs ?? { fromX: 0, fromY: 0, c1x: 0, c1y: 0, c2x: 0, c2y: 0, toX: 0, toY: 0 } });
const edgeKey = (value: GraphEdge) => `${value.fromRenderHandle}:${value.fromRow}:${value.toRenderHandle}:${value.toRow}`;

export function createProjectionState(): ProjectionState { return { nodes: new Map(), edges: new Map() }; }
export function projectionToRawGraphDelta(projection: any): any | null { const data = projection?.graphData ?? null; return data ? { clear: projection?.clear ? 1 : 0, nodesAdded: data.nodesAdded ?? [], nodesUpdated: data.nodesUpdated ?? [], nodesRemoved: data.nodesRemoved ?? [], edgesAdded: data.edgesAdded ?? [], edgesRemoved: data.edgesRemoved ?? [] } : projection?.clear ? { clear: 1, nodesAdded: [], nodesUpdated: [], nodesRemoved: [], edgesAdded: [], edgesRemoved: [] } : null; }
export function applyProjectionDelta(state: ProjectionState, delta: any): void { if (!delta) return; if (delta.clear === 1) { state.nodes.clear(); state.edges.clear(); } for (const id of delta.nodesRemoved ?? []) state.nodes.delete(Number(id)); for (const raw of [...(delta.nodesAdded ?? []), ...(delta.nodesUpdated ?? [])]) { const value = node(raw); state.nodes.set(value.renderHandle, value); } for (const raw of delta.edgesRemoved ?? []) state.edges.delete(edgeKey(edge(raw))); for (const raw of delta.edgesAdded ?? []) { const value = edge(raw); state.edges.set(edgeKey(value), value); } }
export function projectionSnapshot(state: ProjectionState): { nodes: GraphNode[]; edges: GraphEdge[] } { return { nodes: [...state.nodes.values()], edges: [...state.edges.values()] }; }
