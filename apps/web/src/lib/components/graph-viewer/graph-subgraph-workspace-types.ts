// Responsibility: define subgraph-workspace graph data types.
import type { GraphEdge, GraphNode } from '@treease/graph-viewer-runtime';
import type { PathSeg } from '../../store/tree-path';

export type SubgraphWorkspaceGraphData = {
  pathKey: string;
  path: PathSeg[];
  nodes: GraphNode[];
  edges: GraphEdge[];
  minX: number;
  minY: number;
  width: number;
  height: number;
};
