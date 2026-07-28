import { FixedSizeVirtualList } from './virtual-list-core';
import type {
  LeaferVirtualListHandle,
  LeaferVirtualListHost,
  LeaferVirtualListPoint,
  VirtualListAlign,
  VirtualListOptions,
  VirtualListScrollGesture,
} from './types';

const SCROLLBAR_WIDTH = 6;
const SCROLLBAR_MIN_SIZE = 24;

type CreateLeaferVirtualListOptions = Omit<VirtualListOptions, 'onChange'> & {
  host: LeaferVirtualListHost;
  onRangeChange?: (state: ReturnType<FixedSizeVirtualList['getState']>) => void;
};

function toPoint(value: { x?: number; y?: number } | null | undefined): LeaferVirtualListPoint | null {
  if (!value || !Number.isFinite(value.x) || !Number.isFinite(value.y)) return null;
  return { x: Number(value.x), y: Number(value.y) };
}

function fallbackClientPoint(event: unknown): LeaferVirtualListPoint | null {
  if (!event || typeof event !== 'object') return null;
  const candidate = event as {
    clientX?: number;
    clientY?: number;
    x?: number;
    y?: number;
    origin?: { x?: number; y?: number };
  };
  if (Number.isFinite(candidate.clientX) && Number.isFinite(candidate.clientY)) {
    return { x: Number(candidate.clientX), y: Number(candidate.clientY) };
  }
  const origin = candidate.origin;
  if (origin && Number.isFinite(origin.x) && Number.isFinite(origin.y)) {
    return { x: Number(origin.x), y: Number(origin.y) };
  }
  if (Number.isFinite(candidate.x) && Number.isFinite(candidate.y)) {
    return { x: Number(candidate.x), y: Number(candidate.y) };
  }
  return null;
}

function getEventPoint(host: LeaferVirtualListHost, target: any, event: unknown, space: 'client' | 'box'): LeaferVirtualListPoint | null {
  return (
    toPoint(host.getPointFromEvent?.(host.hostApp, target, event, space)) ??
    (space === 'client' ? fallbackClientPoint(event) : null)
  );
}

function noop(): void {}

function asCleanup(value: (() => void) | void): () => void {
  return typeof value === 'function' ? value : noop;
}

export function createLeaferVirtualList(options: CreateLeaferVirtualListOptions): LeaferVirtualListHandle {
  const host = options.host;
  const cleanups: Array<() => void> = [];
  const list = new FixedSizeVirtualList({
    count: options.count,
    itemSize: options.itemSize,
    viewportSize: options.viewportSize,
    overscan: options.overscan,
    scrollOffset: options.scrollOffset,
    onChange: (state) => {
      syncVisuals();
      options.onRangeChange?.(state);
    },
  });
  let dragState: { startClientY: number; startOffset: number } | null = null;
  let isFocused = false;

  const requestRender = (): void => {
    host.requestRender?.();
  };

  const applyScrollOwner = (): void => {
    host.viewportBox.scrollTo = (input: { x?: number; y?: number } | number, y?: number) => {
      if (typeof input === 'number') {
        list.scrollToOffset(y ?? 0);
      } else {
        list.scrollToOffset(input.y ?? 0);
      }
      syncVisuals();
      options.onRangeChange?.(list.getState());
    };
    Object.defineProperty(host.viewportBox, 'scrollY', {
      configurable: true,
      enumerable: true,
      get: () => list.getScrollOffset(),
      set: (value: number) => {
        list.setScrollOffset(value);
        syncVisuals();
        options.onRangeChange?.(list.getState());
      },
    });
  };

  const syncVisuals = (): void => {
    const state = list.getState();
    host.viewportBox.height = state.viewportSize;
    host.contentBox.height = state.totalSize;
    host.contentBox.y = -state.scrollOffset;

    const canScroll = state.maxScrollOffset > 0 && state.viewportSize > 0;
    host.trackBox.visible = canScroll;
    host.thumbBox.visible = canScroll;
    host.trackBox.x = Math.max(0, (host.viewportBox.width ?? 0) - SCROLLBAR_WIDTH);
    host.trackBox.y = 0;
    host.trackBox.width = SCROLLBAR_WIDTH;
    host.trackBox.height = state.viewportSize;
    host.thumbBox.x = host.trackBox.x;
    host.thumbBox.width = SCROLLBAR_WIDTH;

    if (!canScroll) {
      host.thumbBox.y = 0;
      host.thumbBox.height = 0;
      requestRender();
      return;
    }

    const trackHeight = state.viewportSize;
    const thumbHeight = Math.max(
      Math.min(trackHeight, (trackHeight * state.viewportSize) / Math.max(state.totalSize, 1)),
      SCROLLBAR_MIN_SIZE,
    );
    const availableHeight = Math.max(0, trackHeight - thumbHeight);
    const thumbY = state.maxScrollOffset > 0 ? (state.scrollOffset / state.maxScrollOffset) * availableHeight : 0;
    host.thumbBox.y = thumbY;
    host.thumbBox.height = thumbHeight;
    requestRender();
  };

  const consumeGesture = (gesture: VirtualListScrollGesture): void => {
    if (gesture.moveType && gesture.moveType !== 'move') return;
    if (!isFocused) return;
    const previous = list.getScrollOffset();
    list.scrollToOffset(previous - gesture.deltaY);
    gesture.stop();
    gesture.stopNow();
  };

  const handleTrackPointerDown = (event: unknown): void => {
    const point = getEventPoint(host, host.trackBox, event, 'box');
    if (!point) return;
    const thumbTop = host.thumbBox.y ?? 0;
    const thumbBottom = thumbTop + (host.thumbBox.height ?? 0);
    if (point.y >= thumbTop && point.y <= thumbBottom) return;

    const state = list.getState();
    const nextOffset = point.y < thumbTop ? state.scrollOffset - state.viewportSize : state.scrollOffset + state.viewportSize;
    list.scrollToOffset(nextOffset);
  };

  const attachWindowDrag = (startEvent: unknown): void => {
    const startPoint = getEventPoint(host, host.thumbBox, startEvent, 'client');
    if (!startPoint) return;
    isFocused = true;
    dragState = {
      startClientY: startPoint.y,
      startOffset: list.getScrollOffset(),
    };

    const handleMove = (moveEvent: PointerEvent): void => {
      if (!dragState) return;
      const state = list.getState();
      const trackHeight = host.trackBox.height ?? 0;
      const thumbHeight = host.thumbBox.height ?? 0;
      const draggableHeight = Math.max(0, trackHeight - thumbHeight);
      if (draggableHeight <= 0 || state.maxScrollOffset <= 0) return;
      const nextClient = getEventPoint(host, host.thumbBox, moveEvent, 'client') ?? fallbackClientPoint(moveEvent);
      if (!nextClient) return;
      const deltaClientY = nextClient.y - dragState.startClientY;
      const deltaOffset = (deltaClientY / draggableHeight) * state.maxScrollOffset;
      list.scrollToOffset(dragState.startOffset + deltaOffset);
    };

    const handleUp = (): void => {
      dragState = null;
      if (typeof window !== 'undefined') {
        window.removeEventListener('pointermove', handleMove);
        window.removeEventListener('pointerup', handleUp);
      }
    };

    if (typeof window !== 'undefined') {
      window.addEventListener('pointermove', handleMove);
      window.addEventListener('pointerup', handleUp);
      cleanups.push(() => {
        window.removeEventListener('pointermove', handleMove);
        window.removeEventListener('pointerup', handleUp);
      });
    }
  };

  const handleDocumentPointerDown = (event: Event): void => {
    const point = getEventPoint(host, host.viewportBox, event, 'box');
    isFocused =
      !!point &&
      point.x >= 0 &&
      point.y >= 0 &&
      point.x <= (host.viewportBox.width ?? 0) &&
      point.y <= (host.viewportBox.height ?? 0);
  };

  applyScrollOwner();
  if (typeof document !== 'undefined') {
    document.addEventListener('pointerdown', handleDocumentPointerDown, true);
    cleanups.push(() => {
      document.removeEventListener('pointerdown', handleDocumentPointerDown, true);
    });
  }
  cleanups.push(asCleanup(host.bindVerticalScrollGesture?.(host.viewportBox, consumeGesture)));
  cleanups.push(asCleanup(host.bindPointerDown?.(host.trackBox, handleTrackPointerDown)));
  cleanups.push(asCleanup(host.bindPointerDown?.(host.thumbBox, attachWindowDrag)));
  syncVisuals();

  return {
    setCount: (count: number) => list.setCount(count),
    setItemSize: (itemSize: number) => list.setItemSize(itemSize),
    setViewportSize: (viewportSize: number) => list.setViewportSize(viewportSize),
    setOverscan: (overscan: number) => list.setOverscan(overscan),
    setScrollOffset: (offset: number) => list.setScrollOffset(offset),
    setOptions: (options) => list.setOptions(options),
    scrollToOffset: (offset: number) => list.scrollToOffset(offset),
    scrollToIndex: (index: number, align?: VirtualListAlign) => list.scrollToIndex(index, align),
    getRange: () => list.getRange(),
    getVirtualItems: () => list.getVirtualItems(),
    getTotalSize: () => list.getTotalSize(),
    getScrollOffset: () => list.getScrollOffset(),
    destroy: () => {
      dragState = null;
      while (cleanups.length > 0) {
        cleanups.pop()?.();
      }
    },
  };
}
