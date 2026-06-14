import type { LeaferAppLike, LeaferBox } from './model';

export type GraphGeometryRect = {
  left: number;
  top: number;
  width: number;
  height: number;
};

export type GraphGeometryPoint = {
  x: number;
  y: number;
};

type WorldPointCapableBox = LeaferBox & {
  getWorldPointByBox?: (point: { x: number; y: number }) => { x?: number; y?: number } | null;
};

export function getWorldRectFromBoxLike(box: LeaferBox): GraphGeometryRect | null {
  let current: LeaferBox | null | undefined = box;
  let localX = 0;
  let localY = 0;
  const width = Number(box.width ?? 0);
  const height = Number(box.height ?? 0);
  while (current) {
    const worldBox = current as WorldPointCapableBox;
    if (typeof worldBox.getWorldPointByBox === 'function') {
      const topLeftWorld = worldBox.getWorldPointByBox({ x: localX, y: localY });
      const bottomRightWorld = worldBox.getWorldPointByBox({ x: localX + width, y: localY + height });
      if (!topLeftWorld || !bottomRightWorld) return null;
      return {
        left: Number(topLeftWorld.x ?? 0),
        top: Number(topLeftWorld.y ?? 0),
        width: Number(bottomRightWorld.x ?? 0) - Number(topLeftWorld.x ?? 0),
        height: Number(bottomRightWorld.y ?? 0) - Number(topLeftWorld.y ?? 0),
      };
    }
    localX += Number(current.x ?? 0);
    localY += Number(current.y ?? 0);
    current = (current.parent ?? null) as LeaferBox | null;
  }
  return null;
}

export function getClientRectFromWorldRect(
  worldRect: GraphGeometryRect | null,
  app: LeaferAppLike | null,
): GraphGeometryRect | null {
  if (!worldRect || typeof app?.getClientPointByWorld !== 'function') return null;
  app.updateClientBounds?.();
  const topLeftClient = app.getClientPointByWorld({ x: worldRect.left, y: worldRect.top });
  const bottomRightClient = app.getClientPointByWorld({
    x: worldRect.left + worldRect.width,
    y: worldRect.top + worldRect.height,
  });
  const left = Number(topLeftClient?.x);
  const top = Number(topLeftClient?.y);
  const right = Number(bottomRightClient?.x);
  const bottom = Number(bottomRightClient?.y);
  if (![left, top, right, bottom].every((value) => Number.isFinite(value))) return null;
  return {
    left,
    top,
    width: right - left,
    height: bottom - top,
  };
}

export function getClientRectFromBoxLike(box: LeaferBox, app: LeaferAppLike | null): GraphGeometryRect | null {
  return getClientRectFromWorldRect(getWorldRectFromBoxLike(box), app);
}

export function getClientProbeCoordFromBoxLike(
  box: LeaferBox,
  app: LeaferAppLike | null,
): GraphGeometryPoint | null {
  const worldBox = box as WorldPointCapableBox;
  if (typeof worldBox.getWorldPointByBox === 'function' && typeof app?.getClientPointByWorld === 'function') {
    app.updateClientBounds?.();
    const worldPoint = worldBox.getWorldPointByBox({
      x: Number(box.width ?? 0) / 2,
      y: Number(box.height ?? 0) / 2,
    });
    const clientPoint = worldPoint
      ? app.getClientPointByWorld({ x: Number(worldPoint.x ?? 0), y: Number(worldPoint.y ?? 0) })
      : null;
    const x = Number(clientPoint?.x);
    const y = Number(clientPoint?.y);
    if (Number.isFinite(x) && Number.isFinite(y)) {
      return { x: Math.round(x), y: Math.round(y) };
    }
  }
  const rect = getClientRectFromBoxLike(box, app);
  if (!rect) return null;
  return {
    x: Math.round(rect.left + rect.width / 2),
    y: Math.round(rect.top + rect.height / 2),
  };
}
