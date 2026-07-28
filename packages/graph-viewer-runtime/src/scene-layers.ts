export type GraphViewerLayers<T = any> = {
  edgeLayer: T | null;
  nodeLayer: T | null;
  overlayLayer: T | null;
};

/** Creates the canonical edge/node/overlay stack below one Leafer render root. */
export function ensureGraphViewerLayers<T>(options: {
  root: { add: (target: T) => void } | null;
  BoxCtor: (new (config: Record<string, unknown>) => T) | null;
  layers: GraphViewerLayers<T>;
}): Partial<GraphViewerLayers<T>> {
  const { root, BoxCtor, layers } = options;
  if (!root || !BoxCtor) return {};
  const next: Partial<GraphViewerLayers<T>> = {};
  if (!layers.edgeLayer) {
    next.edgeLayer = new BoxCtor({ x: 0, y: 0, width: 0, height: 0, fill: 'transparent' });
    root.add(next.edgeLayer);
  }
  if (!layers.nodeLayer) {
    next.nodeLayer = new BoxCtor({ x: 0, y: 0, width: 0, height: 0, fill: 'transparent' });
    root.add(next.nodeLayer);
  }
  if (!layers.overlayLayer) {
    next.overlayLayer = new BoxCtor({ x: 0, y: 0, width: 0, height: 0, fill: 'transparent', hittable: false, hitChildren: false });
    root.add(next.overlayLayer);
  }
  return next;
}
