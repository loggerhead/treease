import type { MinimapBounds, MinimapNode, MinimapZoomLayerLike } from './types';

export type MinimapScale = {
  scaleX: number;
  scaleY: number;
};

export type MinimapPoint = {
  x: number;
  y: number;
};

export type MinimapTransform = {
  contentBounds: MinimapBounds;
  scaleX: number;
  scaleY: number;
  offset: MinimapPoint;
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

export function createMinimapTransform(
  contentBounds: MinimapBounds,
  width: number,
  height: number,
): MinimapTransform {
  const bounds = normalizeBounds(contentBounds);
  const safeWidth = Math.max(MIN_BOUNDS_SIZE, finiteOr(width, MIN_BOUNDS_SIZE));
  const safeHeight = Math.max(MIN_BOUNDS_SIZE, finiteOr(height, MIN_BOUNDS_SIZE));
  return {
    contentBounds: bounds,
    scaleX: safeWidth / bounds.width,
    scaleY: safeHeight / bounds.height,
    offset: { x: 0, y: 0 },
  };
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

export function worldToMinimapPoint(point: MinimapPoint, transform: MinimapTransform): MinimapPoint {
  const content = normalizeBounds(transform.contentBounds);
  return {
    x: transform.offset.x + (finiteOr(point.x, 0) - content.x) * transform.scaleX,
    y: transform.offset.y + (finiteOr(point.y, 0) - content.y) * transform.scaleY,
  };
}

export function minimapToWorldPoint(point: MinimapPoint, transform: MinimapTransform): MinimapPoint {
  const content = normalizeBounds(transform.contentBounds);
  return {
    x: content.x + (finiteOr(point.x, 0) - transform.offset.x) / transform.scaleX,
    y: content.y + (finiteOr(point.y, 0) - transform.offset.y) / transform.scaleY,
  };
}

export function worldToMinimapRect(bounds: MinimapBounds, transform: MinimapTransform): MinimapBounds {
  const origin = worldToMinimapPoint(bounds, transform);
  return {
    x: origin.x,
    y: origin.y,
    width: Math.max(0, finiteOr(bounds.width, 0)) * transform.scaleX,
    height: Math.max(0, finiteOr(bounds.height, 0)) * transform.scaleY,
  };
}

export function minimapDeltaToWorldDelta(delta: MinimapPoint, transform: MinimapTransform): MinimapPoint {
  return {
    x: finiteOr(delta.x, 0) / transform.scaleX,
    y: finiteOr(delta.y, 0) / transform.scaleY,
  };
}

export function findClosestNodeToPoint(
  nodes: readonly MinimapNode[],
  point: MinimapPoint,
): MinimapNode | null {
  let closest: MinimapNode | null = null;
  let closestDistance = Number.POSITIVE_INFINITY;
  for (const node of nodes) {
    const left = finiteOr(node.x, 0);
    const top = finiteOr(node.y, 0);
    const right = left + Math.max(0, finiteOr(node.width, 0));
    const bottom = top + Math.max(0, finiteOr(node.height, 0));
    const dx = Math.max(left - point.x, 0, point.x - right);
    const dy = Math.max(top - point.y, 0, point.y - bottom);
    const distance = dx * dx + dy * dy;
    if (distance < closestDistance) {
      closest = node;
      closestDistance = distance;
    }
  }
  return closest;
}
