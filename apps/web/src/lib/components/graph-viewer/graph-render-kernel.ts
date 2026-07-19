// Responsibility: GraphViewer scene-rendering kernel for node/edge drawing, edge filtering, and render-config application.
import type { GraphViewerConfig } from '../../settings/ui-settings';
import { filterDenseOffscreenEdges, type TableVisibleRange } from './graph-edge-filter';
import {
  createCellText,
  drawSimpleNode,
  drawTableNode,
  type DrawContext,
  type GraphEdge,
  type GraphNode,
} from '../../graph/graph-viewer-render';

type PenLike = {
  setStyle: (style: { stroke: string; strokeWidth: number }) => void;
  moveTo: (x: number, y: number) => void;
  bezierCurveTo: (
    c1x: number,
    c1y: number,
    c2x: number,
    c2y: number,
    toX: number,
    toY: number,
  ) => void;
};

type PenCtorLike = new () => PenLike;

type LayerLike = {
  removeAll: (destroyChildren?: boolean) => void;
  add: (target: unknown) => void;
};

type RenderGraphEdgesInput = {
  nodes: GraphNode[];
  edges: GraphEdge[];
  layer: LayerLike | null;
  PenCtor: PenCtorLike | null;
  renderConfig: GraphViewerConfig;
  container?: HTMLElement | null;
  leafer?: Parameters<typeof filterDenseOffscreenEdges>[3];
  maxPerSource?: number | null;
  tableVisibleRanges?: ReadonlyMap<number, TableVisibleRange>;
};

type RenderGraphNodesInput = {
  nodes: GraphNode[];
  drawContext: DrawContext;
  nodeBoxMap: Map<number, any>;
  registerMetaClickTarget?: (target: any, cell: GraphNode['meta'], kind: 'meta') => void;
};

type RenderGraphNodeInput = {
  node: GraphNode;
  drawContext: DrawContext;
  registerMetaClickTarget?: (target: any, cell: GraphNode['meta'], kind: 'meta') => void;
  showMeta?: boolean;
};

export type RenderGraphNodeResult = {
  metaText: any | null;
  nodeBox: any | null;
  tableRuntime?: any;
};

export function renderGraphEdges(input: RenderGraphEdgesInput): GraphEdge[] {
  if (!input.layer || !input.PenCtor) return [];
  input.layer.removeAll(true);
  const nodeMap = new Map(input.nodes.map((node) => [node.renderHandle, node]));
  const edgesToRender =
    input.maxPerSource == null
      ? input.edges
      : filterDenseOffscreenEdges(
          input.nodes,
          input.edges,
          input.container ?? null,
          input.leafer ?? null,
          input.maxPerSource,
          input.tableVisibleRanges,
        );
  edgesToRender.forEach((edge) => {
    const from = nodeMap.get(edge.fromRenderHandle);
    const to = nodeMap.get(edge.toRenderHandle);
    if (!from || !to) return;
    const curve = edge.bezierArgs;
    const pen = new input.PenCtor();
    pen.setStyle({ stroke: input.renderConfig.colors.edge, strokeWidth: 1 });
    pen.moveTo(curve.fromX, curve.fromY);
    pen.bezierCurveTo(curve.c1x, curve.c1y, curve.c2x, curve.c2y, curve.toX, curve.toY);
    input.layer.add(pen);
  });
  return edgesToRender;
}

export function renderGraphNode(input: RenderGraphNodeInput): RenderGraphNodeResult {
  const metaText =
    input.showMeta === false
      ? null
      : createCellText(input.drawContext, input.drawContext.nodeLayer, input.node.meta, 'meta', input.node.kind);
  if (metaText) {
    input.registerMetaClickTarget?.(metaText, input.node.meta, 'meta');
  }
  if (input.node.kind === 'table' && input.node.table) {
    const result = drawTableNode(input.drawContext, input.node);
    return {
      metaText,
      nodeBox: result?.nodeBox ?? null,
      tableRuntime: result?.tableRuntime,
    };
  }
  const result = drawSimpleNode(input.drawContext, input.node);
  return {
    metaText,
    nodeBox: result.nodeBox,
  };
}

export function renderGraphNodes(input: RenderGraphNodesInput): void {
  input.nodes.forEach((node) => {
    const result = renderGraphNode({
      node,
      drawContext: input.drawContext,
      registerMetaClickTarget: input.registerMetaClickTarget,
    });
    if (result.nodeBox) input.nodeBoxMap.set(node.renderHandle, result.nodeBox);
  });
}
