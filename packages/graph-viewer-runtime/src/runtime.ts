import { defaultGraphViewerRenderConfig, type GraphViewerRenderConfig } from './config';
import { renderGraphEdges, renderGraphNode } from './render-kernel';
import { createGraphPointerController } from './pointer-controller';
import type { GraphCell, GraphCellKind, GraphEdge, GraphNode } from './types';
import { loadGraphViewerRuntime, type LoadedGraphRuntime } from './runtime-loader';
import { createGraphViewportController } from './viewport-controller';

export type GraphData = { nodes: GraphNode[]; edges: GraphEdge[] };
export type GraphActivation = { path: GraphCell['path']; nodeKind?: GraphNode['kind']; target: 'cell' | 'node' };
export type GraphHighlightTarget = { renderHandle: number } | null;
export type RawGraphDelta = unknown;
export type GraphViewerInteraction = {
  onActivate?: (event: GraphActivation) => void;
  onRuntimeReady?: (runtime: GraphRuntimeHandle) => void;
  onError?: (error: unknown) => void;
};
export type GraphRuntimeHandle = { app: unknown; modules: unknown };
export type GraphViewerRuntimeOptions = {
  host: HTMLElement;
  config?: GraphViewerRenderConfig;
  interaction?: GraphViewerInteraction;
  /** Hosts own delta normalization and freshness; runtime only applies the resulting graph. */
  reduceDelta?: (current: GraphData, delta: RawGraphDelta) => GraphData;
};

type LeaferModules = {
  App: new (config: Record<string, unknown>) => any;
  Leafer: new (config: Record<string, unknown>) => any;
  Box: new (config: Record<string, unknown>) => any;
  Text: new (config: Record<string, unknown>) => any;
  Pen: new () => any;
  PointerEvent?: { TAP?: string; CLICK?: string; DOWN?: string; MOVE?: string; UP?: string };
  MoveEvent?: { BEFORE_MOVE?: string; MOVE?: string };
};

/** Framework-neutral Leafer graph surface; hosts supply only interaction policy. */
export class GraphViewerRuntime {
  private app: any | null = null;
  private edgeLayer: any | null = null;
  private nodeLayer: any | null = null;
  private modules: LeaferModules | null = null;
  private tableRuntimes: Array<{ destroy?: () => void }> = [];
  private highlightedTargets: any[] = [];
  private nodeBoxes = new Map<number, any>();
  private workspaceProbeTarget: any | null = null;
  private graph: GraphData | null = null;
  private lastTargetActivationAt = 0;
  private loadedRuntime: LoadedGraphRuntime | null = null;
  private viewportController: ReturnType<typeof createGraphViewportController> | null = null;
  private disposed = false;

  constructor(private readonly options: GraphViewerRuntimeOptions) {
    this.host = options.host;
    this.config = options.config ?? defaultGraphViewerRenderConfig;
    this.host.addEventListener('click', this.handleCanvasClick, true);
    this.host.addEventListener('pointerdown', this.handleCanvasClick, true);
  }

  private readonly host: HTMLElement;
  private readonly config: GraphViewerRenderConfig;

  async replaceGraph(graph: GraphData): Promise<void> {
    await this.ensureRuntime();
    if (this.disposed || !this.modules || !this.app || !this.nodeLayer || !this.edgeLayer) return;

    this.tableRuntimes.forEach((runtime) => runtime.destroy?.());
    this.tableRuntimes = [];
    this.highlightedTargets = [];
    this.nodeBoxes.clear();
    this.workspaceProbeTarget = null;
    this.graph = graph;
    this.nodeLayer.removeAll(true);
    this.edgeLayer.removeAll(true);
    const { Box, Text, Pen } = this.modules;
    const pointerController = createGraphPointerController({
      getPointerEventCtor: () => this.modules?.PointerEvent,
      getMoveEventCtor: () => this.modules?.MoveEvent,
      getActiveApp: () => this.app,
    });
    const drawContext = {
      nodeLayer: this.nodeLayer,
      styleConfig: this.config,
      languageIdValue: 'json',
      fontSize: this.config.layout.baseFontSize,
      BoxCtor: Box,
      TextCtor: Text,
      PenCtor: Pen,
      editable: false,
      registerCellBox: () => {},
      unregisterCellBox: () => {},
      registerRowBox: () => {},
      unregisterRowBox: () => {},
      registerClickTarget: (target: any, cell: GraphCell, kind: GraphCellKind, nodeKind?: GraphData['nodes'][number]['kind']) => {
        if (!this.workspaceProbeTarget && (nodeKind === 'object' || nodeKind === 'table')) this.workspaceProbeTarget = target;
        pointerController.bindPointerClick(target, () => {
          this.lastTargetActivationAt = Date.now();
          this.options.interaction?.onActivate?.({ path: cell.path, nodeKind, target: 'cell' });
        });
        return '';
      },
      bindVerticalScrollGesture: pointerController.bindVerticalScrollGesture,
      bindPointerDown: pointerController.bindPointerDown,
      getPointFromEvent: pointerController.getPointFromEvent,
      requestRender: () => this.requestRender(),
    };
    renderGraphEdges({
      nodes: graph.nodes,
      edges: graph.edges,
      layer: this.edgeLayer,
      PenCtor: Pen,
      renderConfig: this.config,
    });
    for (const node of graph.nodes) {
      const result = renderGraphNode({
        node,
        drawContext,
        registerMetaClickTarget: (target: any, cell: GraphCell) => {
          pointerController.bindPointerClick(target, () => {
            this.lastTargetActivationAt = Date.now();
            this.options.interaction?.onActivate?.({ path: cell.path, nodeKind: node.kind, target: 'cell' });
          });
        },
      });
      if (result.nodeBox) this.nodeBoxes.set(node.renderHandle, result.nodeBox);
      if (result.tableRuntime) this.tableRuntimes.push(result.tableRuntime);
    }
    this.fitToGraph(graph);
    this.requestRender();
    this.publishWorkspaceProbe();
  }

  async applyDelta(delta: RawGraphDelta): Promise<void> {
    if (!this.graph) {
      const error = new Error('Cannot apply a graph delta before replaceGraph().');
      this.options.interaction?.onError?.(error);
      throw error;
    }
    if (!this.options.reduceDelta) {
      const error = new Error('GraphViewerRuntime requires a host delta reducer for applyDelta().');
      this.options.interaction?.onError?.(error);
      throw error;
    }
    try {
      await this.replaceGraph(this.options.reduceDelta(this.graph, delta));
    } catch (error) {
      this.options.interaction?.onError?.(error);
      throw error;
    }
  }

  destroy(): void {
    this.disposed = true;
    this.tableRuntimes.forEach((runtime) => runtime.destroy?.());
    this.tableRuntimes = [];
    this.host.removeEventListener('click', this.handleCanvasClick, true);
    this.host.removeEventListener('pointerdown', this.handleCanvasClick, true);
    this.app?.view?.removeEventListener?.('click', this.handleCanvasClick, true);
    this.app?.view?.removeEventListener?.('pointerdown', this.handleCanvasClick, true);
    this.loadedRuntime?.destroy();
    this.loadedRuntime = null;
    this.app = null;
    this.viewportController = null;
    this.edgeLayer = null;
    this.nodeLayer = null;
  }

  private async ensureRuntime(): Promise<void> {
    if (this.app || this.disposed) return;
    const loaded = await loadGraphViewerRuntime({ host: this.host, preferApp: false });
    if (this.disposed) return;
    this.loadedRuntime = loaded;
    this.modules = loaded.modules as LeaferModules;
    this.app = loaded.app;
    this.viewportController = createGraphViewportController({
      getContainer: () => this.host as HTMLDivElement,
      getLeafer: () => this.app,
      getSuppressGraphPointerUntil: () => 0,
      getMoveEventName: () => this.modules?.MoveEvent?.BEFORE_MOVE ?? this.modules?.MoveEvent?.MOVE,
      getZoomEventName: () => undefined,
      bindPointerClick: () => {},
      updateViewportOverlays: () => {},
      getPanConstraintBounds: () => this.graph ? graphBounds(this.graph) : null,
    });
    const root = this.app.zoomLayer ?? this.app;
    this.edgeLayer = new this.modules!.Box({ fill: 'transparent', hittable: false, hitChildren: false });
    this.nodeLayer = new this.modules!.Box({ fill: 'transparent' });
    root.add(this.edgeLayer);
    root.add(this.nodeLayer);
    this.app.view?.addEventListener?.('click', this.handleCanvasClick, true);
    this.app.view?.addEventListener?.('pointerdown', this.handleCanvasClick, true);
    this.options.interaction?.onRuntimeReady?.({ app: this.app, modules: this.modules });
  }

  fitToGraph(graph = this.graph): void {
    const zoomLayer = this.app?.zoomLayer;
    if (!zoomLayer || !graph || graph.nodes.length === 0) return;
    const left = Math.min(...graph.nodes.map((node) => node.boxArgs.x));
    const top = Math.min(...graph.nodes.map((node) => node.boxArgs.y));
    zoomLayer.x = this.config.layout.canvasPadding - left;
    zoomLayer.y = this.config.layout.canvasPadding - top;
  }

  private requestRender(): void {
    this.app?.update?.();
    this.app?.forceRender?.();
  }

  zoomIn(): void { this.zoomBy(1.2); }

  zoomOut(): void { this.zoomBy(1 / 1.2); }

  centerOnNode(renderHandle: number): boolean {
    const node = this.graph?.nodes.find((candidate) => candidate.renderHandle === renderHandle);
    if (!node || !this.viewportController) return false;
    this.viewportController.centerOnNode(node);
    return true;
  }

  highlight(target: GraphHighlightTarget | any): void {
    for (const previous of this.highlightedTargets) {
      previous.selected = false;
      previous.selectedStyle = undefined;
    }
    const renderTarget = typeof target?.renderHandle === 'number' ? this.nodeBoxes.get(target.renderHandle) : target;
    if (!renderTarget) return;
    this.highlightedTargets = [renderTarget];
    renderTarget.selectedStyle = { stroke: '#84a300', strokeWidth: 2, fill: '#f8ffe1' };
    renderTarget.selected = true;
    this.requestRender();
  }

  private zoomBy(factor: number): void {
    this.viewportController?.applyZoom(factor);
  }

  private handleCanvasClick = (event: Event): void => {
    // Leafer's target event handles the precise cell first. The DOM fallback is
    // retained for canvas clicks that land on a node background rather than text.
    window.setTimeout(() => {
      if (Date.now() - this.lastTargetActivationAt < 32) return;
      if (!this.graph) return;
      const pointer = event as MouseEvent;
      const world = this.app?.getWorldPointByClient?.({ x: pointer.clientX, y: pointer.clientY });
      const node = this.graph.nodes.find((candidate) => world &&
        Number(world.x) >= candidate.boxArgs.x && Number(world.x) <= candidate.boxArgs.x + candidate.boxArgs.width &&
        Number(world.y) >= candidate.boxArgs.y && Number(world.y) <= candidate.boxArgs.y + candidate.boxArgs.height,
      );
      if (!node) return;
      const nodeBox = this.nodeBoxes.get(node.renderHandle);
      if (nodeBox) this.highlight(nodeBox);
      this.options.interaction?.onActivate?.({ path: node.path, nodeKind: node.kind, target: 'node' });
    }, 0);
  };

  private publishWorkspaceProbe(): void {
    const target = this.workspaceProbeTarget as { getWorldPointByBox?: (point: { x: number; y: number }) => { x?: number; y?: number } | null; width?: number; height?: number } | null;
    const world = target?.getWorldPointByBox?.({ x: Number(target.width ?? 0) / 2, y: Number(target.height ?? 0) / 2 });
    const client = world ? this.app?.getClientPointByWorld?.(world) : null;
    const bounds = this.host.getBoundingClientRect();
    if (!client || !Number.isFinite(client.x) || !Number.isFinite(client.y) || bounds.width <= 0 || bounds.height <= 0) return;
    this.host.dataset.treeaseWorkspaceProbe = JSON.stringify({ x: Number(client.x) - bounds.left, y: Number(client.y) - bounds.top });
  }
}

function graphBounds(graph: GraphData): { left: number; top: number; right: number; bottom: number } | null {
  if (!graph.nodes.length) return null;
  return graph.nodes.reduce((bounds, node) => ({
    left: Math.min(bounds.left, node.boxArgs.x),
    top: Math.min(bounds.top, node.boxArgs.y),
    right: Math.max(bounds.right, node.boxArgs.x + node.boxArgs.width),
    bottom: Math.max(bounds.bottom, node.boxArgs.y + node.boxArgs.height),
  }), { left: Infinity, top: Infinity, right: -Infinity, bottom: -Infinity });
}

export function createGraphViewerRuntime(options: GraphViewerRuntimeOptions): GraphViewerRuntime {
  return new GraphViewerRuntime(options);
}
