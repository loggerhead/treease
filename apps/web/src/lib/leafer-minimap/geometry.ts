import type { MinimapBounds, MinimapNode, MinimapZoomLayerLike } from './types';

export type MinimapScale = {
  scaleX: number;
  scaleY: number;
};

export type MinimapPoint = {
  x: number;
  y: number;
};

const MIN_BOUNDS_SIZE = 1;

function finiteOr(value: unknown, fallback: number): number {
  return Number.isFinite(value) ? Number(value) : fallback;
}

export function normalizeBounds(bounds: MinimapBounds): MinimapBounds {
  return {
    x: finiteOr(bounds.x, 0),
    y: finiteOr(bounds.y, 0),
    width: Math.max(MIN_BOUNDS_SIZE, finiteOr(bounds.width, MIN_BOUNDS_SIZE)),
    height: Math.max(MIN_BOUNDS_SIZE, finiteOr(bounds.height, MIN_BOUNDS_SIZE)),
  };
}

export function computeContentBounds(nodes: MinimapNode[], padding = 0): MinimapBounds {
  if (nodes.length === 0) {
    return { x: 0, y: 0, width: MIN_BOUNDS_SIZE, height: MIN_BOUNDS_SIZE };
  }

  let minX = Number.POSITIVE_INFINITY;
  let minY = Number.POSITIVE_INFINITY;
  let maxX = Number.NEGATIVE_INFINITY;
  let maxY = Number.NEGATIVE_INFINITY;

  for (const node of nodes) {
    const x = finiteOr(node.x, 0);
    const y = finiteOr(node.y, 0);
    const width = Math.max(0, finiteOr(node.width, 0));
    const height = Math.max(0, finiteOr(node.height, 0));
    minX = Math.min(minX, x);
    minY = Math.min(minY, y);
    maxX = Math.max(maxX, x + width);
    maxY = Math.max(maxY, y + height);
  }

  if (!Number.isFinite(minX) || !Number.isFinite(minY) || !Number.isFinite(maxX) || !Number.isFinite(maxY)) {
    return { x: 0, y: 0, width: MIN_BOUNDS_SIZE, height: MIN_BOUNDS_SIZE };
  }

  const safePadding = Math.max(0, finiteOr(padding, 0));
  return normalizeBounds({
    x: minX - safePadding,
    y: minY - safePadding,
    width: maxX - minX + safePadding * 2,
    height: maxY - minY + safePadding * 2,
  });
}

export function computeMinimapScale(contentBounds: MinimapBounds, width: number, height: number): number {
  const bounds = normalizeBounds(contentBounds);
  const safeWidth = Math.max(MIN_BOUNDS_SIZE, finiteOr(width, MIN_BOUNDS_SIZE));
  const safeHeight = Math.max(MIN_BOUNDS_SIZE, finiteOr(height, MIN_BOUNDS_SIZE));
  return Math.min(safeWidth / bounds.width, safeHeight / bounds.height);
}

export function getZoomScale(zoomLayer: MinimapZoomLayerLike | null | undefined): MinimapScale {
  const rawScale = zoomLayer?.scale ?? zoomLayer?.__?.scale;
  const scaleFromObjectX = typeof rawScale === 'object' ? rawScale.x : undefined;
  const scaleFromObjectY = typeof rawScale === 'object' ? rawScale.y : undefined;
  const scaleFromNumber = typeof rawScale === 'number' ? rawScale : undefined;
  const scaleX = finiteOr(zoomLayer?.scaleX ?? zoomLayer?.__?.scaleX ?? scaleFromObjectX ?? scaleFromNumber, 1);
  const scaleY = finiteOr(zoomLayer?.scaleY ?? zoomLayer?.__?.scaleY ?? scaleFromObjectY ?? scaleFromNumber, scaleX);
  return {
    scaleX: scaleX > 0 ? scaleX : 1,
    scaleY: scaleY > 0 ? scaleY : 1,
  };
}

export function getViewportWorldBounds(
  container: HTMLElement | { getBoundingClientRect?: () => { width?: number; height?: number } } | null,
  zoomLayer: MinimapZoomLayerLike | null | undefined,
): MinimapBounds | null {
  if (!container || !zoomLayer) return null;
  const rect = container.getBoundingClientRect?.();
  const width = finiteOr(rect?.width, 0);
  const height = finiteOr(rect?.height, 0);
  if (width <= 0 || height <= 0) return null;

  const { scaleX, scaleY } = getZoomScale(zoomLayer);
  const offsetX = finiteOr(zoomLayer.x, 0);
  const offsetY = finiteOr(zoomLayer.y, 0);
  return {
    x: -offsetX / scaleX,
    y: -offsetY / scaleY,
    width: width / scaleX,
    height: height / scaleY,
  };
}

export function worldToMinimapRect(
  bounds: MinimapBounds,
  contentBounds: MinimapBounds,
  scale: number,
  offset: MinimapPoint = { x: 0, y: 0 },
): MinimapBounds {
  const safeScale = Math.max(0, finiteOr(scale, 0));
  const content = normalizeBounds(contentBounds);
  return {
    x: offset.x + (bounds.x - content.x) * safeScale,
    y: offset.y + (bounds.y - content.y) * safeScale,
    width: bounds.width * safeScale,
    height: bounds.height * safeScale,
  };
}

export function minimapDeltaToWorldDelta(delta: MinimapPoint, scale: number): MinimapPoint {
  const safeScale = Math.max(Number.EPSILON, finiteOr(scale, 1));
  return {
    x: finiteOr(delta.x, 0) / safeScale,
    y: finiteOr(delta.y, 0) / safeScale,
  };
}

export function clampViewportToContent(view: MinimapBounds, contentBounds: MinimapBounds): MinimapBounds {
  const normalizedView = normalizeBounds(view);
  const content = normalizeBounds(contentBounds);
  const maxX = content.x + content.width - normalizedView.width;
  const maxY = content.y + content.height - normalizedView.height;

  return {
    x:
      normalizedView.width >= content.width
        ? content.x + (content.width - normalizedView.width) / 2
        : Math.min(Math.max(normalizedView.x, content.x), maxX),
    y:
      normalizedView.height >= content.height
        ? content.y + (content.height - normalizedView.height) / 2
        : Math.min(Math.max(normalizedView.y, content.y), maxY),
    width: normalizedView.width,
    height: normalizedView.height,
  };
}
