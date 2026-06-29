// 职责：子图工作区 graph 数据类型定义
import type { GraphEdge, GraphNode } from '../../graph/graph-viewer-render';
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
