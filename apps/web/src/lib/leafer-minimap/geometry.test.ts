import { describe, expect, it } from 'vitest';
import {
  computeContentBounds,
  createMinimapTransform,
  findClosestNodeToPoint,
  getViewportWorldBounds,
  minimapToWorldPoint,
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
    const transform = createMinimapTransform(content, 200, 100);

    expect(worldToMinimapRect({ x: 100, y: 50, width: 400, height: 200 }, transform)).toEqual({
      x: 20,
      y: 10,
      width: 80,
      height: 40,
    });
  });

  it('converts minimap drag delta back to world delta', () => {
    const transform = createMinimapTransform({ x: 0, y: 0, width: 400, height: 400 }, 200, 100);

    expect(minimapDeltaToWorldDelta({ x: 20, y: -10 }, transform)).toEqual({ x: 40, y: -40 });
  });

  it('preserves both dimensions and reverses pointer coordinates for an extremely tall graph', () => {
    const content = { x: -24, y: -53844.143, width: 7144, height: 1154845.4 };
    const transform = createMinimapTransform(content, 220, 150);
    const projected = worldToMinimapRect(content, transform);

    expect(projected).toEqual({ x: 0, y: 0, width: 220, height: 150 });
    expect(minimapToWorldPoint({ x: 111.5234, y: 113.5391 }, transform)).toEqual({
      x: 3597.4689527272726,
      y: 820289.9060342666,
    });
  });

  it('resolves a minimap click to the closest graph node', () => {
    const nodes = [
      { id: 1, x: 0, y: 0, width: 100, height: 100 },
      { id: 2, x: 700, y: 800, width: 100, height: 100 },
    ];

    expect(findClosestNodeToPoint(nodes, { x: 650, y: 700 })).toEqual(nodes[1]);
  });
});
