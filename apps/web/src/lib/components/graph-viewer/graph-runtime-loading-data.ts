export type GraphRuntimeLoadingNode = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export type GraphRuntimeLoadingEdge = {
  fromX: number;
  fromY: number;
  c1x: number;
  c1y: number;
  c2x: number;
  c2y: number;
  toX: number;
  toY: number;
};

export type GraphRuntimeLoadingBounds = {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
};

export type GraphRuntimeLoadingNodeBarRole = 'key' | 'value' | 'single';

export type GraphRuntimeLoadingNodeBar = {
  x: number;
  y: number;
  width: number;
  height: number;
  role: GraphRuntimeLoadingNodeBarRole;
};

export const GRAPH_RUNTIME_LOADING_NODES: GraphRuntimeLoadingNode[] = [
  { x: 0, y: 0, width: 248, height: 74 },
  { x: 308, y: 0, width: 154, height: 110 },
  { x: 308, y: 170, width: 110, height: 56 },
  { x: 308, y: 286, width: 242, height: 84 },
  { x: 308, y: 430, width: 592, height: 128 },
  { x: 960, y: 430, width: 556, height: 38 },
];

export const GRAPH_RUNTIME_LOADING_EDGES: GraphRuntimeLoadingEdge[] = [
  { fromX: 248, fromY: 10, c1x: 288, c1y: 10, c2x: 268, c2y: 10, toX: 308, toY: 10 },
  { fromX: 248, fromY: 28, c1x: 288, c1y: 28, c2x: 268, c2y: 180, toX: 308, toY: 180 },
  { fromX: 248, fromY: 46, c1x: 288, c1y: 46, c2x: 268, c2y: 300, toX: 308, toY: 300 },
  { fromX: 900, fromY: 548, c1x: 940, c1y: 548, c2x: 920, c2y: 440, toX: 960, toY: 440 },
  { fromX: 248, fromY: 64, c1x: 288, c1y: 64, c2x: 268, c2y: 440, toX: 308, toY: 440 },
];

export const GRAPH_RUNTIME_LOADING_NODE_BARS: GraphRuntimeLoadingNodeBar[][] = [
  [
    { x: 22, y: 24, width: 48, height: 8, role: 'key' },
    { x: 100, y: 24, width: 108, height: 8, role: 'value' },
    { x: 22, y: 42, width: 33, height: 8, role: 'key' },
    { x: 100, y: 42, width: 91, height: 8, role: 'value' },
  ],
  [
    { x: 324, y: 38, width: 29, height: 10, role: 'key' },
    { x: 373, y: 38, width: 63, height: 10, role: 'value' },
    { x: 324, y: 62, width: 20, height: 10, role: 'key' },
    { x: 373, y: 62, width: 53, height: 10, role: 'value' },
  ],
  [
    { x: 345, y: 185, width: 36, height: 9, role: 'single' },
    { x: 339, y: 202, width: 48, height: 9, role: 'single' },
  ],
  [
    { x: 330, y: 313, width: 46, height: 9, role: 'key' },
    { x: 406, y: 313, width: 105, height: 9, role: 'value' },
    { x: 330, y: 334, width: 32, height: 9, role: 'key' },
    { x: 406, y: 334, width: 88, height: 9, role: 'value' },
  ],
  [
    { x: 340, y: 449, width: 121, height: 12, role: 'key' },
    { x: 515, y: 449, width: 318, height: 12, role: 'value' },
    { x: 340, y: 475, width: 86, height: 12, role: 'key' },
    { x: 515, y: 475, width: 268, height: 12, role: 'value' },
    { x: 340, y: 501, width: 130, height: 12, role: 'key' },
    { x: 515, y: 501, width: 240, height: 12, role: 'value' },
    { x: 340, y: 527, width: 95, height: 12, role: 'key' },
    { x: 515, y: 527, width: 297, height: 12, role: 'value' },
  ],
  [{ x: 1091, y: 445, width: 295, height: 8, role: 'single' }],
];

export const GRAPH_RUNTIME_LOADING_VIEWBOX_PADDING = 24;

function updateBounds(bounds: GraphRuntimeLoadingBounds | null, x: number, y: number): GraphRuntimeLoadingBounds {
  if (!bounds) {
    return { minX: x, minY: y, maxX: x, maxY: y };
  }
  return {
    minX: Math.min(bounds.minX, x),
    minY: Math.min(bounds.minY, y),
    maxX: Math.max(bounds.maxX, x),
    maxY: Math.max(bounds.maxY, y),
  };
}

export function getGraphRuntimeLoadingBounds(
  nodes: GraphRuntimeLoadingNode[] = GRAPH_RUNTIME_LOADING_NODES,
  edges: GraphRuntimeLoadingEdge[] = GRAPH_RUNTIME_LOADING_EDGES,
): GraphRuntimeLoadingBounds {
  let bounds: GraphRuntimeLoadingBounds | null = null;

  for (const node of nodes) {
    bounds = updateBounds(bounds, node.x, node.y);
    bounds = updateBounds(bounds, node.x + node.width, node.y + node.height);
  }

  for (const edge of edges) {
    bounds = updateBounds(bounds, edge.fromX, edge.fromY);
    bounds = updateBounds(bounds, edge.c1x, edge.c1y);
    bounds = updateBounds(bounds, edge.c2x, edge.c2y);
    bounds = updateBounds(bounds, edge.toX, edge.toY);
  }

  return bounds ?? { minX: 0, minY: 0, maxX: 0, maxY: 0 };
}

export function getGraphRuntimeLoadingViewBox(
  nodes: GraphRuntimeLoadingNode[] = GRAPH_RUNTIME_LOADING_NODES,
  edges: GraphRuntimeLoadingEdge[] = GRAPH_RUNTIME_LOADING_EDGES,
  padding = GRAPH_RUNTIME_LOADING_VIEWBOX_PADDING,
): string {
  const bounds = getGraphRuntimeLoadingBounds(nodes, edges);
  const width = Math.max(1, bounds.maxX - bounds.minX + padding * 2);
  const height = Math.max(1, bounds.maxY - bounds.minY + padding * 2);
  return `${bounds.minX - padding} ${bounds.minY - padding} ${width} ${height}`;
}
