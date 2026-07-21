// Responsibility: control GraphViewer viewport interactions, including mouse/touch pan and zoom, centerOnNode, event binding, and state adaptation.
import type { GraphNode } from '../../graph/graph-viewer-render';
import type { LeaferAppLike, LeaferBox } from './model';
import type { LeaferEventTarget } from './graph-pointer-controller';
import {
  GRAPH_PAN_CONSTRAINT_PADDING,
  clampPanOffsetToGraphBounds,
  getViewportCenter as getViewportCenterFromContainer,
  getZoomScale,
  type GraphWorldBounds,
  type LeaferZoomLayer,
} from './graph-viewport-geometry';

export type { LeaferZoomLayer } from './graph-viewport-geometry';


type CreateGraphViewportControllerOptions = {
  getContainer: () => HTMLDivElement | null;
  getLeafer: () => (LeaferAppLike & { zoomLayer?: LeaferZoomLayer; getValidScale?: (scale: number) => number }) | null;
  getSuppressGraphPointerUntil: () => number;
  getMoveEventName: () => string | undefined;
  getZoomEventName: () => string | undefined;
  bindPointerClick: (target: LeaferEventTarget, handler: (event: unknown) => void | Promise<void>) => void;
  updateViewportOverlays: () => void;
  getLastAutoOffset: () => { x: number; y: number } | null;
  setLastAutoOffset: (value: { x: number; y: number } | null) => void;
  getPanConstraintBounds?: () => GraphWorldBounds | null;
};

export function createGraphViewportController(options: CreateGraphViewportControllerOptions) {
  function clampViewportPanOffset(): void {
    const leafer = options.getLeafer();
    const container = options.getContainer();
    const bounds = options.getPanConstraintBounds?.() ?? null;
    if (!leafer?.zoomLayer || !container || !bounds) return;
    const layer = leafer.zoomLayer as LeaferZoomLayer;
    const { scaleX, scaleY } = getZoomScale(layer);
    const rect = container.getBoundingClientRect();
    const clamped = clampPanOffsetToGraphBounds(
      {
        viewportWidth: rect.width,
        viewportHeight: rect.height,
        scaleX,
        scaleY,
        offsetX: layer.x ?? 0,
        offsetY: layer.y ?? 0,
      },
      bounds,
      GRAPH_PAN_CONSTRAINT_PADDING,
    );
    layer.x = clamped.x;
    layer.y = clamped.y;
  }

  function updateSize(): void {
    const leafer = options.getLeafer();
    const container = options.getContainer();
    if (!leafer || !container) return;
    const { width, height } = container.getBoundingClientRect();
    if (width > 0 && height > 0) {
      leafer.resize?.({ width, height });
    }
  }

  function getViewportCenter(): { x: number; y: number } {
    return getViewportCenterFromContainer(options.getContainer());
  }

  function syncViewportOverlays(): void {
    options.updateViewportOverlays();
  }

  function handleViewportMove(): void {
    clampViewportPanOffset();
    syncViewportOverlays();
  }

  function handleViewportZoom(): void {
    syncViewportOverlays();
  }

  function handleCanvasClick(): void {
    if (Date.now() < options.getSuppressGraphPointerUntil()) return;
  }

  function registerViewportEvents(target: LeaferEventTarget): void {
    if (!target?.on) return;
    const moveEvent = options.getMoveEventName();
    const zoomEvent = options.getZoomEventName();
    if (moveEvent) target.on(moveEvent, handleViewportMove);
    if (zoomEvent) target.on(zoomEvent, handleViewportZoom);
    options.bindPointerClick(target, handleCanvasClick);
  }

  function applyZoom(changeScale: number): void {
    const leafer = options.getLeafer();
    const container = options.getContainer();
    if (!leafer?.zoomLayer || !container) return;
    const layer = leafer.zoomLayer as LeaferZoomLayer;
    const center = getViewportCenter();
    const { scaleX } = getZoomScale(layer);
    const validScale = typeof leafer.getValidScale === 'function' ? leafer.getValidScale(changeScale) : changeScale;
    const nextScale = scaleX * validScale;
    if (!Number.isFinite(nextScale) || nextScale <= 0) return;
    const currentX = layer.x ?? 0;
    const currentY = layer.y ?? 0;
    const worldX = (center.x - currentX) / scaleX;
    const worldY = (center.y - currentY) / scaleX;
    layer.scaleX = nextScale;
    layer.scaleY = nextScale;
    layer.x = center.x - worldX * nextScale;
    layer.y = center.y - worldY * nextScale;
    leafer.update?.();
  }

  function centerOnBox(box: LeaferBox): boolean {
    const leafer = options.getLeafer();
    if (!leafer?.zoomLayer) return false;
    const layer = leafer.zoomLayer as LeaferZoomLayer;
    const worldBox = box as LeaferBox & {
      getWorldPointByBox?: (point: { x: number; y: number }) => { x?: number; y?: number } | null;
    };
    const worldLeafer = leafer as LeaferAppLike & {
      updateClientBounds?: () => void;
      clientBounds?: { x: number; y: number; width: number; height: number };
      getClientPointByWorld?: (point: { x: number; y: number }) => { x?: number; y?: number } | null;
    };
    const highlightWorld =
      typeof worldBox.getWorldPointByBox === 'function'
        ? worldBox.getWorldPointByBox({ x: (box.width ?? 0) / 2, y: (box.height ?? 0) / 2 })
        : null;
    const highlightWorldX = Number(highlightWorld?.x);
    const highlightWorldY = Number(highlightWorld?.y);
    if (!Number.isFinite(highlightWorldX) || !Number.isFinite(highlightWorldY)) return false;
    worldLeafer.updateClientBounds?.();
    const clientBounds = worldLeafer.clientBounds;
    if (!clientBounds) return false;
    const highlightClient =
      typeof worldLeafer.getClientPointByWorld === 'function'
        ? worldLeafer.getClientPointByWorld({ x: highlightWorldX, y: highlightWorldY })
        : null;
    const highlightClientX = Number(highlightClient?.x);
    const highlightClientY = Number(highlightClient?.y);
    if (!Number.isFinite(highlightClientX) || !Number.isFinite(highlightClientY)) return false;
    const viewportClientCenterX = clientBounds.x + clientBounds.width / 2;
    const viewportClientCenterY = clientBounds.y + clientBounds.height / 2;
    layer.x = (layer.x ?? 0) + viewportClientCenterX - highlightClientX;
    layer.y = (layer.y ?? 0) + viewportClientCenterY - highlightClientY;
    options.setLastAutoOffset({ x: layer.x ?? 0, y: layer.y ?? 0 });
    leafer.update?.();
    return true;
  }

  function centerOnNode(node: GraphNode): void {
    const leafer = options.getLeafer();
    const container = options.getContainer();
    if (!leafer?.zoomLayer || !container) return;
    const layer = leafer.zoomLayer as LeaferZoomLayer;
    const center = getViewportCenter();
    const { scaleX, scaleY } = getZoomScale(layer);
    const targetX = node.boxArgs.x + node.boxArgs.width / 2;
    const targetY = node.boxArgs.y + node.boxArgs.height / 2;
    layer.x = center.x - targetX * scaleX;
    layer.y = center.y - targetY * scaleY;
    options.setLastAutoOffset({ x: layer.x ?? 0, y: layer.y ?? 0 });
    leafer.update?.();
  }

  return {
    updateSize,
    getViewportCenter,
    getZoomScale,
    syncViewportOverlays,
    clampViewportPanOffset,
    handleViewportMove,
    handleViewportZoom,
    handleCanvasClick,
    registerViewportEvents,
    applyZoom,
    centerOnBox,
    centerOnNode,
  };
}
