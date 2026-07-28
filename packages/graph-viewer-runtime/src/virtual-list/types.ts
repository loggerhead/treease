export type VirtualListRange = {
  start: number;
  end: number;
};

export type VirtualListItem = {
  index: number;
  key: number;
  size: number;
  start: number;
  end: number;
};

export type VirtualListState = {
  count: number;
  itemSize: number;
  viewportSize: number;
  overscan: number;
  scrollOffset: number;
  totalSize: number;
  maxScrollOffset: number;
  range: VirtualListRange;
  items: VirtualListItem[];
};

export type VirtualListAlign = 'auto' | 'start' | 'center' | 'end';

export type VirtualListOptions = {
  count: number;
  itemSize: number;
  viewportSize: number;
  overscan?: number;
  scrollOffset?: number;
  onChange?: (state: VirtualListState) => void;
};

export type VirtualListUpdateOptions = Partial<
  Pick<VirtualListState, 'count' | 'itemSize' | 'viewportSize' | 'overscan' | 'scrollOffset'>
>;


export type VirtualListScrollGesture = {
  event: unknown;
  deltaY: number;
  moveType?: string;
  stop: () => void;
  stopNow: () => void;
};

export type LeaferVirtualListPointSpace = 'client' | 'box' | 'local' | 'world';

export type LeaferVirtualListPoint = {
  x: number;
  y: number;
};

export type LeaferVirtualListHandle = {
  setCount: (count: number) => void;
  setItemSize: (itemSize: number) => void;
  setViewportSize: (viewportSize: number) => void;
  setOverscan: (overscan: number) => void;
  setScrollOffset: (offset: number) => void;
  setOptions: (options: VirtualListUpdateOptions) => void;
  scrollToOffset: (offset: number) => void;
  scrollToIndex: (index: number, align?: VirtualListAlign) => void;
  getRange: () => VirtualListRange;
  getVirtualItems: () => VirtualListItem[];
  getTotalSize: () => number;
  getScrollOffset: () => number;
  destroy: () => void;
};

export type LeaferVirtualListHost = {
  hostApp?: any;
  viewportBox: any;
  contentBox: any;
  trackBox: any;
  thumbBox: any;
  bindVerticalScrollGesture?: (
    target: any,
    handler: (gesture: VirtualListScrollGesture) => void,
  ) => (() => void) | void;
  bindPointerDown?: (target: any, handler: (event: unknown) => void | Promise<void>) => (() => void) | void;
  getPointFromEvent?: (
    hostApp: any,
    target: any,
    event: unknown,
    space: LeaferVirtualListPointSpace,
  ) => LeaferVirtualListPoint | null;
  requestRender?: () => void;
};
