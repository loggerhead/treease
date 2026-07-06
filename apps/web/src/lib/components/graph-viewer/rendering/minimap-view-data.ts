import type { MinimapViewData } from '../../../leafer-minimap';

import type { GraphSceneViewData } from './index';

export function toMinimapViewData(graphData: GraphSceneViewData | null): MinimapViewData | null {
  if (!graphData) return null;

  const spread = 1.1;
  const nodes = graphData.nodes;
  const centerY = nodes.length > 0 ? nodes.reduce((sum, node) => sum + node.boxArgs.y, 0) / nodes.length : 0;

  const yDeltaByHandle = new Map<number, number>();
  const spreadNodes = nodes.map((node) => {
    const handle = node.renderHandle;
    const originalY = node.boxArgs.y;
    const spreadY = centerY + (originalY - centerY) * spread;
    yDeltaByHandle.set(handle, spreadY - originalY);
    return {
      id: handle,
      kind: node.kind,
      x: node.boxArgs.x,
      y: spreadY,
      width: node.boxArgs.width,
      height: node.boxArgs.height,
    };
  });

  return {
    nodes: spreadNodes,
    edges: graphData.edges.map((edge) => {
      const fromDelta = yDeltaByHandle.get(edge.fromRenderHandle) ?? 0;
      const toDelta = yDeltaByHandle.get(edge.toRenderHandle) ?? 0;
      return {
        fromX: edge.bezierArgs.fromX,
        fromY: edge.bezierArgs.fromY + fromDelta,
        c1x: edge.bezierArgs.c1x,
        c1y: edge.bezierArgs.c1y + fromDelta,
        c2x: edge.bezierArgs.c2x,
        c2y: edge.bezierArgs.c2y + toDelta,
        toX: edge.bezierArgs.toX,
        toY: edge.bezierArgs.toY + toDelta,
      };
    }),
  };
}
