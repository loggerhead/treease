// Responsibility: turn Core projection deltas into a UI-neutral graph snapshot.
// This module is intentionally dependency-free from Svelte/Leafer so every Treease
// surface (Web and the Chrome extension) consumes identical Core graph facts.
import type { DocumentProjectionDelta } from '@core-wasm/index';
import { normalizeRawEdge, normalizeRawNode } from './worker-protocol/graph-delta-normalize';

export type ProjectionGraphState = {
  nodes: Map<number, any>;
  edges: Map<string, any>;
};

function edgeKey(edge: any): string {
  return `${edge.from?.stableId ?? edge.fromRenderHandle}:${edge.fromRow}:${edge.to?.stableId ?? edge.toRenderHandle}:${edge.toRow}`;
}

export function createProjectionGraphState(): ProjectionGraphState {
  return { nodes: new Map(), edges: new Map() };
}

export function projectionToRawGraphDelta(projection: DocumentProjectionDelta | null): any | null {
  const graphData = projection?.graphData ?? null;
  if (!graphData) return projection?.clear ? { clear: 1, nodesAdded: [], nodesUpdated: [], nodesRemoved: [], edgesAdded: [], edgesRemoved: [] } : null;
  return {
    clear: projection?.clear ? 1 : 0,
    nodesAdded: graphData.nodesAdded ?? [], nodesUpdated: graphData.nodesUpdated ?? [], nodesRemoved: graphData.nodesRemoved ?? [],
    edgesAdded: graphData.edgesAdded ?? [], edgesRemoved: graphData.edgesRemoved ?? [],
  };
}

export function applyProjectionGraphDelta(state: ProjectionGraphState, delta: any): void {
  if (!delta) return;
  if (delta.clear === 1) { state.nodes.clear(); state.edges.clear(); }
  for (const id of delta.nodesRemoved ?? []) state.nodes.delete(Number(id));
  for (const raw of [...(delta.nodesAdded ?? []), ...(delta.nodesUpdated ?? [])]) {
    const node = normalizeRawNode(raw);
    state.nodes.set(node.renderHandle, node);
  }
  for (const raw of delta.edgesRemoved ?? []) {
    const edge = normalizeRawEdge(raw);
    state.edges.delete(edgeKey(edge));
  }
  for (const raw of delta.edgesAdded ?? []) {
    const edge = normalizeRawEdge(raw);
    state.edges.set(edgeKey(edge), edge);
  }
}

export function projectionGraphSnapshot(state: ProjectionGraphState): { nodes: any[]; edges: any[] } {
  return { nodes: [...state.nodes.values()], edges: [...state.edges.values()] };
}
