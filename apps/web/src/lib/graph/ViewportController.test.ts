// 职责：graph-viewport-geometry 视口几何纯函数的单元测试
import { describe, expect, it, vi } from 'vitest';
import {
  applyZoom,
  centerOnNode,
  doesBoxIntersectBounds,
  getViewportBounds,
  getViewportCenter,
  getZoomScale,
  isPointInBounds,
  updateSize,
  type ViewportState,
} from '../components/graph-viewer/graph-viewport-geometry';
import type { ViewportBounds } from '../components/graph-viewer/graph-viewport-geometry';

describe('ViewportController', () => {
  describe('getViewportCenter', () => {
    it('returns 0,0 when container is null', () => {
      expect(getViewportCenter(null)).toEqual({ x: 0, y: 0 });
    });

    it('returns center of container rect', () => {
      const container = {
        getBoundingClientRect: () => ({ width: 800, height: 600 }),
      } as HTMLElement;
      expect(getViewportCenter(container)).toEqual({ x: 400, y: 300 });
    });
  });

  describe('getZoomScale', () => {
    it('returns scale from direct properties', () => {
      const layer = { scaleX: 2, scaleY: 3 };
      expect(getZoomScale(layer)).toEqual({ scaleX: 2, scaleY: 3 });
    });

    it('returns scale from __ properties', () => {
      const layer = { __: { scaleX: 1.5, scaleY: 1.5 } };
      expect(getZoomScale(layer)).toEqual({ scaleX: 1.5, scaleY: 1.5 });
    });

    it('defaults to 1 when no scale info', () => {
      expect(getZoomScale(null)).toEqual({ scaleX: 1, scaleY: 1 });
      expect(getZoomScale({})).toEqual({ scaleX: 1, scaleY: 1 });
    });
  });

  describe('getViewportBounds', () => {
    it('returns null when container is null', () => {
      expect(getViewportBounds(null, null)).toBeNull();
    });

    it('returns null when leafer has no zoomLayer', () => {
      const container = { getBoundingClientRect: () => ({ width: 100, height: 100 }) } as any;
      expect(getViewportBounds(container, {} as any)).toBeNull();
    });

    it('returns null when scale is zero or negative', () => {
      const container = { getBoundingClientRect: () => ({ width: 100, height: 100 }) } as any;
      const leafer = { zoomLayer: { scaleX: 0, scaleY: 0, x: 0, y: 0 } } as any;
      const result = getViewportBounds(container, leafer);
      expect(result).toBeNull();
      expect(leafer.zoomLayer.scaleX).toBe(0);
      expect(leafer.zoomLayer.scaleY).toBe(0);
      expect(leafer.zoomLayer.x).toBe(0);
      expect(leafer.zoomLayer.y).toBe(0);
    });

    it('returns null when scale is not finite', () => {
      const container = { getBoundingClientRect: () => ({ width: 100, height: 100 }) } as any;
      const leafer = { zoomLayer: { scaleX: Infinity, scaleY: 1, x: 5, y: 10 } } as any;
      const result = getViewportBounds(container, leafer);
      expect(result).toBeNull();
      expect(leafer.zoomLayer.x).toBe(5);
      expect(leafer.zoomLayer.y).toBe(10);
    });

    it('computes correct bounds at scale 1 with no offset', () => {
      const container = { getBoundingClientRect: () => ({ width: 500, height: 400 }) } as any;
      const leafer = { zoomLayer: { scaleX: 1, scaleY: 1, x: 0, y: 0 } } as any;
      const bounds = getViewportBounds(container, leafer);
      expect(bounds).toEqual({ left: 0, right: 500, top: 0, bottom: 400 });
    });

    it('computes correct bounds with offset and scale', () => {
      const container = { getBoundingClientRect: () => ({ width: 500, height: 400 }) } as any;
      const leafer = { zoomLayer: { scaleX: 2, scaleY: 2, x: -100, y: -50 } } as any;
      const bounds = getViewportBounds(container, leafer);
      expect(bounds).not.toBeNull();
      expect(bounds!.left).toBeCloseTo(50);
      expect(bounds!.right).toBeCloseTo(300);
      expect(bounds!.top).toBeCloseTo(25);
      expect(bounds!.bottom).toBeCloseTo(225);
    });
  });

  describe('isPointInBounds', () => {
    const bounds: ViewportBounds = { left: 0, right: 100, top: 0, bottom: 100 };

    it('returns true for point inside bounds', () => {
      expect(isPointInBounds({ x: 50, y: 50 }, bounds)).toBe(true);
    });

    it('returns true for point on boundary', () => {
      expect(isPointInBounds({ x: 0, y: 0 }, bounds)).toBe(true);
      expect(isPointInBounds({ x: 100, y: 100 }, bounds)).toBe(true);
    });

    it('returns false for point outside', () => {
      expect(isPointInBounds({ x: -1, y: 50 }, bounds)).toBe(false);
      expect(isPointInBounds({ x: 50, y: 101 }, bounds)).toBe(false);
    });
  });

  describe('doesBoxIntersectBounds', () => {
    const bounds: ViewportBounds = { left: 10, right: 110, top: 10, bottom: 110 };

    it('returns true for overlapping box', () => {
      expect(doesBoxIntersectBounds(
        { x: 50, y: 50, width: 20, height: 20, cornerRadius: 0 },
        bounds,
      )).toBe(true);
    });

    it('returns true for box containing bounds', () => {
      expect(doesBoxIntersectBounds(
        { x: 0, y: 0, width: 200, height: 200, cornerRadius: 0 },
        bounds,
      )).toBe(true);
    });

    it('returns false for box entirely to the left', () => {
      expect(doesBoxIntersectBounds(
        { x: 0, y: 50, width: 5, height: 10, cornerRadius: 0 },
        bounds,
      )).toBe(false);
    });

    it('returns false for box entirely to the right', () => {
      expect(doesBoxIntersectBounds(
        { x: 200, y: 50, width: 10, height: 10, cornerRadius: 0 },
        bounds,
      )).toBe(false);
    });

    it('returns false for box entirely above', () => {
      expect(doesBoxIntersectBounds(
        { x: 50, y: 0, width: 10, height: 5, cornerRadius: 0 },
        bounds,
      )).toBe(false);
    });

    it('returns false for box entirely below', () => {
      expect(doesBoxIntersectBounds(
        { x: 50, y: 200, width: 10, height: 10, cornerRadius: 0 },
        bounds,
      )).toBe(false);
    });

    it('returns true for partially overlapping box', () => {
      expect(doesBoxIntersectBounds(
        { x: 5, y: 5, width: 10, height: 10, cornerRadius: 0 },
        bounds,
      )).toBe(true);
    });
  });

  describe('applyZoom', () => {
    it('does nothing when leafer has no zoomLayer', () => {
      const state: ViewportState = {
        leafer: {} as any,
        container: {} as any,
        edgeLayer: null,
        nodeLayer: null,
        lastAutoOffset: null,
      };
      applyZoom(state, 1.5);
      expect(state.lastAutoOffset).toBeNull();
      expect((state.leafer as any).zoomLayer).toBeUndefined();
    });

    it('does nothing when container is null', () => {
      const layer = { scaleX: 1, scaleY: 1, x: 0, y: 0 };
      const state: ViewportState = {
        leafer: { zoomLayer: layer } as any,
        container: null,
        edgeLayer: null,
        nodeLayer: null,
        lastAutoOffset: null,
      };
      applyZoom(state, 1.5);
      expect(layer.scaleX).toBe(1);
      expect(layer.scaleY).toBe(1);
    });

    it('applies zoom transform correctly', () => {
      const layer = { scaleX: 1, scaleY: 1, x: 0, y: 0 };
      const update = vi.fn();
      const state: ViewportState = {
        leafer: { zoomLayer: layer, update } as any,
        container: { getBoundingClientRect: () => ({ width: 400, height: 300 }) } as any,
        edgeLayer: null,
        nodeLayer: null,
        lastAutoOffset: null,
      };
      applyZoom(state, 2);
      expect(layer.scaleX).toBe(2);
      expect(layer.scaleY).toBe(2);
      expect(update).toHaveBeenCalled();
    });

    it('uses getValidScale callback when provided', () => {
      const layer = { scaleX: 1, scaleY: 1, x: 0, y: 0 };
      const update = vi.fn();
      const state: ViewportState = {
        leafer: { zoomLayer: layer, update } as any,
        container: { getBoundingClientRect: () => ({ width: 400, height: 300 }) } as any,
        edgeLayer: null,
        nodeLayer: null,
        lastAutoOffset: null,
      };
      applyZoom(state, 3, (s) => Math.min(s, 2));
      expect(layer.scaleX).toBe(2);
    });

    it('skips when resulting scale is not finite', () => {
      const layer = { scaleX: 1, scaleY: 1, x: 5, y: 10 };
      const state: ViewportState = {
        leafer: { zoomLayer: layer } as any,
        container: { getBoundingClientRect: () => ({ width: 400, height: 300 }) } as any,
        edgeLayer: null,
        nodeLayer: null,
        lastAutoOffset: null,
      };
      applyZoom(state, Infinity);
      expect(layer.scaleX).toBe(1);
      expect(layer.scaleY).toBe(1);
      expect(layer.x).toBe(5);
      expect(layer.y).toBe(10);
    });
  });

  describe('updateSize', () => {
    it('does nothing when container is null', () => {
      const resize = vi.fn();
      const state: ViewportState = {
        leafer: { resize } as any,
        container: null,
        edgeLayer: null,
        nodeLayer: null,
        lastAutoOffset: null,
      };
      updateSize(state);
      expect(resize).not.toHaveBeenCalled();
    });

    it('does nothing when leafer is null', () => {
      const state: ViewportState = {
        leafer: null,
        container: {} as any,
        edgeLayer: null,
        nodeLayer: null,
        lastAutoOffset: null,
      };
      updateSize(state);
      expect(state.leafer).toBeNull();
    });

    it('calls leafer.resize with container dimensions', () => {
      const resize = vi.fn();
      const state: ViewportState = {
        leafer: { resize } as any,
        container: { getBoundingClientRect: () => ({ width: 800, height: 600 }) } as any,
        edgeLayer: null,
        nodeLayer: null,
        lastAutoOffset: null,
      };
      updateSize(state);
      expect(resize).toHaveBeenCalledWith({ width: 800, height: 600 });
    });

    it('skips resize when dimensions are zero', () => {
      const resize = vi.fn();
      const state: ViewportState = {
        leafer: { resize } as any,
        container: { getBoundingClientRect: () => ({ width: 0, height: 0 }) } as any,
        edgeLayer: null,
        nodeLayer: null,
        lastAutoOffset: null,
      };
      updateSize(state);
      expect(resize).not.toHaveBeenCalled();
    });
  });

  describe('centerOnNode', () => {
    it('does nothing when leafer has no zoomLayer', () => {
      const state: ViewportState = {
        leafer: {} as any,
        container: {} as any,
        edgeLayer: null,
        nodeLayer: null,
        lastAutoOffset: null,
      };
      const node = { boxArgs: { x: 100, y: 100, width: 50, height: 50, cornerRadius: 4 } } as any;
      centerOnNode(state, node);
      expect(state.lastAutoOffset).toBeNull();
    });

    it('centers the canvas on the given node with correct offset', () => {
      const layer = { scaleX: 1, scaleY: 1, x: 0, y: 0 };
      const update = vi.fn();
      const state: ViewportState = {
        leafer: { zoomLayer: layer, update } as any,
        container: { getBoundingClientRect: () => ({ width: 800, height: 600 }) } as any,
        edgeLayer: null,
        nodeLayer: null,
        lastAutoOffset: null,
      };
      const node = { boxArgs: { x: 100, y: 200, width: 50, height: 50, cornerRadius: 4 } } as any;
      centerOnNode(state, node);
      expect(layer.x).toBeCloseTo(275);
      expect(layer.y).toBeCloseTo(75);
      expect(state.lastAutoOffset).toEqual({ x: 275, y: 75 });
      expect(update).toHaveBeenCalled();
    });
  });
});
