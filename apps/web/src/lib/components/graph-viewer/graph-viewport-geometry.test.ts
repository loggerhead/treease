import { describe, expect, it } from 'vitest';
import { clampPanOffsetToGraphBounds } from './graph-viewport-geometry';

describe('clampPanOffsetToGraphBounds', () => {
  it('clamps drag offsets to graph bounds plus padding when content exceeds viewport', () => {
    const result = clampPanOffsetToGraphBounds(
      {
        viewportWidth: 400,
        viewportHeight: 300,
        scaleX: 1,
        scaleY: 1,
        offsetX: 999,
        offsetY: -999,
      },
      {
        left: 100,
        top: 80,
        right: 700,
        bottom: 580,
      },
      100,
    );

    expect(result).toEqual({
      x: 0,
      y: -380,
    });
  });

  it('centers smaller content instead of allowing drag jitter', () => {
    const result = clampPanOffsetToGraphBounds(
      {
        viewportWidth: 600,
        viewportHeight: 500,
        scaleX: 1,
        scaleY: 1,
        offsetX: 30,
        offsetY: -50,
      },
      {
        left: 100,
        top: 120,
        right: 320,
        bottom: 300,
      },
      100,
    );

    expect(result).toEqual({
      x: 90,
      y: 40,
    });
  });
});
