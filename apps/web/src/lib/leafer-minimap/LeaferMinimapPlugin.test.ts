import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { LeaferMinimapPlugin } from './LeaferMinimapPlugin';

class MockBox {
  x = 0;
  y = 0;
  width = 0;
  height = 0;
  scaleX = 1;
  scaleY = 1;
  fill = 'transparent';
  visible = true;
  children: unknown[] = [];
  removed = false;
  destroyed = false;
  handlers = new Map<string, (event?: unknown) => void>();

  constructor(props: Record<string, unknown> = {}) {
    Object.assign(this, props);
  }

  add(child: unknown) {
    this.children.push(child);
  }

  removeAll() {
    this.children = [];
  }

  remove() {
    this.removed = true;
  }

  destroy() {
    this.destroyed = true;
  }

  on(event: string, handler: (event?: unknown) => void) {
    this.handlers.set(event, handler);
  }

  off(event: string) {
    this.handlers.delete(event);
  }

  emit(event: string, payload?: unknown) {
    this.handlers.get(event)?.(payload);
  }
}

class MockPen {
  style: Record<string, unknown> | null = null;
  commands: unknown[] = [];

  setStyle(style: Record<string, unknown>) {
    this.style = style;
  }

  moveTo(x: number, y: number) {
    this.commands.push(['M', x, y]);
  }

  bezierCurveTo(c1x: number, c1y: number, c2x: number, c2y: number, toX: number, toY: number) {
    this.commands.push(['C', c1x, c1y, c2x, c2y, toX, toY]);
  }
}

class MockText extends MockBox {}

function createContainer(width = 800, height = 600): HTMLElement {
  return {
    style: { display: '' },
    getBoundingClientRect: () => ({ width, height }),
  } as HTMLElement;
}

describe('LeaferMinimapPlugin', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    const windowListeners = new Map<string, Set<(event: Event) => void>>();
    vi.stubGlobal('window', {
      addEventListener: (type: string, handler: (event: Event) => void) => {
        const bucket = windowListeners.get(type) ?? new Set();
        bucket.add(handler);
        windowListeners.set(type, bucket);
      },
      removeEventListener: (type: string, handler: (event: Event) => void) => {
        windowListeners.get(type)?.delete(handler);
      },
      dispatchEvent: (event: Event) => {
        for (const handler of windowListeners.get(event.type) ?? []) handler(event);
        return true;
      },
    });
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      return setTimeout(() => callback(Date.now()), 0);
    });
    vi.stubGlobal('cancelAnimationFrame', (id: number) => {
      clearTimeout(id);
    });
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it('mounts into sky and renders graph preview', async () => {
    const sky = new MockBox();
    const app = {
      sky,
      zoomLayer: { x: -100, y: -50, scaleX: 1, scaleY: 1 },
      update: vi.fn(),
      on: vi.fn(),
      off: vi.fn(),
    };
    const plugin = new LeaferMinimapPlugin({
      app,
      container: createContainer(),
      constructors: { BoxCtor: MockBox, PenCtor: MockPen, TextCtor: MockText },
      getViewData: () => ({
        nodes: [{ id: 1, kind: 'table', label: 'library.books[0]', x: 0, y: 0, width: 200, height: 100 }],
        edges: [{ fromX: 0, fromY: 0, c1x: 40, c1y: 0, c2x: 80, c2y: 50, toX: 120, toY: 50 }],
      }),
    });

    vi.runOnlyPendingTimers();

    const root = sky.children[0] as MockBox;
    const edgeLayer = root.children[1] as MockBox;
    const nodeLayer = root.children[2] as MockBox;
    const viewport = root.children[3] as MockBox;

    expect(root.x).toBe(564);
    expect(root.y).toBe(434);
    expect(root.scaleX).toBe(1);
    expect(root.scaleY).toBe(1);
    expect(edgeLayer.children).toHaveLength(1);
    expect(nodeLayer.children).toHaveLength(1);
    expect(viewport.visible).toBe(true);

    plugin.destroy();
  });

  it('requests a world viewport when dragging the viewport rectangle', async () => {
    const requestViewport = vi.fn();
    const app = {
      sky: new MockBox(),
      zoomLayer: { x: 0, y: 0, scaleX: 1, scaleY: 1 },
      update: vi.fn(),
      on: vi.fn(),
      off: vi.fn(),
    };
    const plugin = new LeaferMinimapPlugin({
      app,
      container: createContainer(400, 300),
      constructors: { BoxCtor: MockBox, PenCtor: MockPen },
      events: { pointerDown: 'down' },
      getViewData: () => ({
        nodes: [{ id: 1, x: 0, y: 0, width: 800, height: 600 }],
        edges: [],
      }),
      requestViewport,
    });

    vi.runOnlyPendingTimers();

    const root = app.sky.children[0] as MockBox;
    const viewport = root.children[3] as MockBox;
    viewport.emit('down', { clientX: 190, clientY: 140, stop: vi.fn(), stopNow: vi.fn() });
    const moveEvent = new Event('pointermove') as Event & { clientX: number; clientY: number };
    moveEvent.clientX = 210;
    moveEvent.clientY = 150;
    window.dispatchEvent(moveEvent);

    expect(app.zoomLayer).toEqual({ x: 0, y: 0, scaleX: 1, scaleY: 1 });
    expect(app.update).not.toHaveBeenCalled();
    expect(requestViewport).toHaveBeenCalledWith(expect.objectContaining({ x: expect.any(Number), y: expect.any(Number) }));

    plugin.destroy();
  });

  it('requests a world viewport when clicking the minimap background', () => {
    const requestViewport = vi.fn();
    const app = {
      sky: new MockBox(),
      zoomLayer: { x: 0, y: 0, scaleX: 1, scaleY: 1 },
      update: vi.fn(),
      on: vi.fn(),
      off: vi.fn(),
    };
    const plugin = new LeaferMinimapPlugin({
      app,
      container: createContainer(400, 300),
      constructors: { BoxCtor: MockBox, PenCtor: MockPen },
      events: { pointerDown: 'down' },
      getViewData: () => ({
        nodes: [{ id: 1, x: 0, y: 0, width: 1000, height: 800 }],
        edges: [],
      }),
      requestViewport,
    });

    vi.runOnlyPendingTimers();

    const root = app.sky.children[0] as MockBox;
    const background = root.children[0] as MockBox;
    background.emit('down', { clientX: 320, clientY: 220, stop: vi.fn(), stopNow: vi.fn() });

    expect(app.zoomLayer).toEqual({ x: 0, y: 0, scaleX: 1, scaleY: 1 });
    expect(app.update).not.toHaveBeenCalled();
    expect(requestViewport).toHaveBeenCalledWith(expect.objectContaining({ x: expect.any(Number), y: expect.any(Number) }));

    plugin.destroy();
  });

  it('keeps the minimap fixed when the main viewport moves and zooms', () => {
    const appHandlers = new Map<string, (event?: unknown) => void>();
    const app = {
      sky: new MockBox(),
      zoomLayer: { x: 0, y: 0, scaleX: 1, scaleY: 1 },
      on: vi.fn((event: string, handler: (event?: unknown) => void) => {
        appHandlers.set(event, handler);
      }),
      off: vi.fn(),
      update: vi.fn(),
    };
    const plugin = new LeaferMinimapPlugin({
      app,
      container: createContainer(),
      constructors: { BoxCtor: MockBox, PenCtor: MockPen },
      events: { move: 'move', zoom: 'zoom' },
      getViewData: () => ({ nodes: [{ id: 1, x: 0, y: 0, width: 1000, height: 800 }], edges: [] }),
    });

    vi.runOnlyPendingTimers();
    app.zoomLayer.x = -120;
    app.zoomLayer.y = -80;
    app.zoomLayer.scaleX = 2;
    app.zoomLayer.scaleY = 2;
    appHandlers.get('move')?.();
    vi.runOnlyPendingTimers();

    const root = app.sky.children[0] as MockBox;
    expect(root.x).toBe(564);
    expect(root.y).toBe(434);
    expect(root.scaleX).toBe(1);
    expect(root.scaleY).toBe(1);

    plugin.destroy();
  });

  it('does not move the minimap root while dragging the viewport in sky', () => {
    const app = {
      sky: new MockBox(),
      zoomLayer: { x: 0, y: 0, scaleX: 1, scaleY: 1 },
      update: vi.fn(),
      on: vi.fn(),
      off: vi.fn(),
    };
    const plugin = new LeaferMinimapPlugin({
      app,
      container: createContainer(400, 300),
      constructors: { BoxCtor: MockBox, PenCtor: MockPen },
      events: { pointerDown: 'down' },
      getViewData: () => ({
        nodes: [{ id: 1, x: 0, y: 0, width: 800, height: 600 }],
        edges: [],
      }),
    });

    vi.runOnlyPendingTimers();
    const root = app.sky.children[0] as MockBox;
    const viewport = root.children[3] as MockBox;
    const initialRoot = { x: root.x, y: root.y, scaleX: root.scaleX, scaleY: root.scaleY };
    viewport.emit('down', { clientX: 190, clientY: 140, stop: vi.fn(), stopNow: vi.fn() });
    const moveEvent = new Event('pointermove') as Event & { clientX: number; clientY: number };
    moveEvent.clientX = 230;
    moveEvent.clientY = 170;
    window.dispatchEvent(moveEvent);

    expect(root.x).toBe(initialRoot.x);
    expect(root.y).toBe(initialRoot.y);
    expect(root.scaleX).toBe(initialRoot.scaleX);
    expect(root.scaleY).toBe(initialRoot.scaleY);

    plugin.destroy();
  });

  it('mounts into a separate minimap app at the local origin', () => {
    const mountApp = new MockBox();
    const app = {
      sky: new MockBox(),
      zoomLayer: { x: -120, y: -80, scaleX: 2, scaleY: 2 },
      update: vi.fn(),
      on: vi.fn(),
      off: vi.fn(),
    };
    const plugin = new LeaferMinimapPlugin({
      app,
      mountApp,
      mountContainer: createContainer(220, 150),
      container: createContainer(800, 600),
      constructors: { BoxCtor: MockBox, PenCtor: MockPen },
      getViewData: () => ({ nodes: [{ id: 1, x: 0, y: 0, width: 1000, height: 800 }], edges: [] }),
    });

    vi.runOnlyPendingTimers();

    const root = mountApp.children[0] as MockBox;
    expect(root.x).toBe(0);
    expect(root.y).toBe(0);
    expect(root.scaleX).toBe(1);
    expect(root.scaleY).toBe(1);

    plugin.destroy();
  });

  it('clips the viewport rectangle inside the minimap panel', () => {
    const mountApp = new MockBox();
    const app = {
      zoomLayer: { x: -900, y: -700, scaleX: 1, scaleY: 1 },
      update: vi.fn(),
      on: vi.fn(),
      off: vi.fn(),
    };
    const plugin = new LeaferMinimapPlugin({
      app,
      mountApp,
      mountContainer: createContainer(220, 150),
      container: createContainer(800, 600),
      constructors: { BoxCtor: MockBox, PenCtor: MockPen },
      getViewData: () => ({ nodes: [{ id: 1, x: 0, y: 0, width: 1000, height: 800 }], edges: [] }),
    });

    vi.runOnlyPendingTimers();

    const root = mountApp.children[0] as MockBox;
    const viewport = root.children[3] as MockBox;
    expect(viewport.x).toBeGreaterThanOrEqual(0);
    expect(viewport.y).toBeGreaterThanOrEqual(0);
    expect((viewport.x ?? 0) + (viewport.width ?? 0)).toBeLessThanOrEqual(220);
    expect((viewport.y ?? 0) + (viewport.height ?? 0)).toBeLessThanOrEqual(150);

    plugin.destroy();
  });

  it('hides the minimap when all graph nodes are inside the initial viewport', () => {
    const mountContainer = createContainer(220, 150);
    const app = {
      zoomLayer: { x: 0, y: 0, scaleX: 1, scaleY: 1 },
      update: vi.fn(),
      on: vi.fn(),
      off: vi.fn(),
    };
    const mountApp = new MockBox();
    const plugin = new LeaferMinimapPlugin({
      app,
      mountApp,
      mountContainer,
      container: createContainer(800, 600),
      constructors: { BoxCtor: MockBox, PenCtor: MockPen },
      getViewData: () => ({ nodes: [{ id: 1, x: 24, y: 24, width: 120, height: 80 }], edges: [] }),
    });

    vi.runOnlyPendingTimers();

    const root = mountApp.children[0] as MockBox;
    expect(root.visible).toBe(false);
    expect(mountContainer.style.display).toBe('none');

    plugin.destroy();
  });

  it('shows the minimap when at least one graph node is outside the initial viewport', () => {
    const mountContainer = createContainer(220, 150);
    const app = {
      zoomLayer: { x: 0, y: 0, scaleX: 1, scaleY: 1 },
      update: vi.fn(),
      on: vi.fn(),
      off: vi.fn(),
    };
    const mountApp = new MockBox();
    const plugin = new LeaferMinimapPlugin({
      app,
      mountApp,
      mountContainer,
      container: createContainer(800, 600),
      constructors: { BoxCtor: MockBox, PenCtor: MockPen },
      getViewData: () => ({ nodes: [{ id: 1, x: 900, y: 24, width: 120, height: 80 }], edges: [] }),
    });

    vi.runOnlyPendingTimers();

    const root = mountApp.children[0] as MockBox;
    expect(root.visible).toBe(true);
    expect(mountContainer.style.display).toBe('');

    plugin.destroy();
  });

  it('projects an extremely tall graph across the full minimap width', () => {
    const mountApp = new MockBox();
    const plugin = new LeaferMinimapPlugin({
      app: {
        zoomLayer: { x: 0, y: 0, scaleX: 1, scaleY: 1 },
        update: vi.fn(),
        on: vi.fn(),
        off: vi.fn(),
      },
      mountApp,
      mountContainer: createContainer(220, 150),
      container: createContainer(800, 600),
      constructors: { BoxCtor: MockBox, PenCtor: MockPen },
      getViewData: () => ({
        nodes: [
          { id: 1, x: 0, y: 0, width: 100, height: 100 },
          { id: 2, x: 7044, y: 1154745, width: 100, height: 100 },
        ],
        edges: [],
      }),
    });

    vi.runOnlyPendingTimers();

    const root = mountApp.children[0] as MockBox;
    const nodeLayer = root.children[2] as MockBox;
    const [first, last] = nodeLayer.children as MockBox[];
    expect((last.x ?? 0) - (first.x ?? 0)).toBeGreaterThan(200);

    plugin.destroy();
  });

  it('cleans up mounted nodes and bound app events on destroy', async () => {
    const appHandlers = new Map<string, (event?: unknown) => void>();
    const app = {
      sky: new MockBox(),
      zoomLayer: { x: 0, y: 0, scaleX: 1, scaleY: 1 },
      on: vi.fn((event: string, handler: (event?: unknown) => void) => {
        appHandlers.set(event, handler);
      }),
      off: vi.fn((event: string) => {
        appHandlers.delete(event);
      }),
      update: vi.fn(),
    };
    const plugin = new LeaferMinimapPlugin({
      app,
      container: createContainer(),
      constructors: { BoxCtor: MockBox, PenCtor: MockPen },
      events: { move: 'move', zoom: 'zoom' },
      getViewData: () => ({ nodes: [], edges: [] }),
    });

    vi.runOnlyPendingTimers();
    const root = app.sky.children[0] as MockBox;

    plugin.destroy();

    expect(root.removed).toBe(true);
    expect(root.destroyed).toBe(true);
    expect(app.off).toHaveBeenCalledWith('move', expect.any(Function));
    expect(app.off).toHaveBeenCalledWith('zoom', expect.any(Function));
    expect(appHandlers.size).toBe(0);
  });
});
