import { describe, expect, it } from 'vitest';
import {
  clampViewportToContent,
  computeContentBounds,
  computeMinimapScale,
  getViewportWorldBounds,
  minimapDeltaToWorldDelta,
  worldToMinimapRect,
} from './geometry';

describe('leafer minimap geometry', () => {
  it('falls back to a safe empty content bounds', () => {
    expect(computeContentBounds([])).toEqual({ x: 0, y: 0, width: 1, height: 1 });
  });

  it('computes padded bounds for all graph nodes', () => {
    expect(
      computeContentBounds(
        [
          { id: 1, x: 10, y: 20, width: 100, height: 60 },
          { id: 2, x: -30, y: 80, width: 50, height: 40 },
        ],
        10,
      ),
    ).toEqual({ x: -40, y: 10, width: 160, height: 120 });
  });

  it('computes viewport bounds from zoom layer offsets and scale', () => {
    const container = {
      getBoundingClientRect: () => ({ width: 800, height: 600 }),
    };

    expect(getViewportWorldBounds(container, { x: -200, y: -100, scaleX: 2, scaleY: 2 })).toEqual({
      x: 100,
      y: 50,
      width: 400,
      height: 300,
    });
  });

  it('projects world viewport bounds into minimap coordinates', () => {
    const content = { x: 0, y: 0, width: 1000, height: 500 };
    const scale = computeMinimapScale(content, 200, 100);

    expect(worldToMinimapRect({ x: 100, y: 50, width: 400, height: 200 }, content, scale)).toEqual({
      x: 20,
      y: 10,
      width: 80,
      height: 40,
    });
  });

  it('converts minimap drag delta back to world delta', () => {
    expect(minimapDeltaToWorldDelta({ x: 20, y: -10 }, 0.5)).toEqual({ x: 40, y: -20 });
  });

  it('clamps viewport without producing invalid values when content is smaller than view', () => {
    expect(clampViewportToContent({ x: 20, y: 20, width: 200, height: 100 }, { x: 0, y: 0, width: 100, height: 50 })).toEqual({
      x: -50,
      y: -25,
      width: 200,
      height: 100,
    });
  });
});
