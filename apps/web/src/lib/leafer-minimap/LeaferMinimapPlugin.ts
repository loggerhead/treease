import {
  clampViewportToContent,
  computeContentBounds,
  computeMinimapScale,
  getViewportWorldBounds,
  getZoomScale,
  minimapDeltaToWorldDelta,
  worldToMinimapRect,
} from './geometry';
import type {
  LeaferMinimapPluginHandle,
  LeaferMinimapPluginOptions,
  MinimapAppLike,
  MinimapBounds,
  MinimapBoxLike,
  MinimapColors,
  MinimapEdge,
  MinimapNode,
} from './types';

const DEFAULT_WIDTH = 220;
const DEFAULT_HEIGHT = 150;
const DEFAULT_PADDING = 16;
const DEFAULT_CONTENT_PADDING = 24;
const CLICK_EPSILON = 3;

const DEFAULT_COLORS: Required<MinimapColors> = {
  background: 'rgba(255, 255, 255, 0.94)',
  border: 'rgba(203, 213, 225, 0.95)',
  node: 'rgba(148, 163, 184, 0.56)',
  tableNode: 'rgba(14, 165, 233, 0.5)',
  scalarNode: 'rgba(139, 92, 246, 0.48)',
  edge: 'rgba(100, 116, 139, 0.32)',
  viewportFill: 'rgba(14, 165, 233, 0.13)',
  viewportStroke: 'rgba(2, 132, 199, 0.9)',
};

type EventHandler = (event?: unknown) => void;

function resolveColors(colors?: MinimapColors): Required<MinimapColors> {
  return { ...DEFAULT_COLORS, ...colors };
}

function getEventClientPoint(event: unknown): { x: number; y: number } | null {
  const candidate = event as {
    clientX?: number;
    clientY?: number;
    x?: number;
    y?: number;
    origin?: { x?: number; y?: number };
  } | null;
  if (Number.isFinite(candidate?.clientX) && Number.isFinite(candidate?.clientY)) {
    return { x: Number(candidate?.clientX), y: Number(candidate?.clientY) };
  }
  if (Number.isFinite(candidate?.origin?.x) && Number.isFinite(candidate?.origin?.y)) {
    return { x: Number(candidate?.origin?.x), y: Number(candidate?.origin?.y) };
  }
  if (Number.isFinite(candidate?.x) && Number.isFinite(candidate?.y)) {
    return { x: Number(candidate?.x), y: Number(candidate?.y) };
  }
  return null;
}

export class LeaferMinimapPlugin implements LeaferMinimapPluginHandle {
  private readonly app: MinimapAppLike;
  private readonly container: HTMLElement;
  private readonly options: LeaferMinimapPluginOptions;
  private readonly colors: Required<MinimapColors>;
  private readonly width: number;
  private readonly height: number;
  private readonly padding: number;
  private readonly contentPadding: number;
  private readonly rootBox: MinimapBoxLike;
  private readonly backgroundBox: MinimapBoxLike;
  private readonly edgeLayer: MinimapBoxLike;
  private readonly nodeLayer: MinimapBoxLike;
  private readonly viewportBox: MinimapBoxLike;
  private readonly cleanups: Array<() => void> = [];
  private isFixedLayerMount = false;
  private contentBounds: MinimapBounds = { x: 0, y: 0, width: 1, height: 1 };
  private minimapScale = 1;
  private previewOffset = { x: 0, y: 0 };
  private screenPosition = { x: 0, y: 0 };
  private updateFrame: number | null = null;
  private viewportFrame: number | null = null;
  private dragClientStart: { x: number; y: number } | null = null;
  private dragViewStart: MinimapBounds | null = null;
  private clickClientStart: { x: number; y: number } | null = null;
  private visible = true;
  private isDraggingViewport = false;
  private destroyed = false;

  constructor(options: LeaferMinimapPluginOptions) {
    this.options = options;
    this.app = options.app;
    this.container = options.container;
    this.colors = resolveColors(options.colors);
    this.width = options.width ?? DEFAULT_WIDTH;
    this.height = options.height ?? DEFAULT_HEIGHT;
    this.padding = options.padding ?? DEFAULT_PADDING;
    this.contentPadding = options.contentPadding ?? DEFAULT_CONTENT_PADDING;

    const { BoxCtor } = options.constructors;
    this.rootBox = new BoxCtor({
      x: 0,
      y: 0,
      width: this.width,
      height: this.height,
      fill: 'transparent',
      hittable: true,
      hitChildren: true,
    });
    this.backgroundBox = new BoxCtor({
      x: 0,
      y: 0,
      width: this.width,
      height: this.height,
      fill: this.colors.background,
      stroke: this.colors.border,
      strokeWidth: 1,
      strokeAlign: 'inside',
      cornerRadius: 10,
      opacity: 1,
      hittable: true,
      hitChildren: false,
    });
    this.edgeLayer = new BoxCtor({ x: 0, y: 0, width: this.width, height: this.height, fill: 'transparent', hittable: false });
    this.nodeLayer = new BoxCtor({ x: 0, y: 0, width: this.width, height: this.height, fill: 'transparent', hittable: false });
    this.viewportBox = new BoxCtor({
      x: 0,
      y: 0,
      width: 0,
      height: 0,
      fill: this.colors.viewportFill,
      stroke: this.colors.viewportStroke,
      strokeWidth: 1,
      strokeAlign: 'inside',
      cornerRadius: 4,
      cursor: 'move',
      hittable: true,
      hitChildren: false,
    });

    this.rootBox.add?.(this.backgroundBox);
    this.rootBox.add?.(this.edgeLayer);
    this.rootBox.add?.(this.nodeLayer);
    this.rootBox.add?.(this.viewportBox);

    this.mount();
    this.bindEvents();
    this.updateLayout();
    this.update();
  }

  update(): void {
    this.scheduleFullUpdate();
  }

  updateLayout(): void {
    if (this.destroyed) return;
    this.options.mountApp?.resize?.({ width: this.width, height: this.height });
    const rect = this.container.getBoundingClientRect();
    this.screenPosition = {
      x: Math.max(this.padding, rect.width - this.width - this.padding),
      y: Math.max(this.padding, rect.height - this.height - this.padding),
    };
    if (this.options.mountApp) {
      this.rootBox.x = 0;
      this.rootBox.y = 0;
      this.rootBox.scaleX = 1;
      this.rootBox.scaleY = 1;
    } else if (this.isFixedLayerMount) {
      this.rootBox.x = this.screenPosition.x;
      this.rootBox.y = this.screenPosition.y;
      this.rootBox.scaleX = 1;
      this.rootBox.scaleY = 1;
    } else {
      const zoomLayer = this.app.zoomLayer;
      const { scaleX, scaleY } = getZoomScale(zoomLayer);
      this.rootBox.x = (this.screenPosition.x - (zoomLayer?.x ?? 0)) / scaleX;
      this.rootBox.y = (this.screenPosition.y - (zoomLayer?.y ?? 0)) / scaleY;
      this.rootBox.scaleX = 1 / scaleX;
      this.rootBox.scaleY = 1 / scaleY;
    }
    this.rootBox.width = this.width;
    this.rootBox.height = this.height;
    this.backgroundBox.width = this.width;
    this.backgroundBox.height = this.height;
  }

  updateViewport(): void {
    this.scheduleViewportUpdate();
  }

  destroy(): void {
    if (this.destroyed) return;
    this.destroyed = true;
    if (this.updateFrame !== null) cancelAnimationFrame(this.updateFrame);
    if (this.viewportFrame !== null) cancelAnimationFrame(this.viewportFrame);
    this.updateFrame = null;
    this.viewportFrame = null;
    this.detachWindowDrag();
    this.cleanups.splice(0).forEach((cleanup) => cleanup());
    this.rootBox.removeAll?.(true);
    this.rootBox.remove?.();
    this.rootBox.destroy?.();
  }

  private mount(): void {
    if (this.options.mountApp?.add) {
      this.isFixedLayerMount = true;
      this.options.mountApp.add(this.rootBox);
      return;
    }
    const sky = this.app.sky;
    if (sky?.add) {
      this.isFixedLayerMount = true;
      sky.add(this.rootBox);
      return;
    }
    this.isFixedLayerMount = false;
    this.app.add?.(this.rootBox);
  }

  private bindEvents(): void {
    this.bindAppEvent(this.options.events?.move, () => {
      this.updateLayout();
      this.updateViewport();
    });
    this.bindAppEvent(this.options.events?.zoom, () => {
      this.updateLayout();
      this.updateViewport();
    });
    this.bindBoxEvent(this.backgroundBox, this.options.events?.pointerDown, (event) => this.handleBackgroundPointerDown(event));
    this.bindBoxEvent(this.viewportBox, this.options.events?.pointerDown, (event) => this.handleViewportPointerDown(event));
  }

  private bindAppEvent(eventName: string | undefined, handler: EventHandler): void {
    if (!eventName) return;
    if (this.app.on_) {
      const listenerId = this.app.on_(eventName, handler);
      this.cleanups.push(() => {
        if (listenerId != null) this.app.off_?.([listenerId]);
      });
      return;
    }
    this.app.on?.(eventName, handler);
    this.cleanups.push(() => this.app.off?.(eventName, handler));
  }

  private bindBoxEvent(target: MinimapBoxLike, eventName: string | undefined, handler: EventHandler): void {
    if (!eventName) return;
    target.on?.(eventName, handler);
    this.cleanups.push(() => target.off?.(eventName, handler));
  }

  private scheduleFullUpdate(): void {
    if (this.destroyed || this.updateFrame !== null) return;
    this.updateFrame = requestAnimationFrame(() => {
      this.updateFrame = null;
      this.applyFullUpdate();
    });
  }

  private scheduleViewportUpdate(): void {
    if (this.destroyed || this.viewportFrame !== null || this.isDraggingViewport) return;
    this.viewportFrame = requestAnimationFrame(() => {
      this.viewportFrame = null;
      this.applyViewportUpdate();
    });
  }

  private applyFullUpdate(): void {
    if (this.destroyed) return;
    const viewData = this.options.getViewData();
    this.edgeLayer.removeAll?.(true);
    this.nodeLayer.removeAll?.(true);
    if (!viewData || viewData.nodes.length === 0) {
      this.setVisible(false);
      this.viewportBox.visible = false;
      return;
    }

    const viewBounds = getViewportWorldBounds(this.container, this.app.zoomLayer);
    if (viewBounds && this.areAllNodesInsideViewport(viewData.nodes, viewBounds)) {
      this.setVisible(false);
      this.viewportBox.visible = false;
      return;
    }
    this.setVisible(true);
    this.contentBounds = computeContentBounds(viewData.nodes, this.contentPadding);
    this.minimapScale = computeMinimapScale(this.contentBounds, this.width, this.height);
    this.previewOffset = {
      x: Math.max(0, (this.width - this.contentBounds.width * this.minimapScale) / 2),
      y: Math.max(0, (this.height - this.contentBounds.height * this.minimapScale) / 2),
    };

    this.renderEdges(viewData.edges);
    this.renderNodes(viewData.nodes);
    this.applyViewportUpdate();
  }

  private applyViewportUpdate(): void {
    if (this.destroyed) return;
    this.updateLayout();
    const viewData = this.options.getViewData();
    const viewBounds = getViewportWorldBounds(this.container, this.app.zoomLayer);
    if (!viewData || viewData.nodes.length === 0 || !viewBounds) {
      this.setVisible(false);
      this.viewportBox.visible = false;
      return;
    }
    if (this.areAllNodesInsideViewport(viewData.nodes, viewBounds)) {
      this.setVisible(false);
      this.viewportBox.visible = false;
      return;
    }
    this.setVisible(true);
    const minimapRect = this.clampRectToPanel(worldToMinimapRect(viewBounds, this.contentBounds, this.minimapScale, this.previewOffset));
    this.viewportBox.visible = true;
    this.viewportBox.x = minimapRect.x;
    this.viewportBox.y = minimapRect.y;
    this.viewportBox.width = minimapRect.width;
    this.viewportBox.height = minimapRect.height;
  }

  private renderNodes(nodes: MinimapNode[]): void {
    const { BoxCtor } = this.options.constructors;
    for (const node of nodes) {
      const rect = worldToMinimapRect(node, this.contentBounds, this.minimapScale, this.previewOffset);
      const box = new BoxCtor({
        x: rect.x,
        y: rect.y,
        width: Math.max(1, rect.width),
        height: Math.max(1, rect.height),
        fill: this.getNodeColor(node),
        cornerRadius: Math.min(3, Math.max(1, rect.width, rect.height) / 8),
        hittable: false,
        hitChildren: false,
      });
      this.nodeLayer.add?.(box);
    }
  }

  private renderEdges(edges: MinimapEdge[]): void {
    const { PenCtor } = this.options.constructors;
    for (const edge of edges) {
      const pen = new PenCtor();
      pen.setStyle?.({ stroke: this.colors.edge, strokeWidth: 1 });
      const from = this.worldPointToMinimap(edge.fromX, edge.fromY);
      const c1 = this.worldPointToMinimap(edge.c1x, edge.c1y);
      const c2 = this.worldPointToMinimap(edge.c2x, edge.c2y);
      const to = this.worldPointToMinimap(edge.toX, edge.toY);
      pen.moveTo?.(from.x, from.y);
      pen.bezierCurveTo?.(c1.x, c1.y, c2.x, c2.y, to.x, to.y);
      this.edgeLayer.add?.(pen);
    }
  }

  private getNodeColor(node: MinimapNode): string {
    if (node.kind === 'table') return this.colors.tableNode;
    if (node.kind === 'scalar') return this.colors.scalarNode;
    return this.colors.node;
  }

  private setVisible(visible: boolean): void {
    if (this.visible === visible) return;
    this.visible = visible;
    this.rootBox.visible = visible;
    if (this.options.mountContainer?.style) {
      this.options.mountContainer.style.display = visible ? '' : 'none';
    }
  }

  private areAllNodesInsideViewport(nodes: MinimapNode[], viewBounds: MinimapBounds): boolean {
    return nodes.every((node) => {
      const x = Number.isFinite(node.x) ? Number(node.x) : 0;
      const y = Number.isFinite(node.y) ? Number(node.y) : 0;
      const width = Math.max(0, Number.isFinite(node.width) ? Number(node.width) : 0);
      const height = Math.max(0, Number.isFinite(node.height) ? Number(node.height) : 0);
      return (
        x >= viewBounds.x &&
        y >= viewBounds.y &&
        x + width <= viewBounds.x + viewBounds.width &&
        y + height <= viewBounds.y + viewBounds.height
      );
    });
  }

  private worldPointToMinimap(x: number, y: number): { x: number; y: number } {
    return {
      x: this.previewOffset.x + (x - this.contentBounds.x) * this.minimapScale,
      y: this.previewOffset.y + (y - this.contentBounds.y) * this.minimapScale,
    };
  }

  private handleBackgroundPointerDown(event: unknown): void {
    const pointer = event as { stop?: () => void; stopNow?: () => void } | null;
    const point = this.getLocalPointFromEvent(event);
    const viewBounds = getViewportWorldBounds(this.container, this.app.zoomLayer);
    if (!point || !viewBounds) return;
    pointer?.stop?.();
    pointer?.stopNow?.();
    const worldCenter = {
      x: this.contentBounds.x + (point.x - this.previewOffset.x) / this.minimapScale,
      y: this.contentBounds.y + (point.y - this.previewOffset.y) / this.minimapScale,
    };
    this.applyWorldViewport({
      ...viewBounds,
      x: worldCenter.x - viewBounds.width / 2,
      y: worldCenter.y - viewBounds.height / 2,
    });
  }

  private handleViewportPointerDown(event: unknown): void {
    const pointer = event as { stop?: () => void; stopNow?: () => void } | null;
    const point = getEventClientPoint(event);
    const viewBounds = getViewportWorldBounds(this.container, this.app.zoomLayer);
    if (!point || !viewBounds) return;
    pointer?.stop?.();
    pointer?.stopNow?.();
    this.isDraggingViewport = true;
    this.dragClientStart = point;
    this.clickClientStart = point;
    this.dragViewStart = viewBounds;
    this.attachWindowDrag();
  }

  private attachWindowDrag(): void {
    if (typeof window === 'undefined') return;
    window.addEventListener('pointermove', this.handleWindowPointerMove);
    window.addEventListener('pointerup', this.handleWindowPointerUp);
  }

  private detachWindowDrag(): void {
    if (typeof window === 'undefined') return;
    window.removeEventListener('pointermove', this.handleWindowPointerMove);
    window.removeEventListener('pointerup', this.handleWindowPointerUp);
  }

  private handleWindowPointerMove = (event: PointerEvent): void => {
    if (!this.dragClientStart || !this.dragViewStart) return;
    const point = getEventClientPoint(event);
    if (!point) return;
    this.applyViewportDelta(
      {
        x: point.x - this.dragClientStart.x,
        y: point.y - this.dragClientStart.y,
      },
      this.dragViewStart,
    );
  };

  private handleWindowPointerUp = (): void => {
    this.detachWindowDrag();
    const wasClick =
      this.clickClientStart &&
      this.dragClientStart &&
      Math.abs(this.dragClientStart.x - this.clickClientStart.x) <= CLICK_EPSILON &&
      Math.abs(this.dragClientStart.y - this.clickClientStart.y) <= CLICK_EPSILON;
    this.dragClientStart = null;
    this.clickClientStart = null;
    this.dragViewStart = null;
    this.isDraggingViewport = false;
    if (!wasClick) this.update();
  };

  private applyViewportDelta(delta: { x: number; y: number }, baseView?: MinimapBounds): void {
    const zoomLayer = this.app.zoomLayer;
    if (!zoomLayer) return;
    const currentView = baseView ?? getViewportWorldBounds(this.container, zoomLayer);
    if (!currentView) return;
    const worldDelta = minimapDeltaToWorldDelta(delta, this.minimapScale);
    this.applyWorldViewport({
      ...currentView,
      x: currentView.x + worldDelta.x,
      y: currentView.y + worldDelta.y,
    });
  }

  private applyWorldViewport(view: MinimapBounds): void {
    const zoomLayer = this.app.zoomLayer;
    if (!zoomLayer) return;
    const nextView = clampViewportToContent(view, this.contentBounds);
    const { scaleX, scaleY } = getZoomScale(zoomLayer);
    const nextX = -nextView.x * scaleX;
    const nextY = -nextView.y * scaleY;
    if (Math.abs((zoomLayer.x ?? 0) - nextX) < 0.5 && Math.abs((zoomLayer.y ?? 0) - nextY) < 0.5) {
      this.applyViewportUpdate();
      return;
    }
    zoomLayer.x = nextX;
    zoomLayer.y = nextY;
    this.app.update?.();
    this.options.onViewportChange?.();
    this.applyViewportUpdate();
  }

  private getLocalPointFromEvent(event: unknown): { x: number; y: number } | null {
    const point = getEventClientPoint(event);
    if (!point) return null;
    const rect = (this.options.mountContainer ?? this.container).getBoundingClientRect();
    const rectLeft = Number.isFinite(rect.left) ? Number(rect.left) : 0;
    const rectTop = Number.isFinite(rect.top) ? Number(rect.top) : 0;
    const offsetX = this.options.mountContainer ? 0 : this.screenPosition.x;
    const offsetY = this.options.mountContainer ? 0 : this.screenPosition.y;
    const x = point.x - rectLeft - offsetX;
    const y = point.y - rectTop - offsetY;
    if (!Number.isFinite(x) || !Number.isFinite(y)) return null;
    return { x, y };
  }

  private clampRectToPanel(rect: MinimapBounds): MinimapBounds {
    const left = Math.min(Math.max(rect.x, 0), this.width);
    const top = Math.min(Math.max(rect.y, 0), this.height);
    const right = Math.min(Math.max(rect.x + rect.width, 0), this.width);
    const bottom = Math.min(Math.max(rect.y + rect.height, 0), this.height);
    return {
      x: Math.min(left, right),
      y: Math.min(top, bottom),
      width: Math.abs(right - left),
      height: Math.abs(bottom - top),
    };
  }
}
