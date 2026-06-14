import type { VirtualListAlign, VirtualListItem, VirtualListOptions, VirtualListRange, VirtualListState, VirtualListUpdateOptions } from './types';

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

function normalizeCount(count: number): number {
  return Number.isFinite(count) ? Math.max(0, Math.floor(count)) : 0;
}

function normalizeSize(value: number): number {
  return Number.isFinite(value) ? Math.max(0, value) : 0;
}

function buildState(
  count: number,
  itemSize: number,
  viewportSize: number,
  overscan: number,
  scrollOffset: number,
): VirtualListState {
  const safeCount = normalizeCount(count);
  const safeItemSize = normalizeSize(itemSize);
  const safeViewportSize = normalizeSize(viewportSize);
  const safeOverscan = normalizeCount(overscan);
  const totalSize = safeCount * safeItemSize;
  const maxScrollOffset = Math.max(0, totalSize - safeViewportSize);
  const safeScrollOffset = clamp(scrollOffset, 0, maxScrollOffset);
  const visibleCount =
    safeCount === 0 || safeItemSize <= 0 || safeViewportSize <= 0 ? 0 : Math.ceil(safeViewportSize / safeItemSize);
  const baseStart = safeItemSize > 0 ? Math.floor(safeScrollOffset / safeItemSize) : 0;
  const start = Math.min(safeCount, Math.max(0, baseStart));
  const end = Math.min(safeCount, Math.max(start, start + visibleCount + safeOverscan));
  const items: VirtualListItem[] = [];

  for (let index = start; index < end; index += 1) {
    const itemStart = index * safeItemSize;
    items.push({
      index,
      key: index,
      size: safeItemSize,
      start: itemStart,
      end: itemStart + safeItemSize,
    });
  }

  return {
    count: safeCount,
    itemSize: safeItemSize,
    viewportSize: safeViewportSize,
    overscan: safeOverscan,
    scrollOffset: safeScrollOffset,
    totalSize,
    maxScrollOffset,
    range: { start, end },
    items,
  };
}

function sameRange(left: VirtualListRange, right: VirtualListRange): boolean {
  return left.start === right.start && left.end === right.end;
}

export class FixedSizeVirtualList {
  private state: VirtualListState;
  private readonly onChange?: (state: VirtualListState) => void;

  constructor(options: VirtualListOptions) {
    this.onChange = options.onChange;
    this.state = buildState(
      options.count,
      options.itemSize,
      options.viewportSize,
      options.overscan ?? 0,
      options.scrollOffset ?? 0,
    );
  }

  private commit(nextState: VirtualListState): void {
    const previous = this.state;
    this.state = nextState;
    const changed =
      previous.count !== nextState.count ||
      previous.itemSize !== nextState.itemSize ||
      previous.viewportSize !== nextState.viewportSize ||
      previous.overscan !== nextState.overscan ||
      previous.scrollOffset !== nextState.scrollOffset ||
      previous.totalSize !== nextState.totalSize ||
      !sameRange(previous.range, nextState.range);
    if (changed) this.onChange?.(nextState);
  }

  private rebuild(next: Partial<Pick<VirtualListState, 'count' | 'itemSize' | 'viewportSize' | 'overscan' | 'scrollOffset'>>): void {
    this.commit(
      buildState(
        next.count ?? this.state.count,
        next.itemSize ?? this.state.itemSize,
        next.viewportSize ?? this.state.viewportSize,
        next.overscan ?? this.state.overscan,
        next.scrollOffset ?? this.state.scrollOffset,
      ),
    );
  }

  setCount(count: number): void {
    this.rebuild({ count });
  }

  setItemSize(itemSize: number): void {
    this.rebuild({ itemSize });
  }

  setViewportSize(viewportSize: number): void {
    this.rebuild({ viewportSize });
  }

  setOverscan(overscan: number): void {
    this.rebuild({ overscan });
  }

  setScrollOffset(scrollOffset: number): void {
    this.rebuild({ scrollOffset });
  }

  setOptions(options: VirtualListUpdateOptions): void {
    this.rebuild(options);
  }

  scrollToOffset(offset: number): void {
    this.setScrollOffset(offset);
  }

  scrollToIndex(index: number, align: VirtualListAlign = 'auto'): void {
    if (this.state.count === 0 || this.state.itemSize <= 0) {
      this.setScrollOffset(0);
      return;
    }
    const safeIndex = clamp(Math.floor(index), 0, Math.max(0, this.state.count - 1));
    const itemStart = safeIndex * this.state.itemSize;
    const itemEnd = itemStart + this.state.itemSize;
    const viewportStart = this.state.scrollOffset;
    const viewportEnd = viewportStart + this.state.viewportSize;

    let nextOffset = viewportStart;
    if (align === 'start') {
      nextOffset = itemStart;
    } else if (align === 'end') {
      nextOffset = itemEnd - this.state.viewportSize;
    } else if (align === 'center') {
      nextOffset = itemStart - (this.state.viewportSize - this.state.itemSize) / 2;
    } else if (itemStart < viewportStart) {
      nextOffset = itemStart;
    } else if (itemEnd > viewportEnd) {
      nextOffset = itemEnd - this.state.viewportSize;
    }

    this.setScrollOffset(nextOffset);
  }

  getState(): VirtualListState {
    return this.state;
  }

  getRange(): VirtualListRange {
    return this.state.range;
  }

  getVirtualItems(): VirtualListItem[] {
    return this.state.items;
  }

  getTotalSize(): number {
    return this.state.totalSize;
  }

  getMaxScrollOffset(): number {
    return this.state.maxScrollOffset;
  }

  getScrollOffset(): number {
    return this.state.scrollOffset;
  }
}
