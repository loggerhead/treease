// Responsibility: GraphViewer viewport geometry helpers for center, zoom scale, bounds, intersection, and applyZoom.
import type { App as LeaferApp, Box } from "leafer-ui";
import type { GraphBoxArgs, GraphNode } from "../../graph/graph-viewer-render";

export type LeaferZoomLayer = {
  x?: number;
  y?: number;
  scaleX?: number;
  scaleY?: number;
  __?: { scaleX?: number; scaleY?: number };
};

export type ViewportBounds = {
  left: number;
  right: number;
  top: number;
  bottom: number;
};

export type GraphWorldBounds = ViewportBounds;
export const GRAPH_PAN_CONSTRAINT_PADDING = 500;

export type ViewportState = {
  leafer: LeaferApp | null;
  container: HTMLDivElement | null;
  edgeLayer: Box | null;
  nodeLayer: Box | null;
  lastAutoOffset: { x: number; y: number } | null;
};

export function getViewportCenter(container: HTMLElement | null): { x: number; y: number } {
  if (!container) return { x: 0, y: 0 };
  const rect = container.getBoundingClientRect();
  return { x: rect.width / 2, y: rect.height / 2 };
}

export function getZoomScale(layer: LeaferZoomLayer | null | undefined): {
  scaleX: number;
  scaleY: number;
} {
  const scaleX = layer?.scaleX ?? layer?.__?.scaleX ?? 1;
  const scaleY = layer?.scaleY ?? layer?.__?.scaleY ?? 1;
  return { scaleX, scaleY };
}

export function clampPanOffsetToGraphBounds(
  viewport: {
    viewportWidth: number;
    viewportHeight: number;
    scaleX: number;
    scaleY: number;
    offsetX: number;
    offsetY: number;
  },
  bounds: GraphWorldBounds,
  padding = GRAPH_PAN_CONSTRAINT_PADDING,
): { x: number; y: number } {
  const normalizeZero = (value: number) => (Object.is(value, -0) ? 0 : value);
  const left = bounds.left - padding;
  const top = bounds.top - padding;
  const right = bounds.right + padding;
  const bottom = bounds.bottom + padding;
  const contentWidth = (right - left) * viewport.scaleX;
  const contentHeight = (bottom - top) * viewport.scaleY;

  let minX = viewport.viewportWidth - right * viewport.scaleX;
  let maxX = -left * viewport.scaleX;
  let minY = viewport.viewportHeight - bottom * viewport.scaleY;
  let maxY = -top * viewport.scaleY;

  if (contentWidth <= viewport.viewportWidth) {
    const centeredX = (viewport.viewportWidth - contentWidth) / 2 - left * viewport.scaleX;
    minX = centeredX;
    maxX = centeredX;
  }

  if (contentHeight <= viewport.viewportHeight) {
    const centeredY = (viewport.viewportHeight - contentHeight) / 2 - top * viewport.scaleY;
    minY = centeredY;
    maxY = centeredY;
  }

  return {
    x: normalizeZero(Math.min(maxX, Math.max(minX, viewport.offsetX))),
    y: normalizeZero(Math.min(maxY, Math.max(minY, viewport.offsetY))),
  };
}

export function getViewportBounds(
  container: HTMLElement | null,
  leafer: (LeaferApp & { zoomLayer?: LeaferZoomLayer }) | null,
): ViewportBounds | null {
  if (!container || typeof container.getBoundingClientRect !== "function" || !leafer?.zoomLayer)
    return null;
  const rect = container.getBoundingClientRect();
  const layer = leafer.zoomLayer;
  const { scaleX, scaleY } = getZoomScale(layer);
  if (!Number.isFinite(scaleX) || !Number.isFinite(scaleY) || scaleX <= 0 || scaleY <= 0)
    return null;
  const offsetX = layer.x ?? 0;
  const offsetY = layer.y ?? 0;
  return {
    left: (0 - offsetX) / scaleX,
    right: (rect.width - offsetX) / scaleX,
    top: (0 - offsetY) / scaleY,
    bottom: (rect.height - offsetY) / scaleY,
  };
}

export function isPointInBounds(point: { x: number; y: number }, bounds: ViewportBounds): boolean {
  return (
    point.x >= bounds.left &&
    point.x <= bounds.right &&
    point.y >= bounds.top &&
    point.y <= bounds.bottom
  );
}

export function doesBoxIntersectBounds(box: GraphBoxArgs, bounds: ViewportBounds): boolean {
  const boxRight = box.x + box.width;
  const boxBottom = box.y + box.height;
  if (boxRight < bounds.left) return false;
  if (box.x > bounds.right) return false;
  if (boxBottom < bounds.top) return false;
  if (box.y > bounds.bottom) return false;
  return true;
}

export function applyZoom(
  state: ViewportState,
  changeScale: number,
  getValidScale?: (scale: number) => number,
): void {
  const { leafer, container } = state;
  if (!leafer?.zoomLayer || !container) return;
  const layer = leafer.zoomLayer as LeaferZoomLayer;
  const center = getViewportCenter(container);
  const { scaleX } = getZoomScale(layer);
  const validScale = typeof getValidScale === "function" ? getValidScale(changeScale) : changeScale;
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
  (leafer as any).update?.();
}

export function updateSize(state: ViewportState): void {
  const { leafer, container } = state;
  if (!leafer || !container) return;
  const { width, height } = container.getBoundingClientRect();
  if (width > 0 && height > 0) {
    leafer.resize({ width, height });
  }
}

export function centerOnNode(state: ViewportState, node: GraphNode): void {
  const { leafer, container } = state;
  if (!leafer?.zoomLayer || !container) return;
  const layer = leafer.zoomLayer as LeaferZoomLayer;
  const center = getViewportCenter(container);
  const { scaleX, scaleY } = getZoomScale(layer);
  const targetX = node.boxArgs.x + node.boxArgs.width / 2;
  const targetY = node.boxArgs.y + node.boxArgs.height / 2;
  layer.x = center.x - targetX * scaleX;
  layer.y = center.y - targetY * scaleY;
  state.lastAutoOffset = { x: layer.x ?? 0, y: layer.y ?? 0 };
  (leafer as any).update?.();
}
