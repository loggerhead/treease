import { afterEach, describe, expect, it, vi } from 'vitest';
import { createLeaferVirtualList } from './leafer-virtual-list';

class MockBox {
  x: number;
  y: number;
  width: number;
  height: number;
  visible = true;
  children: any[] = [];

  constructor(props: Record<string, any> = {}) {
    this.x = props.x ?? 0;
    this.y = props.y ?? 0;
    this.width = props.width ?? 0;
    this.height = props.height ?? 0;
    this.visible = props.visible ?? true;
  }

  add(child: any) {
    this.children.push(child);
  }
}

afterEach(() => {
  vi.restoreAllMocks();
  delete (globalThis as Record<string, unknown>).window;
  delete (globalThis as Record<string, unknown>).document;
});

describe('createLeaferVirtualList', () => {
  it('updates content position and consumes move gestures', () => {
    const requestRender = vi.fn();
    let gestureHandler: ((gesture: any) => void) | null = null;
    const documentListeners = new Map<string, Set<(event: Event) => void>>();
    (globalThis as Record<string, unknown>).document = {
      addEventListener: (type: string, handler: (event: Event) => void) => {
        const bucket = documentListeners.get(type) ?? new Set();
        bucket.add(handler);
        documentListeners.set(type, bucket);
      },
      removeEventListener: (type: string, handler: (event: Event) => void) => {
        documentListeners.get(type)?.delete(handler);
      },
    };
    const viewportBox = new MockBox({ width: 120, height: 80 });
    const contentBox = new MockBox({ width: 120, height: 400 });
    const trackBox = new MockBox({ width: 6, height: 80 });
    const thumbBox = new MockBox({ width: 6, height: 0 });
    const onRangeChange = vi.fn();

    const list = createLeaferVirtualList({
      count: 20,
      itemSize: 20,
      viewportSize: 80,
      overscan: 1,
      host: {
        viewportBox,
        contentBox,
        trackBox,
        thumbBox,
        requestRender,
        bindVerticalScrollGesture: (_target, handler) => {
          gestureHandler = handler;
          return () => {
            gestureHandler = null;
          };
        },
        bindPointerDown: () => () => {},
        getPointFromEvent: (_hostApp, _target, event, space) => {
          const value = event as Record<string, { x: number; y: number } | undefined>;
          return value[space] ?? null;
        },
      },
      onRangeChange,
    });

    const stop = vi.fn();
    const stopNow = vi.fn();
    for (const handler of documentListeners.get('pointerdown') ?? []) {
      handler({ type: 'pointerdown', box: { x: 10, y: 10 } } as unknown as Event);
    }
    gestureHandler?.({ event: {}, deltaY: -40, moveType: 'move', stop, stopNow });

    expect(list.getScrollOffset()).toBe(40);
    expect(contentBox.y).toBe(-40);
    expect(list.getRange()).toEqual({ start: 2, end: 7 });
    expect(stop).toHaveBeenCalledTimes(1);
    expect(stopNow).toHaveBeenCalledTimes(1);
    expect(requestRender).toHaveBeenCalled();
    expect(onRangeChange).toHaveBeenCalled();
  });

  it('ignores non-scroll move types and supports track click plus thumb drag', () => {
    const listeners = new Map<string, Set<(event: Event) => void>>();
    const documentListeners = new Map<string, Set<(event: Event) => void>>();
    (globalThis as Record<string, unknown>).window = {
      addEventListener: (type: string, handler: (event: Event) => void) => {
        const bucket = listeners.get(type) ?? new Set();
        bucket.add(handler);
        listeners.set(type, bucket);
      },
      removeEventListener: (type: string, handler: (event: Event) => void) => {
        listeners.get(type)?.delete(handler);
      },
      dispatchEvent: (event: Event) => {
        for (const handler of listeners.get(event.type) ?? []) handler(event);
        return true;
      },
    };
    (globalThis as Record<string, unknown>).document = {
      addEventListener: (type: string, handler: (event: Event) => void) => {
        const bucket = documentListeners.get(type) ?? new Set();
        bucket.add(handler);
        documentListeners.set(type, bucket);
      },
      removeEventListener: (type: string, handler: (event: Event) => void) => {
        documentListeners.get(type)?.delete(handler);
      },
    };
    let gestureHandler: ((gesture: any) => void) | null = null;
    const pointerHandlers = new Map<any, (event: unknown) => void | Promise<void>>();
    const viewportBox = new MockBox({ width: 120, height: 80 });
    const contentBox = new MockBox({ width: 120, height: 400 });
    const trackBox = new MockBox({ width: 6, height: 80 });
    const thumbBox = new MockBox({ width: 6, height: 0 });

    const list = createLeaferVirtualList({
      count: 20,
      itemSize: 20,
      viewportSize: 80,
      overscan: 1,
      host: {
        viewportBox,
        contentBox,
        trackBox,
        thumbBox,
        requestRender: vi.fn(),
        bindVerticalScrollGesture: (_target, handler) => {
          gestureHandler = handler;
          return () => {
            gestureHandler = null;
          };
        },
        bindPointerDown: (target, handler) => {
          pointerHandlers.set(target, handler);
          return () => {
            pointerHandlers.delete(target);
          };
        },
        getPointFromEvent: (_hostApp, _target, event, space) => {
          const value = event as Record<string, { x: number; y: number } | undefined>;
          return value[space] ?? null;
        },
      },
    });

    for (const handler of documentListeners.get('pointerdown') ?? []) {
      handler({ type: 'pointerdown', box: { x: 10, y: 10 } } as unknown as Event);
    }
    gestureHandler?.({ event: {}, deltaY: 60, moveType: 'drag', stop: vi.fn(), stopNow: vi.fn() });
    expect(list.getScrollOffset()).toBe(0);

    pointerHandlers.get(trackBox)?.({ box: { x: 0, y: 70 } });
    expect(list.getScrollOffset()).toBe(80);

    pointerHandlers.get(thumbBox)?.({ client: { x: 0, y: 10 } });
    const moveEvent = new Event('pointermove') as Event & { clientX: number; clientY: number };
    moveEvent.clientX = 0;
    moveEvent.clientY = 40;
    (globalThis.window as { dispatchEvent: (event: Event) => boolean }).dispatchEvent(moveEvent);

    expect(list.getScrollOffset()).toBeGreaterThan(80);

    const stop = vi.fn();
    const stopNow = vi.fn();
    list.scrollToOffset(320);
    gestureHandler?.({ event: {}, deltaY: -40, moveType: 'move', stop, stopNow });
    expect(list.getScrollOffset()).toBe(320);
    expect(stop).toHaveBeenCalledTimes(1);
    expect(stopNow).toHaveBeenCalledTimes(1);

    for (const handler of documentListeners.get('pointerdown') ?? []) {
      handler({ type: 'pointerdown', box: { x: 200, y: 200 } } as unknown as Event);
    }
    const blurStop = vi.fn();
    const blurStopNow = vi.fn();
    gestureHandler?.({ event: {}, deltaY: -40, moveType: 'move', stop: blurStop, stopNow: blurStopNow });
    expect(blurStop).not.toHaveBeenCalled();
    expect(blurStopNow).not.toHaveBeenCalled();
    list.destroy();
  });
});
