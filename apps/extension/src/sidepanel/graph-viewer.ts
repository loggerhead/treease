// Responsibility: host the same Leafer graph rendering kernel used by Treease Web.
// The extension deliberately owns only the shell and local Core document job; node,
// table and edge drawing stay in the Web GraphViewer implementation.
import { graphViewerConfig } from '@treease-web/lib/settings/ui-settings';
import { renderGraphEdges, renderGraphNode } from '@treease-web/lib/components/graph-viewer/graph-render-kernel';
import { createGraphPointerController } from '@treease-web/lib/components/graph-viewer/graph-pointer-controller';
import type { GraphCell, GraphCellKind } from '@treease-web/lib/graph/graph-viewer-render';
import type { GraphData } from '../shared/types';
import { resolveWorkspacePath } from './workspace-path';

type Path = GraphData['nodes'][number]['path'];
type SelectPath = (path: Path, options: { openWorkspace: boolean; workspacePath?: Path }) => void;

type LeaferModules = {
  App: new (config: Record<string, unknown>) => any;
  Leafer: new (config: Record<string, unknown>) => any;
  Box: new (config: Record<string, unknown>) => any;
  Text: new (config: Record<string, unknown>) => any;
  Pen: new () => any;
  PointerEvent?: { TAP?: string; CLICK?: string; DOWN?: string; MOVE?: string; UP?: string };
  MoveEvent?: { BEFORE_MOVE?: string; MOVE?: string };
};

/**
 * A read-only GraphViewer surface for the extension. Its rendering functions and
 * visual configuration are imported from apps/web, rather than reimplemented.
 */
export class ExtensionGraphViewer {
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
  private resizeObserver: ResizeObserver | null = null;
  private disposed = false;

  constructor(private readonly host: HTMLElement, private readonly onSelectPath: SelectPath) {
    host.addEventListener('click', this.handleCanvasClick, true);
    host.addEventListener('pointerdown', this.handleCanvasClick, true);
  }

  async render(graph: GraphData): Promise<void> {
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
      styleConfig: graphViewerConfig,
      languageIdValue: 'json',
      fontSize: graphViewerConfig.layout.baseFontSize,
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
          this.highlight(target.parent ?? target);
          const workspacePath = resolveWorkspacePath(this.graph, cell.path as Path);
          this.onSelectPath(cell.path as Path, { openWorkspace: workspacePath !== null, workspacePath: workspacePath ?? undefined });
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
      renderConfig: graphViewerConfig,
    });
    for (const node of graph.nodes) {
      const result = renderGraphNode({
        node,
        drawContext,
        registerMetaClickTarget: (target: any, cell: GraphCell) => {
          pointerController.bindPointerClick(target, () => {
            this.lastTargetActivationAt = Date.now();
            this.highlight(target.parent ?? target);
            const workspacePath = resolveWorkspacePath(this.graph, cell.path as Path);
            this.onSelectPath(cell.path as Path, { openWorkspace: workspacePath !== null, workspacePath: workspacePath ?? undefined });
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

  destroy(): void {
    this.disposed = true;
    this.resizeObserver?.disconnect();
    this.resizeObserver = null;
    this.tableRuntimes.forEach((runtime) => runtime.destroy?.());
    this.tableRuntimes = [];
    this.host.removeEventListener('click', this.handleCanvasClick, true);
    this.host.removeEventListener('pointerdown', this.handleCanvasClick, true);
    this.app?.view?.removeEventListener?.('click', this.handleCanvasClick, true);
    this.app?.view?.removeEventListener?.('pointerdown', this.handleCanvasClick, true);
    this.app?.destroy?.();
    this.app = null;
    this.edgeLayer = null;
    this.nodeLayer = null;
  }

  private async ensureRuntime(): Promise<void> {
    if (this.app || this.disposed) return;
    // Viewport supplies the exact pan/zoom model GraphViewer uses in Treease Web.
    await import('@leafer-in/viewport');
    const module = await import('leafer-ui') as unknown as LeaferModules;
    if (this.disposed) return;
    this.modules = module;
    // Web's GraphRuntimeHost supports both App and Leafer. A Side Panel needs one
    // viewport root (not App's multi-canvas wrapper), so select Leafer explicitly.
    this.app = new module.Leafer({
      view: this.host,
      type: 'viewport',
      move: { drag: false, holdSpaceKey: true, holdRightKey: true, scroll: true },
      zoom: { disabled: false },
      wheel: { zoomMode: false },
      multiTouch: { disabled: false },
    });
    const root = this.app.zoomLayer ?? this.app;
    this.edgeLayer = new module.Box({ fill: 'transparent', hittable: false, hitChildren: false });
    this.nodeLayer = new module.Box({ fill: 'transparent' });
    root.add(this.edgeLayer);
    root.add(this.nodeLayer);
    this.app.view?.addEventListener?.('click', this.handleCanvasClick, true);
    this.app.view?.addEventListener?.('pointerdown', this.handleCanvasClick, true);
    this.resize();
    this.resizeObserver = new ResizeObserver(() => this.resize());
    this.resizeObserver.observe(this.host);
  }

  private resize(): void {
    const bounds = this.host.getBoundingClientRect();
    if (bounds.width > 0 && bounds.height > 0) this.app?.resize?.({ width: bounds.width, height: bounds.height });
  }

  private fitToGraph(graph: GraphData): void {
    const zoomLayer = this.app?.zoomLayer;
    if (!zoomLayer || graph.nodes.length === 0) return;
    const left = Math.min(...graph.nodes.map((node) => node.boxArgs.x));
    const top = Math.min(...graph.nodes.map((node) => node.boxArgs.y));
    zoomLayer.x = graphViewerConfig.layout.canvasPadding - left;
    zoomLayer.y = graphViewerConfig.layout.canvasPadding - top;
  }

  private requestRender(): void {
    this.app?.update?.();
    this.app?.forceRender?.();
  }

  private highlight(target: any): void {
    for (const previous of this.highlightedTargets) {
      previous.selected = false;
      previous.selectedStyle = undefined;
    }
    this.highlightedTargets = [target];
    target.selectedStyle = { stroke: '#84a300', strokeWidth: 2, fill: '#f8ffe1' };
    target.selected = true;
    this.requestRender();
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
      this.onSelectPath(node.path as Path, { openWorkspace: node.kind === 'object' || node.kind === 'table', workspacePath: node.path as Path });
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
