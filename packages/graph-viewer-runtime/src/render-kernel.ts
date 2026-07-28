// Responsibility: GraphViewer scene-rendering kernel for node/edge drawing, edge filtering, and render-config application.
import type { GraphViewerRenderConfig } from "./config";
import {
  createCellText,
  drawSimpleNode,
  drawTableNode,
  type DrawContext,
  type GraphEdge,
  type GraphNode,
} from "./render";

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
  remove?: () => void;
  destroy?: () => void;
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
  renderConfig: GraphViewerRenderConfig;
  edgeRenderByKey?: Map<string, PenLike>;
};

function graphEdgeKey(edge: GraphEdge): string {
  const curve = edge.bezierArgs;
  return [
    edge.fromRenderHandle,
    edge.fromRow,
    edge.toRenderHandle,
    edge.toRow,
    curve.fromX,
    curve.fromY,
    curve.c1x,
    curve.c1y,
    curve.c2x,
    curve.c2y,
    curve.toX,
    curve.toY,
  ].join(":");
}

type RenderGraphNodesInput = {
  nodes: GraphNode[];
  drawContext: DrawContext;
  nodeBoxMap: Map<number, any>;
  registerMetaClickTarget?: (target: any, cell: GraphNode["meta"], kind: "meta") => void;
};

type RenderGraphNodeInput = {
  node: GraphNode;
  drawContext: DrawContext;
  registerMetaClickTarget?: (target: any, cell: GraphNode["meta"], kind: "meta") => void;
  showMeta?: boolean;
};

export type RenderGraphNodeResult = {
  metaText: any | null;
  nodeBox: any | null;
  tableRuntime?: any;
};

export function renderGraphEdges(input: RenderGraphEdgesInput): GraphEdge[] {
  if (!input.layer || !input.PenCtor) return [];
  const { layer, PenCtor } = input;
  if (!input.edgeRenderByKey) layer.removeAll(true);
  const renderedEdges: GraphEdge[] = [];
  const desiredKeys = new Set<string>();
  input.edges.forEach((edge) => {
    const key = graphEdgeKey(edge);
    desiredKeys.add(key);
    renderedEdges.push(edge);
    if (input.edgeRenderByKey?.has(key)) return;
    const curve = edge.bezierArgs;
    const pen = new PenCtor();
    pen.setStyle({ stroke: input.renderConfig.colors.edge, strokeWidth: 1 });
    pen.moveTo(curve.fromX, curve.fromY);
    pen.bezierCurveTo(curve.c1x, curve.c1y, curve.c2x, curve.c2y, curve.toX, curve.toY);
    layer.add(pen);
    input.edgeRenderByKey?.set(key, pen);
  });
  if (input.edgeRenderByKey) {
    for (const [key, pen] of input.edgeRenderByKey) {
      if (desiredKeys.has(key)) continue;
      pen.remove?.();
      pen.destroy?.();
      input.edgeRenderByKey.delete(key);
    }
  }
  return renderedEdges;
}

export function renderGraphNode(input: RenderGraphNodeInput): RenderGraphNodeResult {
  const metaText =
    input.showMeta === false
      ? null
      : createCellText(
          input.drawContext,
          input.drawContext.nodeLayer,
          input.node.meta,
          "meta",
          input.node.kind,
        );
  if (metaText) {
    input.registerMetaClickTarget?.(metaText, input.node.meta, "meta");
  }
  if (input.node.kind === "table" && input.node.table) {
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
