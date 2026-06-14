import { describe, expect, it, vi } from 'vitest';
import { createGraphDirtyRegion } from './graph-dirty-region';

describe('graph-dirty-region', () => {
  it('unions marked rects and flushes once', () => {
    const forceRender = vi.fn();
    const region = createGraphDirtyRegion();

    region.mark({ left: 10, top: 20, width: 30, height: 40 });
    region.mark({ left: 25, top: 5, width: 50, height: 25 });

    expect(region.getCurrent()).toEqual({
      left: 10,
      top: 5,
      width: 65,
      height: 55,
    });

    expect(region.flush({ forceRender }, false)).toEqual({
      left: 10,
      top: 5,
      width: 65,
      height: 55,
    });
    expect(forceRender).toHaveBeenCalledWith(
      {
        left: 10,
        top: 5,
        width: 65,
        height: 55,
      },
      false,
    );
    expect(region.getCurrent()).toBeNull();
  });

  it('ignores invalid rectangles', () => {
    const region = createGraphDirtyRegion();

    region.mark({ left: 0, top: 0, width: -1, height: 10 });
    region.mark({ left: Number.NaN, top: 0, width: 10, height: 10 });

    expect(region.getCurrent()).toBeNull();
    expect(region.flush()).toBeNull();
  });
});
