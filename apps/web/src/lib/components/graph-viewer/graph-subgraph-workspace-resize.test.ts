// @vitest-environment jsdom

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const renderKernelMocks = vi.hoisted(() => ({
  renderGraphEdges: vi.fn(),
  renderGraphNode: vi.fn(({ node }: any) => ({
    nodeBox: {
      x: node.boxArgs.x,
      y: node.boxArgs.y,
      width: node.boxArgs.width,
      height: node.boxArgs.height,
      removeAll: vi.fn(),
      remove: vi.fn(),
    },
  })),
}));

vi.mock('@treease/graph-viewer-runtime', async (importOriginal) => ({
  ...(await importOriginal()),
  ...renderKernelMocks,
  getZoomScale: vi.fn((layer: { scaleX?: number; scaleY?: number }) => ({
    scaleX: layer.scaleX ?? 1,
    scaleY: layer.scaleY ?? 1,
  })),
  clampPanOffsetToGraphBounds: vi.fn((viewport: { offsetX: number; offsetY: number }) => ({
    x: viewport.offsetX,
    y: viewport.offsetY,
  })),
}));

import { destroySubgraphWorkspaceRuntime, renderSubgraphWorkspaceGraph } from './graph-subgraph-workspace';

class MockResizeObserver {
  static instances: MockResizeObserver[] = [];

  callback: ResizeObserverCallback;
  observed: Element[] = [];
  disconnected = false;

  constructor(callback: ResizeObserverCallback) {
    this.callback = callback;
    MockResizeObserver.instances.push(this);
  }

  observe(target: Element): void {
    this.observed.push(target);
  }

  disconnect(): void {
    this.disconnected = true;
  }

  trigger(): void {
    this.callback([] as ResizeObserverEntry[], this as unknown as ResizeObserver);
  }
}

class MockBox {
  x = 0;
  y = 0;
  width = 0;
  height = 0;
  tag = 'Box';
  children: unknown[] = [];

  constructor(props: Record<string, unknown> = {}) {
    Object.assign(this, props);
  }

  add(child: unknown): void {
    this.children.push(child);
  }

  removeAll(): void {}

  remove(): void {}
}

class MockApp {
  zoomLayer = new MockBox();
  resize = vi.fn();
  updateClientBounds = vi.fn();
  update = vi.fn();
  destroy = vi.fn();
}

function defineClientSize(element: HTMLElement, size: { width: number; height: number }): void {
  Object.defineProperty(element, 'clientWidth', {
    configurable: true,
    get: () => size.width,
  });
  Object.defineProperty(element, 'clientHeight', {
    configurable: true,
    get: () => size.height,
  });
}

function createGraph() {
  return {
    pathKey: 'k:root',
    path: [],
    minX: 0,
    minY: 0,
    width: 240,
    height: 120,
    edges: [],
    nodes: [
      {
        renderHandle: 1,
        kind: 'object',
        depth: 0,
        path: [],
        boxArgs: { x: 0, y: 0, width: 240, height: 120, cornerRadius: 8 },
        meta: {
          text: 'root',
          value: '{1}',
          valueType: 'object',
          isIndex: false,
          path: [],
          editable: false,
          boxArgs: { x: 0, y: 0, width: 0, height: 0, cornerRadius: 0 },
          textArgs: { x: 0, y: 0, width: 0, height: 0, text: 'root', textAlign: 'left', verticalAlign: 'middle', editable: false },
        },
        rows: [],
      },
    ],
  } as any;
}

describe('renderSubgraphWorkspaceGraph resize sync', () => {
  const originalResizeObserver = globalThis.ResizeObserver;

  beforeEach(() => {
    MockResizeObserver.instances = [];
    globalThis.ResizeObserver = MockResizeObserver as unknown as typeof ResizeObserver;
  });

  afterEach(() => {
    globalThis.ResizeObserver = originalResizeObserver;
    vi.clearAllMocks();
  });

  it('resizes the workspace runtime when the host height grows', async () => {
    const size = { width: 360, height: 180 };
    const mount = document.createElement('div');
    defineClientSize(mount, size);

    const app = new MockApp();
    const runtime = await renderSubgraphWorkspaceGraph(mount as HTMLDivElement, createGraph(), {
      getConstructors: () => ({
        LeaferCtor: class {
          constructor() {
            return app;
          }
        } as any,
        BoxCtor: MockBox as any,
        TextCtor: MockBox as any,
        PenCtor: class {
          setStyle(): void {}
          moveTo(): void {}
          bezierCurveTo(): void {}
        } as any,
      }),
      getRenderConfig: () => ({
        layout: { baseFontSize: 14 },
      }) as any,
      getLanguageId: () => 'json',
      getValueTypeToSemType: () => ({}),
      bindGraphEditorLifecycle: vi.fn(),
      bindPointerClick: vi.fn(),
      resolveInteractiveCellPath: vi.fn(async (cell) => cell.path ?? []),
      onActivateCell: vi.fn(),
    });

    expect(runtime).not.toBeNull();
    expect(app.resize).toHaveBeenCalledWith({ width: 360, height: 180 });

    size.height = 320;
    MockResizeObserver.instances[0]?.trigger();

    expect(app.resize).toHaveBeenLastCalledWith({ width: 360, height: 320 });

    destroySubgraphWorkspaceRuntime(runtime);
    expect(MockResizeObserver.instances[0]?.disconnected).toBe(true);
  });
});
