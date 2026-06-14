// 职责：GraphViewer 领域的视口几何纯函数：center、zoom scale、bounds、intersection、applyZoom
import type { App as LeaferApp, Box } from 'leafer-ui';
import type { GraphBoxArgs, GraphNode } from '../../graph/graph-viewer-render';

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

export function getZoomScale(layer: LeaferZoomLayer | null | undefined): { scaleX: number; scaleY: number } {
  const scaleX = layer?.scaleX ?? layer?.__?.scaleX ?? 1;
  const scaleY = layer?.scaleY ?? layer?.__?.scaleY ?? 1;
  return { scaleX, scaleY };
}

export function getViewportBounds(container: HTMLElement | null, leafer: (LeaferApp & { zoomLayer?: LeaferZoomLayer }) | null): ViewportBounds | null {
  if (!container || !leafer?.zoomLayer) return null;
  const rect = container.getBoundingClientRect();
  const layer = leafer.zoomLayer;
  const { scaleX, scaleY } = getZoomScale(layer);
  if (!Number.isFinite(scaleX) || !Number.isFinite(scaleY) || scaleX <= 0 || scaleY <= 0) return null;
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
  return point.x >= bounds.left && point.x <= bounds.right && point.y >= bounds.top && point.y <= bounds.bottom;
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
  const validScale = typeof getValidScale === 'function' ? getValidScale(changeScale) : changeScale;
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
