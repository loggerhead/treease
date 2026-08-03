import { describe, expect, it, vi } from 'vitest';
import { createGraphViewportController } from './viewport-controller';

function createController(zoom = vi.fn()) {
  const updateRenderableProjection = vi.fn();
  const updateViewportOverlays = vi.fn();
  return {
    controller: createGraphViewportController({
      getContainer: () => null,
      getLeafer: () => ({ zoom, zoomLayer: {} }),
      getSuppressGraphPointerUntil: () => 0,
      getMoveEventName: () => undefined,
      getZoomEventName: () => undefined,
      bindPointerClick: () => {},
      updateRenderableProjection,
      updateViewportOverlays,
    }),
    zoom,
    updateRenderableProjection,
    updateViewportOverlays,
  };
}

describe('graph viewport reveal', () => {
  it('uses Leafer view control to animate a cell reveal and syncs each viewport update', () => {
    const { controller, zoom, updateRenderableProjection, updateViewportOverlays } = createController();
    const box = { width: 120, height: 32 };

    expect(controller.centerOnBox(box)).toBe(true);
    expect(zoom).toHaveBeenCalledWith(
      box,
      0,
      true,
      expect.objectContaining({ duration: 0.25 }),
    );
    const transition = zoom.mock.calls[0]?.[3] as { event: { update: () => void; completed: () => void } };
    transition.event.update();
    transition.event.completed();
    expect(updateRenderableProjection).toHaveBeenCalledTimes(2);
    expect(updateViewportOverlays).toHaveBeenCalledTimes(2);
  });

  it('captures and restores the viewport transform', () => {
    const layer = { x: 12, y: -8, scaleX: 1.5, scaleY: 1.5 };
    const update = vi.fn();
    const controller = createGraphViewportController({
      getContainer: () => null,
      getLeafer: () => ({ zoomLayer: layer, update }),
      getSuppressGraphPointerUntil: () => 0,
      getMoveEventName: () => undefined,
      getZoomEventName: () => undefined,
      bindPointerClick: () => {},
      updateViewportOverlays: () => {},
    });

    expect(controller.getViewportState()).toEqual({ x: 12, y: -8, scaleX: 1.5, scaleY: 1.5 });
    layer.x = 0;
    layer.y = 0;
    layer.scaleX = 1;
    layer.scaleY = 1;

    controller.restoreViewportState({ x: 12, y: -8, scaleX: 1.5, scaleY: 1.5 });

    expect(layer).toEqual({ x: 12, y: -8, scaleX: 1.5, scaleY: 1.5 });
    expect(update).toHaveBeenCalledOnce();
  });

  it('uses the node bounds when a cell box is unavailable', () => {
    const { controller, zoom, updateRenderableProjection } = createController();
    const node = { boxArgs: { x: 40, y: 80, width: 200, height: 120 } };

    controller.centerOnNode(node as never);
    expect(zoom).toHaveBeenCalledWith(
      node.boxArgs,
      0,
      true,
      expect.objectContaining({ duration: 0.25 }),
    );
    const transition = zoom.mock.calls[0]?.[3] as { event: { update: () => void; completed: () => void } };
    transition.event.completed();
    expect(updateRenderableProjection).toHaveBeenCalledTimes(1);
  });
});
