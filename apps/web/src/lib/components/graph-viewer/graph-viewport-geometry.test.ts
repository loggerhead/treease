import { describe, expect, it } from 'vitest';
import { GRAPH_PAN_CONSTRAINT_PADDING, clampPanOffsetToGraphBounds } from '@treease/graph-viewer-runtime';

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
      GRAPH_PAN_CONSTRAINT_PADDING,
    );

    expect(result).toEqual({
      x: 400,
      y: -780,
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
      GRAPH_PAN_CONSTRAINT_PADDING,
    );

    expect(result).toEqual({
      x: 30,
      y: -50,
    });
  });
});
