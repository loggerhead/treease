// 职责：Graph hover panel 类型定义：preview kind/target、prewarm candidate、graph cache entry、debug snapshot
import type { PathSeg } from '../../store/tree-path';
import type { GraphCell, GraphEdge, GraphNode } from '../../graph/graph-viewer-render';

export type GraphHoverPreviewKind = 'pre' | 'subgraph';

export type GraphHoverPreviewTarget = {
  cell: GraphCell;
  target: 'key' | 'value' | 'node';
  previewKind: GraphHoverPreviewKind;
};

export type TooltipPanelPrewarmCandidate = {
  cell: GraphCell;
  target: 'value';
};

export type TooltipPanelGraphData = {
  pathKey: string;
  path: PathSeg[];
  nodes: GraphNode[];
  edges: GraphEdge[];
  minX: number;
  minY: number;
  width: number;
  height: number;
};

export type TooltipPanelGraphCacheEntry = {
  signature: string;
  accessOrder: number;
  graph?: TooltipPanelGraphData | null;
  promise?: Promise<TooltipPanelGraphData | null>;
};

export type TooltipPanelPrewarmDebugSnapshot = {
  scheduledPaths: PathSeg[][];
  completedPaths: PathSeg[][];
  inFlightPath: PathSeg[] | null;
};
