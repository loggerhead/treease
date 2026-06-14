export type GraphDirtyRegionRect = {
  left: number;
  top: number;
  width: number;
  height: number;
};

type LeaferForceRenderTarget = {
  forceRender?: (bounds?: GraphDirtyRegionRect, sync?: boolean) => void;
};

function normalizeRect(rect: GraphDirtyRegionRect | null | undefined): GraphDirtyRegionRect | null {
  if (!rect) return null;
  const left = Number(rect.left);
  const top = Number(rect.top);
  const width = Number(rect.width);
  const height = Number(rect.height);
  if (![left, top, width, height].every((value) => Number.isFinite(value))) return null;
  if (width < 0 || height < 0) return null;
  return { left, top, width, height };
}

function unionRects(a: GraphDirtyRegionRect, b: GraphDirtyRegionRect): GraphDirtyRegionRect {
  const left = Math.min(a.left, b.left);
  const top = Math.min(a.top, b.top);
  const right = Math.max(a.left + a.width, b.left + b.width);
  const bottom = Math.max(a.top + a.height, b.top + b.height);
  return {
    left,
    top,
    width: right - left,
    height: bottom - top,
  };
}

export function createGraphDirtyRegion() {
  let current: GraphDirtyRegionRect | null = null;

  function reset(): void {
    current = null;
  }

  function mark(rect: GraphDirtyRegionRect | null | undefined): GraphDirtyRegionRect | null {
    const normalized = normalizeRect(rect);
    if (!normalized) return current;
    current = current ? unionRects(current, normalized) : normalized;
    return current;
  }

  function flush(target?: LeaferForceRenderTarget | null, sync = false): GraphDirtyRegionRect | null {
    const next = current;
    current = null;
    if (next) {
      target?.forceRender?.(next, sync);
    }
    return next;
  }

  function getCurrent(): GraphDirtyRegionRect | null {
    return current;
  }

  return {
    reset,
    mark,
    flush,
    getCurrent,
  };
}
