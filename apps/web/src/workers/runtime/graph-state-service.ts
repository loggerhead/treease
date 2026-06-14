import type { GraphEdge, GraphNode } from './protocol';

export type GraphState = {
  nodes: Map<number, GraphNode>;
  edges: Map<string, GraphEdge>;
};

export type GraphStateService = {
  graphStateByDocumentKey: Map<string, GraphState>;
  getGraphState: (documentKey: string) => GraphState | undefined;
  ensureGraphState: (documentKey: string) => GraphState;
  clearGraphState: (documentKey: string) => void;
  clearAllGraphStates: () => void;
};

function createGraphState(): GraphState {
  return {
    nodes: new Map<number, GraphNode>(),
    edges: new Map<string, GraphEdge>(),
  };
}

export function createGraphStateService(
  graphStateByDocumentKey: Map<string, GraphState> = new Map<string, GraphState>(),
): GraphStateService {
  function getGraphState(documentKey: string): GraphState | undefined {
    return graphStateByDocumentKey.get(documentKey);
  }

  function ensureGraphState(documentKey: string): GraphState {
    const existing = graphStateByDocumentKey.get(documentKey);
    if (existing) return existing;
    const created = createGraphState();
    graphStateByDocumentKey.set(documentKey, created);
    return created;
  }

  function clearGraphState(documentKey: string): void {
    const state = graphStateByDocumentKey.get(documentKey);
    if (!state) return;
    state.nodes.clear();
    state.edges.clear();
    graphStateByDocumentKey.delete(documentKey);
  }

  function clearAllGraphStates(): void {
    for (const state of graphStateByDocumentKey.values()) {
      state.nodes.clear();
      state.edges.clear();
    }
    graphStateByDocumentKey.clear();
  }

  return {
    graphStateByDocumentKey,
    getGraphState,
    ensureGraphState,
    clearGraphState,
    clearAllGraphStates,
  };
}
