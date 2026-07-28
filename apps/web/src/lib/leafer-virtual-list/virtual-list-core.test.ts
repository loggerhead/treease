import { describe, expect, it, vi } from 'vitest';
import { FixedSizeVirtualList } from '@treease/graph-viewer-runtime';

describe('FixedSizeVirtualList', () => {
  it('computes the visible range with overscan', () => {
    const list = new FixedSizeVirtualList({
      count: 100,
      itemSize: 20,
      viewportSize: 80,
      overscan: 1,
    });

    expect(list.getRange()).toEqual({ start: 0, end: 5 });
    expect(list.getVirtualItems().map((item) => item.index)).toEqual([0, 1, 2, 3, 4]);
  });

  it('clamps scroll offset and recomputes the range', () => {
    const onChange = vi.fn();
    const list = new FixedSizeVirtualList({
      count: 10,
      itemSize: 20,
      viewportSize: 80,
      overscan: 2,
      onChange,
    });

    list.scrollToOffset(60);
    expect(list.getScrollOffset()).toBe(60);
    expect(list.getRange()).toEqual({ start: 3, end: 9 });

    list.scrollToOffset(999);
    expect(list.getScrollOffset()).toBe(120);
    expect(list.getRange()).toEqual({ start: 6, end: 10 });
    expect(onChange).toHaveBeenCalled();
  });

  it('supports scrollToIndex align modes', () => {
    const list = new FixedSizeVirtualList({
      count: 50,
      itemSize: 20,
      viewportSize: 100,
      overscan: 0,
    });

    list.scrollToIndex(10, 'start');
    expect(list.getScrollOffset()).toBe(200);

    list.scrollToIndex(10, 'end');
    expect(list.getScrollOffset()).toBe(120);

    list.scrollToIndex(10, 'center');
    expect(list.getScrollOffset()).toBe(160);
  });

  it('keeps scroll offset legal when viewport or count changes', () => {
    const list = new FixedSizeVirtualList({
      count: 20,
      itemSize: 20,
      viewportSize: 80,
      overscan: 1,
      scrollOffset: 200,
    });

    list.setViewportSize(160);
    expect(list.getScrollOffset()).toBe(200);

    list.setCount(8);
    expect(list.getScrollOffset()).toBe(0);
    expect(list.getRange()).toEqual({ start: 0, end: 8 });
  });

  it('commits multiple option changes in one notification', () => {
    const onChange = vi.fn();
    const list = new FixedSizeVirtualList({
      count: 100,
      itemSize: 20,
      viewportSize: 80,
      overscan: 1,
      scrollOffset: 40,
      onChange,
    });

    list.setOptions({
      count: 120,
      itemSize: 20,
      viewportSize: 100,
      overscan: 2,
      scrollOffset: 60,
    });

    expect(list.getScrollOffset()).toBe(60);
    expect(list.getRange()).toEqual({ start: 3, end: 10 });
    expect(onChange).toHaveBeenCalledTimes(1);
  });
});
