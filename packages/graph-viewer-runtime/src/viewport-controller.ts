// Responsibility: control GraphViewer viewport interactions, including mouse/touch pan and zoom, centerOnNode, event binding, and state adaptation.
import type { GraphNode } from './types';
import type { LeaferEventTarget } from './pointer-controller';
import {
  GRAPH_PAN_CONSTRAINT_PADDING,
  clampPanOffsetToGraphBounds,
  getViewportCenter as getViewportCenterFromContainer,
  getZoomScale,
  type GraphWorldBounds,
  type LeaferZoomLayer,
} from './viewport-geometry';

export type { LeaferZoomLayer } from './viewport-geometry';

type LeaferAppLike = {
  zoomLayer?: unknown;
  resize?: (options: { width: number; height: number }) => void;
  update?: () => void;
  zoom?: (
    target: unknown,
    padding?: number,
    scroll?: boolean | 'x' | 'y',
    transition?: boolean | number | LeaferZoomTransition,
  ) => unknown;
};
type LeaferBox = { width?: number; height?: number };
type LeaferZoomTransition = {
  duration: number;
  event: {
    update: () => void;
    completed: () => void;
  };
};

const GRAPH_REVEAL_TRANSITION_SECONDS = 0.25;

type GraphViewportRequest = {
  x: number;
  y: number;
  width: number;
  height: number;
};

type CreateGraphViewportControllerOptions = {
  getContainer: () => HTMLDivElement | null;
  getLeafer: () =>
    | (LeaferAppLike & { zoomLayer?: LeaferZoomLayer; getValidScale?: (scale: number) => number })
    | null;
  getSuppressGraphPointerUntil: () => number;
  getMoveEventName: () => string | undefined;
  getZoomEventName: () => string | undefined;
  bindPointerClick: (
    target: LeaferEventTarget,
    handler: (event: unknown) => void | Promise<void>,
  ) => void;
  updateRenderableProjection?: () => void;
  updateViewportOverlays: () => void;
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
    options.updateRenderableProjection?.();
    options.updateViewportOverlays();
  }

  function handleViewportMove(): void {
    clampViewportPanOffset();
    syncViewportOverlays();
  }

  function handleViewportZoom(): void {
    handleViewportMove();
  }

  function createRevealTransition(): LeaferZoomTransition {
    // Leafer applies an animated zoom asynchronously. Its transition lifecycle
    // is the single source of viewport-change notifications for programmatic
    // reveals, so GraphSceneRuntime can schedule the projection from the
    // actual animated viewport rather than from a guessed delay.
    return {
      duration: GRAPH_REVEAL_TRANSITION_SECONDS,
      event: {
        update: handleViewportMove,
        completed: handleViewportMove,
      },
    };
  }

  function moveToWorldViewport(view: GraphViewportRequest): void {
    const leafer = options.getLeafer();
    if (!leafer?.zoomLayer || !Number.isFinite(view.x) || !Number.isFinite(view.y)) return;
    const layer = leafer.zoomLayer as LeaferZoomLayer;
    const { scaleX, scaleY } = getZoomScale(layer);
    layer.x = -view.x * scaleX;
    layer.y = -view.y * scaleY;
    leafer.update?.();
    handleViewportMove();
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
    const validScale =
      typeof leafer.getValidScale === "function" ? leafer.getValidScale(changeScale) : changeScale;
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
    handleViewportMove();
  }

  function centerOnBox(box: LeaferBox): boolean {
    const leafer = options.getLeafer();
    if (!leafer?.zoom) return false;
    leafer.zoom(box, 0, true, createRevealTransition());
    return true;
  }

  function centerOnNode(node: GraphNode): void {
    const leafer = options.getLeafer();
    leafer?.zoom?.(node.boxArgs, 0, true, createRevealTransition());
  }

  return {
    updateSize,
    getViewportCenter,
    getZoomScale,
    syncViewportOverlays,
    clampViewportPanOffset,
    handleViewportMove,
    handleViewportZoom,
    moveToWorldViewport,
    handleCanvasClick,
    registerViewportEvents,
    applyZoom,
    centerOnBox,
    centerOnNode,
  };
}
