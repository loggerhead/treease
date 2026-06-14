import { beforeEach, describe, expect, it, vi } from 'vitest';

const mockStreamUpdateHandler = vi.hoisted(() => ({
  createEmptyStreamState: vi.fn(() => ({ nodes: [] as any[], edges: [] as any[] })),
  replaceStreamState: vi.fn((state: any, graphData: any) => {
    state.nodes = [...graphData.nodes];
    state.edges = [...graphData.edges];
  }),
  clearStreamState: vi.fn((state: any) => {
    state.nodes = [];
    state.edges = [];
  }),
  applyGraphDeltaToState: vi.fn((delta: any, state: any) => {
    if (delta.clear === 1) {
      state.nodes = [];
      state.edges = [];
    }
    const nodeMap = new Map<number, any>((state.nodes ?? []).map((node: any) => [node.renderHandle, node]));
    for (const nodeId of delta.nodesRemoved ?? []) nodeMap.delete(nodeId);
    for (const node of delta.nodesAdded ?? []) nodeMap.set(node.renderHandle, node);
    for (const node of delta.nodesUpdated ?? []) nodeMap.set(node.renderHandle, node);
    for (const patch of delta.layoutPatches ?? []) {
      const nodeId = patch.render_handle ?? patch.renderHandle ?? patch.group_handle ?? patch.groupHandle;
      const node = nodeMap.get(nodeId);
      if (node) {
        nodeMap.set(nodeId, {
          ...node,
          boxArgs:
            patch.box_args ??
            patch.boxArgs ?? {
              ...node.boxArgs,
              width: patch.width ?? node.boxArgs.width,
              height: patch.height ?? node.boxArgs.height,
            },
        });
      }
    }
    for (const tablePatch of delta.tableCellPatches ?? []) {
      const node = nodeMap.get(tablePatch.tableRenderHandle);
      if (node) nodeMap.set(tablePatch.tableRenderHandle, node);
    }
    state.nodes = [...nodeMap.values()];
  }),
  applyVersionedProjection: vi.fn((state: any, delta: any, version: any) => {
    // baseGraphVersion === 0 is a reset — always accept
    // baseGraphVersion < state.version is a stale chunk — skip
    if (version.baseGraphVersion !== 0 && version.baseGraphVersion < (state.version ?? 0)) {
      return; // stale — no-op
    }
    if (delta.clear === 1) {
      state.nodes = [];
      state.edges = [];
    }
    const nodeMap = new Map<number, any>((state.nodes ?? []).map((node: any) => [node.renderHandle, node]));
    for (const nodeId of delta.nodesRemoved ?? []) nodeMap.delete(nodeId);
    for (const node of delta.nodesAdded ?? []) nodeMap.set(node.renderHandle, node);
    for (const node of delta.nodesUpdated ?? []) nodeMap.set(node.renderHandle, node);
    for (const patch of delta.layoutPatches ?? []) {
      const nodeId = patch.render_handle ?? patch.renderHandle ?? patch.group_handle ?? patch.groupHandle;
      const node = nodeMap.get(nodeId);
      if (node) {
        nodeMap.set(nodeId, {
          ...node,
          boxArgs:
            patch.box_args ??
            patch.boxArgs ?? {
              ...node.boxArgs,
              width: patch.width ?? node.boxArgs.width,
              height: patch.height ?? node.boxArgs.height,
            },
        });
      }
    }
    for (const tablePatch of delta.tableCellPatches ?? []) {
      const node = nodeMap.get(tablePatch.tableRenderHandle);
      if (node) nodeMap.set(tablePatch.tableRenderHandle, node);
    }
    state.nodes = [...nodeMap.values()];
    state.version = version.graphVersion;
  }),
  streamStateToArrays: vi.fn((state: any) => ({
    nodes: [...(state.nodes ?? [])],
    edges: [...(state.edges ?? [])],
  })),
}));

const mockRenderState = vi.hoisted(() => ({
  renderCount: 0,
  patchCount: 0,
}));

vi.mock('../../graph/StreamUpdateHandler', () => mockStreamUpdateHandler);

vi.mock('../../graph/graph-viewer-render', () => ({
  createCellText: vi.fn((drawContext: any, _layer: any, cell: any) => ({
    kind: 'meta-text',
    text: cell?.text ?? '',
    removeAll: vi.fn(),
    remove: vi.fn(),
    on: vi.fn(),
  })),
  describeTableRuntime: vi.fn(() => ({ layoutSignature: 'same-layout' })),
  destroyTableRuntime: vi.fn(),
  patchTableContent: vi.fn((drawContext: any, existingRuntime: any, node: any) => {
    mockRenderState.patchCount += 1;
    drawContext.registerClickTarget(
      {
        kind: 'patched-target',
        nodeId: node.renderHandle,
        order: mockRenderState.patchCount,
        on: vi.fn(),
      },
      { text: `patched-${node.renderHandle}`, path: node.path ?? [] },
      'value',
      node.kind,
    );
    return existingRuntime;
  }),
  patchTableStructure: vi.fn((drawContext: any, existingRuntime: any, node: any) => {
    mockRenderState.patchCount += 1;
    drawContext.registerClickTarget(
      {
        kind: 'patched-structure-target',
        nodeId: node.renderHandle,
        order: mockRenderState.patchCount,
        on: vi.fn(),
      },
      { text: `patched-structure-${node.renderHandle}`, path: node.path ?? [] },
      'value',
      node.kind,
    );
    return existingRuntime;
  }),
  tableRuntimeOps: {},
}));

vi.mock('./graph-render-kernel', () => ({
  renderGraphEdges: vi.fn((_args: any) => []),
  renderGraphNode: vi.fn(({ node, drawContext, registerMetaClickTarget }: any) => {
    mockRenderState.renderCount += 1;
    const nodeBox = {
      x: node.boxArgs.x,
      y: node.boxArgs.y,
      width: node.boxArgs.width,
      height: node.boxArgs.height,
      cornerRadius: node.boxArgs.cornerRadius,
      removeAll: vi.fn(),
      remove: vi.fn(),
    };
    const metaText = {
      kind: 'initial-meta',
      nodeId: node.renderHandle,
      order: mockRenderState.renderCount,
      removeAll: vi.fn(),
      remove: vi.fn(),
      on: vi.fn(),
    };
    registerMetaClickTarget(metaText, node.meta, 'meta');
    drawContext.registerClickTarget(
      {
        kind: 'initial-target',
        nodeId: node.renderHandle,
        order: mockRenderState.renderCount,
        on: vi.fn(),
      },
      { text: `initial-${node.renderHandle}`, path: node.path ?? [] },
      'value',
      node.kind,
    );
    return {
      nodeBox,
      metaText,
      tableRuntime: { layoutSignature: 'same-layout' },
    };
  }),
}));

import { createGraphSceneRuntime } from './graph-scene-runtime';

function createNode(id: number, x: number) {
  return {
    renderHandle: id,
    kind: 'table',
    path: [{ tag: 0, key: 'library', index: 0 }],
    boxArgs: { x, y: 20, width: 200, height: 80, cornerRadius: 8 },
    meta: { text: `node-${id}`, valueType: 'object', path: [{ tag: 0, key: 'library', index: 0 }] },
    table: {
      headerHeight: 24,
      rows: [],
    },
  } as any;
}

function createDeps(options?: { canvasPadding?: number }) {
  const container = {
    setAttribute: vi.fn(),
  } as unknown as HTMLElement;
  const renderRoot = { add: vi.fn() };
  const probeStore: Record<string, any> = {};
  const nodeDataMap = new Map<number, any>();
  const nodeBoxMap = new Map<number, any>();
  const pathKeyToRenderHandleMap = new Map<string, number>();
  let nextClickTargetId = 0;

  class MockBox {
    x = 0;
    y = 0;
    width = 0;
    height = 0;
    fill = 'transparent';
    hittable = true;
    hitChildren = true;
    children: any[] = [];

    constructor(args: Record<string, any> = {}) {
      Object.assign(this, args);
    }

    add(child: any) {
      this.children.push(child);
    }

    removeAll() {}

    remove() {}

    on() {}
  }

  class MockText extends MockBox {}
  class MockPen extends MockBox {}

  const updateLeafer = vi.fn();
  const leafer = {
    zoomLayer: { x: 0, y: 0 },
  };
  let lastAutoOffset: { x: number; y: number } | null = null;
  const setLastAutoOffset = vi.fn((value: { x: number; y: number } | null) => {
    lastAutoOffset = value;
  });
  const layers = {
    nodeLayer: new MockBox(),
    edgeLayer: new MockBox(),
    overlayLayer: new MockBox(),
  } as any;

  return {
    probeStore,
    runtime: createGraphSceneRuntime({
      getContainer: () => container,
      getLeafer: () => leafer,
      getRenderRoot: () => renderRoot,
      getBoxCtor: () => MockBox,
      getTextCtor: () => MockText,
      getPenCtor: () => MockPen,
      getRenderConfig: () =>
        ({
          layout: {
            canvasPadding: options?.canvasPadding ?? 0,
            baseFontSize: 14,
          },
        }) as any,
      getLanguageId: () => 'json',
      getValueTypeToSemType: () => ({}),
      getLastAutoOffset: () => lastAutoOffset,
      setLastAutoOffset,
      getLayers: () => layers,
      setLayers: vi.fn((nextLayers: any) => Object.assign(layers, nextLayers)),
      buildPathSegFromCell: vi.fn(),
      clearSearchHighlight: vi.fn(),
      beginMainGraphRedraw: vi.fn(),
      getNodeDataMap: () => nodeDataMap,
      getNodeBoxMap: () => nodeBoxMap,
      getPathKeyToRenderHandleMap: () => pathKeyToRenderHandleMap,
      getClickTargetProbes: () => Object.values(probeStore),
      getClickTargetProbeStore: () => probeStore as any,
      registerCellBox: vi.fn(),
      unregisterCellBox: vi.fn(),
      registerRowBox: vi.fn(),
      unregisterRowBox: vi.fn(),
      registerClickTarget: vi.fn((target: any, cell: any, kind: any) => {
        const id = `target-${nextClickTargetId++}`;
        probeStore[id] = { id, box: target, cell, target: kind };
        return id;
      }),
      updateLeafer,
    }),
    leafer,
    setLastAutoOffset,
    updateLeafer,
    nodeBoxMap,
  };
}

function useControlledSceneTimers() {
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
  vi.stubGlobal('requestAnimationFrame', ((callback: FrameRequestCallback) => {
    return setTimeout(() => callback(0), 0) as unknown as number;
  }) as typeof requestAnimationFrame);
  vi.stubGlobal('cancelAnimationFrame', ((handle: number) => {
    clearTimeout(handle);
  }) as typeof cancelAnimationFrame);
}

async function flushSceneFrame<T>(pending: Promise<T>): Promise<T> {
  vi.advanceTimersByTime(0);
  return pending;
}

function flushBufferedNodes(): void {
  vi.advanceTimersByTime(250);
}

describe('graph-scene-runtime', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockRenderState.renderCount = 0;
    mockRenderState.patchCount = 0;
    vi.stubGlobal('requestAnimationFrame', ((callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    }) as typeof requestAnimationFrame);
    vi.stubGlobal('cancelAnimationFrame', vi.fn());
  });

  it('removes stale click targets before patching a table node', async () => {
    useControlledSceneTimers();
    try {
      const { runtime, probeStore } = createDeps();
      const initialNode = createNode(1, 20);
      runtime.replaceAll({ nodes: [initialNode], edges: [] });

      expect(Object.keys(probeStore)).toEqual(['target-0', 'target-1']);

      const updatedNode = createNode(1, 40);
      await flushSceneFrame(
        runtime.applyGraphDelta({
          clear: 0,
          nodesAdded: [],
          nodesUpdated: [updatedNode],
          nodesRemoved: [],
          edgesAdded: [],
          edgesRemoved: [],
        } as any),
      );
      flushBufferedNodes();

      expect(Object.keys(probeStore)).toEqual(['target-2', 'target-3']);
    } finally {
      vi.useRealTimers();
    }
  });

  it('marks table cell patch target as dirty without an edge change', async () => {
    useControlledSceneTimers();
    try {
      const { runtime } = createDeps();
      runtime.replaceAll({ nodes: [createNode(1, 20)], edges: [] });
      mockRenderState.patchCount = 0;

      await flushSceneFrame(
        runtime.applyGraphDelta({
          clear: 0,
          nodesAdded: [],
          nodesUpdated: [],
          nodesRemoved: [],
          edgesAdded: [],
          edgesRemoved: [],
          tableCellPatches: [
            { tableRenderHandle: 1, rowIndex: 0, columnIndex: 0, cell: { text: 'ada' } },
          ],
        } as any),
      );
      flushBufferedNodes();

      expect(mockRenderState.patchCount).toBe(1);
    } finally {
      vi.useRealTimers();
    }
  });

  it('coalesces pending table patches without falling back to a full scene clear', async () => {
    useControlledSceneTimers();
    try {
      const { runtime } = createDeps();
      runtime.replaceAll({ nodes: [createNode(1, 20)], edges: [] });
      mockRenderState.renderCount = 0;
      mockRenderState.patchCount = 0;

      const firstApplied = runtime.applyGraphDelta({
        clear: 0,
        nodesAdded: [],
        nodesUpdated: [],
        nodesRemoved: [],
        edgesAdded: [],
        edgesRemoved: [],
        tableCellPatches: [
          { tableRenderHandle: 1, rowIndex: 0, columnIndex: 0, cell: { text: 'ada' } },
        ],
      } as unknown as Parameters<typeof runtime.applyGraphDelta>[0]);
      const secondApplied = runtime.applyGraphDelta({
        clear: 0,
        nodesAdded: [],
        nodesUpdated: [],
        nodesRemoved: [],
        edgesAdded: [],
        edgesRemoved: [],
        tableCellPatches: [
          { tableRenderHandle: 1, rowIndex: 1, columnIndex: 0, cell: { text: 'bea' } },
        ],
      } as unknown as Parameters<typeof runtime.applyGraphDelta>[0]);

      await flushSceneFrame(firstApplied);
      await secondApplied;
      flushBufferedNodes();

      expect(mockRenderState.renderCount).toBe(0);
      expect(mockRenderState.patchCount).toBe(1);
    } finally {
      vi.useRealTimers();
    }
  });


  it('re-renders nodes whose bounds arrive as layout patches', async () => {
    useControlledSceneTimers();
    try {
      const { runtime, nodeBoxMap } = createDeps();
      runtime.replaceAll({ nodes: [createNode(1, 20)], edges: [] });
      expect(nodeBoxMap.get(1)?.x).toBe(20);

      await flushSceneFrame(
        runtime.applyGraphDelta({
          clear: 0,
          nodesAdded: [],
          nodesUpdated: [],
          nodesRemoved: [],
          edgesAdded: [],
          edgesRemoved: [],
          layoutPatches: [
            {
              kind: 'nodeBoundsUpdated',
              renderHandle: 1,
              boxArgs: { x: 80, y: 20, width: 200, height: 80, cornerRadius: 8 },
            },
          ],
        } as any),
      );
      flushBufferedNodes();

      expect(nodeBoxMap.get(1)?.x).toBe(80);
    } finally {
      vi.useRealTimers();
    }
  });

  it('re-applies auto position when a full rebuild delta clears the scene', async () => {
    const { runtime, leafer, setLastAutoOffset } = createDeps({ canvasPadding: 12 });
    runtime.replaceAll({ nodes: [createNode(1, 30)], edges: [] });

    expect(leafer.zoomLayer).toEqual({ x: -18, y: -8 });
    expect(setLastAutoOffset).toHaveBeenLastCalledWith({ x: -18, y: -8 });

    leafer.zoomLayer.x = -18;
    leafer.zoomLayer.y = -8;
    await runtime.applyGraphDelta({
      clear: 1,
      nodesAdded: [createNode(2, 50)],
      nodesUpdated: [],
      nodesRemoved: [],
      edgesAdded: [],
      edgesRemoved: [],
    } as any);

    expect(leafer.zoomLayer).toEqual({ x: -38, y: -8 });
    expect(setLastAutoOffset).toHaveBeenLastCalledWith({ x: -38, y: -8 });
  });

  it('requests a Leafer repaint after a full rebuild clear before viewport interaction', async () => {
    const { runtime, updateLeafer } = createDeps();

    await runtime.applyGraphDelta({
      clear: 1,
      nodesAdded: [createNode(3, 20)],
      nodesUpdated: [],
      nodesRemoved: [],
      edgesAdded: [],
      edgesRemoved: [],
    } as any);

    expect(updateLeafer).toHaveBeenCalled();
  });

  it('does not render buffered partial nodes after a full rebuild clear', async () => {
    useControlledSceneTimers();
    try {
      const { runtime, probeStore } = createDeps();
      const partialNode = createNode(1, 20);
      const finalNode = createNode(2, 50);

      const partialApplied = runtime.applyGraphDelta({
        clear: 0,
        nodesAdded: [partialNode],
        nodesUpdated: [],
        nodesRemoved: [],
        edgesAdded: [],
        edgesRemoved: [],
      } as any);
      await flushSceneFrame(partialApplied);

      expect(Object.values(probeStore).map((entry: any) => entry.cell.text)).toEqual([]);

      const finalApplied = runtime.applyGraphDelta({
        clear: 1,
        nodesAdded: [finalNode],
        nodesUpdated: [],
        nodesRemoved: [],
        edgesAdded: [],
        edgesRemoved: [],
      } as any);
      await flushSceneFrame(finalApplied);

      flushBufferedNodes();

      expect(Object.values(probeStore).map((entry: any) => entry.cell.text).sort()).toEqual([
        'initial-2',
        'node-2',
      ]);
      expect(mockRenderState.renderCount).toBe(1);
    } finally {
      vi.useRealTimers();
    }
  });
});
