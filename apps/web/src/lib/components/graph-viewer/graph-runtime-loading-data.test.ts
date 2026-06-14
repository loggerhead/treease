import { describe, expect, it } from 'vitest';
import {
  GRAPH_RUNTIME_LOADING_EDGES,
  GRAPH_RUNTIME_LOADING_NODE_BARS,
  GRAPH_RUNTIME_LOADING_NODES,
  getGraphRuntimeLoadingBounds,
  getGraphRuntimeLoadingViewBox,
} from './graph-runtime-loading-data';

describe('graph-runtime-loading-data', () => {
  it('keeps the sample-json graph layout frozen for the loading skeleton', () => {
    expect(GRAPH_RUNTIME_LOADING_NODES).toEqual([
      { x: 0, y: 0, width: 248, height: 74 },
      { x: 308, y: 0, width: 154, height: 110 },
      { x: 308, y: 170, width: 110, height: 56 },
      { x: 308, y: 286, width: 242, height: 84 },
      { x: 308, y: 430, width: 592, height: 128 },
      { x: 960, y: 430, width: 556, height: 38 },
    ]);

    expect(GRAPH_RUNTIME_LOADING_EDGES).toEqual([
      { fromX: 248, fromY: 10, c1x: 288, c1y: 10, c2x: 268, c2y: 10, toX: 308, toY: 10 },
      { fromX: 248, fromY: 28, c1x: 288, c1y: 28, c2x: 268, c2y: 180, toX: 308, toY: 180 },
      { fromX: 248, fromY: 46, c1x: 288, c1y: 46, c2x: 268, c2y: 300, toX: 308, toY: 300 },
      { fromX: 900, fromY: 548, c1x: 940, c1y: 548, c2x: 920, c2y: 440, toX: 960, toY: 440 },
      { fromX: 248, fromY: 64, c1x: 288, c1y: 64, c2x: 268, c2y: 440, toX: 308, toY: 440 },
    ]);
  });

  it('derives a padded viewBox from the frozen layout', () => {
    expect(getGraphRuntimeLoadingBounds()).toEqual({ minX: 0, minY: 0, maxX: 1516, maxY: 558 });
    expect(getGraphRuntimeLoadingViewBox()).toBe('-24 -24 1564 606');
  });

  it('keeps paired key-value rows frozen for medium nodes', () => {
    expect(GRAPH_RUNTIME_LOADING_NODE_BARS[0]).toEqual([
      { x: 22, y: 24, width: 48, height: 8, role: 'key' },
      { x: 100, y: 24, width: 108, height: 8, role: 'value' },
      { x: 22, y: 42, width: 33, height: 8, role: 'key' },
      { x: 100, y: 42, width: 91, height: 8, role: 'value' },
    ]);
  });

  it('keeps expanded key-value rows frozen for large nodes', () => {
    expect(GRAPH_RUNTIME_LOADING_NODE_BARS[4]).toEqual([
      { x: 340, y: 449, width: 121, height: 12, role: 'key' },
      { x: 515, y: 449, width: 318, height: 12, role: 'value' },
      { x: 340, y: 475, width: 86, height: 12, role: 'key' },
      { x: 515, y: 475, width: 268, height: 12, role: 'value' },
      { x: 340, y: 501, width: 130, height: 12, role: 'key' },
      { x: 515, y: 501, width: 240, height: 12, role: 'value' },
      { x: 340, y: 527, width: 95, height: 12, role: 'key' },
      { x: 515, y: 527, width: 297, height: 12, role: 'value' },
    ]);
  });

  it('keeps compact single-column rows frozen for small nodes', () => {
    expect(GRAPH_RUNTIME_LOADING_NODE_BARS[2]).toEqual([
      { x: 345, y: 185, width: 36, height: 9, role: 'single' },
      { x: 339, y: 202, width: 48, height: 9, role: 'single' },
    ]);
  });
});
