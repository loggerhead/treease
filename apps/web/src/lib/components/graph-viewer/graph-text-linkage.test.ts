import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('../../services/TreePathService', () => ({
  resolveTreePathFromTextResult: vi.fn(),
}));

vi.mock('../../services/SnapshotProjectionService', () => ({
  queryPathValue: vi.fn(),
}));

import { createGraphTextLinkageController } from './graph-text-linkage';
import { buildPathKey } from '../../graph/graph-viewer-path';
import { resolveTreePathFromTextResult } from '../../services/TreePathService';
import { queryPathValue } from '../../services/SnapshotProjectionService';

class MockBox {
  fill: string | undefined;
  stroke: string | undefined;
  selected = false;
  selectedStyle: { fill?: string; stroke?: string } | undefined;

  constructor(fill = 'transparent') {
    this.fill = fill;
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((nextResolve) => {
    resolve = nextResolve;
  });
  return { promise, resolve };
}

function createBaseDeps(overrides: Record<string, any> = {}) {
  return {
    getDocumentKey: () => 'cache',
    getSourceText: () => '',
    getLanguageId: () => 'json' as const,
    getActiveSnapshotId: () => 7,
    getEnableNest: () => true,
    getRenderConfig: () =>
      ({
        colors: {
          table: {
            rowBackground: '#fff',
            rowBorder: '#ddd',
            hoverRowBackground: '#eee',
            hoverCellBackground: '#ff0',
          },
        },
      }) as any,
    getNodeDataMap: () => new Map(),
    getNodeBoxMap: () => new Map(),
    getCellBoxByPathMap: () => new Map(),
    getTableCellAnchorMap: () => new Map(),
    getPathKeyToRenderHandleMap: () => new Map(),
    getClickTargetProbes: () => [],
    setGraphHighlightTestState: vi.fn(),
    setGraphRevealTestState: vi.fn(),
    setGraphRowScrollTestState: vi.fn(),
    scrollTableCellAnchorIntoView: vi.fn(),
    buildPathSegFromCell: () => null,
    upsertCellEntry: vi.fn(),
    centerOnBox: () => false,
    centerOnNode: vi.fn(),
    updateLeafer: vi.fn(),
    updateActiveTempModel: vi.fn(),
    getEditorRevision: () => 0,
    getGraphAppliedRevision: () => 0,
    dispatchReveal: vi.fn(),
    handleError: vi.fn(),
    ...overrides,
  };
}

describe('graph-text-linkage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(queryPathValue).mockResolvedValue({ status: 'ready', data: { value: 'present' } } as any);
  });

  it('preserves a breadcrumb reveal as a viewport-navigation source', () => {
    const path = [{ tag: 0, key: 'preview', index: 0 }] as any[];
    const updateActiveTempModel = vi.fn();
    const controller = createGraphTextLinkageController(createBaseDeps({ updateActiveTempModel }));

    controller.emitReveal(path, 'value', 'breadcrumb');

    const update = updateActiveTempModel.mock.calls[0][0];
    expect(update({} as any).graphHighlight).toMatchObject({
      path,
      target: 'value',
      source: 'breadcrumb',
    });
  });

  it('uses table anchor index to materialize an offscreen search cell before highlight', async () => {
    const path = [
      { tag: 0, key: 'rows', index: 0 },
      { tag: 1, key: '', index: 42 },
      { tag: 0, key: 'name', index: 0 },
    ] as any[];
    const pathKey = buildPathKey(path);
    const highlightedBox = new MockBox();
    const cellMap = new Map<string, any>();
    const scrollTableCellAnchorIntoView = vi.fn(() => {
      cellMap.set(pathKey, {
        value: highlightedBox,
        row: new MockBox('#fff'),
      });
      return true;
    });
    const setGraphHighlightTestState = vi.fn();

    const controller = createGraphTextLinkageController(
      createBaseDeps({
        getCellBoxByPathMap: () => cellMap,
        getTableCellAnchorMap: () =>
          new Map([
            [
              pathKey,
              {
                nodeId: 9,
                rowIndex: 42,
                cellIndex: 1,
                target: 'value',
              },
            ],
          ]),
        scrollTableCellAnchorIntoView,
        setGraphHighlightTestState,
      }),
    );

    const revealed = await controller.revealPath(path, { target: 'value', navigate: true });
    if (revealed) controller.emitReveal(path, 'value', 'search');

    expect(scrollTableCellAnchorIntoView).toHaveBeenCalledWith({
      nodeId: 9,
      rowIndex: 42,
      cellIndex: 1,
      target: 'value',
    });
    expect(setGraphHighlightTestState).toHaveBeenCalledWith(path, 'value', highlightedBox);
  });

  it('scrolls the table row into view when revealPath navigates', async () => {
    const path = ['$', 'library', 'book', 0, 'title'] as any[];
    const pathKey = buildPathKey(path);
    const scrollTo = vi.fn();
    const setGraphRowScrollTestState = vi.fn();
    const updateLeafer = vi.fn();
    const row = new MockBox('#fff') as MockBox & { y: number; height: number };
    row.y = 120;
    row.height = 20;
    const cellMap = new Map<string, any>([
      [
        pathKey,
        {
          value: new MockBox(),
          row,
          scrollOwner: { scrollTo },
          bodyHeight: 80,
          contentHeight: 240,
        },
      ],
    ]);

    const controller = createGraphTextLinkageController({
      getDocumentKey: () => 'cache',
      getSourceText: () => '',
      getLanguageId: () => 'json',
      getActiveSnapshotId: () => 7,
      getEnableNest: () => true,
      getRenderConfig: () =>
        ({
          colors: {
            table: {
              rowBackground: '#fff',
              rowBorder: '#ddd',
              hoverRowBackground: '#eee',
              hoverCellBackground: '#ff0',
            },
          },
        }) as any,
      getNodeDataMap: () => new Map(),
      getNodeBoxMap: () => new Map(),
      getCellBoxByPathMap: () => cellMap,
      getPathKeyToRenderHandleMap: () => new Map(),
      getClickTargetProbes: () => [],
      setGraphHighlightTestState: vi.fn(),
      setGraphRevealTestState: vi.fn(),
      setGraphRowScrollTestState,
      buildPathSegFromCell: () => null,
      upsertCellEntry: (map, cell, updater) => {
        const entry = map.get(pathKey) ?? {};
        updater(entry);
        map.set(pathKey, entry);
      },
      centerOnBox: () => false,
      centerOnNode: vi.fn(),
      updateLeafer,
      updateActiveTempModel: vi.fn(),
      getEditorRevision: () => 0,
      getGraphAppliedRevision: () => 0,
      dispatchReveal: vi.fn(),
      handleError: vi.fn(),
    });

    controller.revealPath(path, { target: 'value', navigate: true });
    await Promise.resolve();

    expect(scrollTo).toHaveBeenCalledWith({ x: 0, y: 90 });
    expect(setGraphRowScrollTestState).toHaveBeenCalledWith(path, 90);
    expect(updateLeafer).toHaveBeenCalled();
  });

  it('refreshes active highlight after a reused slot is rebound to another row', async () => {
    const highlightedBox = new MockBox();
    const path = ['$','library','book',0,'title'] as any[];
    const pathKey = buildPathKey(path);
    const cellMap = new Map<string, any>([
      [
        pathKey,
        {
          value: highlightedBox,
          row: new MockBox('#fff'),
        },
      ],
    ]);

    const controller = createGraphTextLinkageController({
      getDocumentKey: () => 'cache',
      getSourceText: () => '',
      getLanguageId: () => 'json',
      getActiveSnapshotId: () => 7,
      getEnableNest: () => true,
      getRenderConfig: () =>
        ({
          colors: {
            table: {
              rowBackground: '#fff',
              rowBorder: '#ddd',
              hoverRowBackground: '#eee',
              hoverCellBackground: '#ff0',
            },
          },
        }) as any,
      getNodeDataMap: () => new Map(),
      getNodeBoxMap: () => new Map(),
      getCellBoxByPathMap: () => cellMap,
      getPathKeyToRenderHandleMap: () => new Map(),
      getClickTargetProbes: () => [],
      setGraphHighlightTestState: vi.fn(),
      setGraphRevealTestState: vi.fn(),
      setGraphRowScrollTestState: vi.fn(),
      buildPathSegFromCell: () => null,
      upsertCellEntry: (map, cell, updater) => {
        const entry = map.get(pathKey) ?? {};
        updater(entry);
        map.set(pathKey, entry);
      },
      centerOnBox: () => false,
      centerOnNode: vi.fn(),
      updateLeafer: vi.fn(),
      updateActiveTempModel: vi.fn(),
      getEditorRevision: () => 0,
      getGraphAppliedRevision: () => 0,
      dispatchReveal: vi.fn(),
      handleError: vi.fn(),
    });

    controller.revealPath(path, { target: 'value', navigate: false });
    await Promise.resolve();

    cellMap.delete(pathKey);
    cellMap.set('$.library.book[46].title', {
      value: highlightedBox,
      row: new MockBox('#fff'),
    });

    controller.refreshActiveHighlight();
    expect(highlightedBox.selected).toBe(false);
  });

  it('preserves the requested highlight path when local render bindings are temporarily missing', async () => {
    const path = ['$', 'library', 'book', 0, 'title'] as any[];
    const pathKey = buildPathKey(path);
    const updateActiveTempModel = vi.fn();
    const highlightedBox = new MockBox();
    const cellMap = new Map<string, any>();

    const controller = createGraphTextLinkageController({
      getDocumentKey: () => 'cache',
      getSourceText: () => '',
      getLanguageId: () => 'json',
      getActiveSnapshotId: () => 7,
      getEnableNest: () => true,
      getRenderConfig: () =>
        ({
          colors: {
            table: {
              rowBackground: '#fff',
              rowBorder: '#ddd',
              hoverRowBackground: '#eee',
              hoverCellBackground: '#ff0',
            },
          },
        }) as any,
      getNodeDataMap: () => new Map(),
      getNodeBoxMap: () => new Map(),
      getCellBoxByPathMap: () => cellMap,
      getPathKeyToRenderHandleMap: () => new Map(),
      getClickTargetProbes: () => [],
      setGraphHighlightTestState: vi.fn(),
      setGraphRevealTestState: vi.fn(),
      setGraphRowScrollTestState: vi.fn(),
      buildPathSegFromCell: () => null,
      upsertCellEntry: vi.fn(),
      centerOnBox: () => false,
      centerOnNode: vi.fn(),
      updateLeafer: vi.fn(),
      updateActiveTempModel,
      getEditorRevision: () => 0,
      getGraphAppliedRevision: () => 0,
      dispatchReveal: vi.fn(),
      handleError: vi.fn(),
    });

    controller.revealPath(path, { target: 'value', navigate: false });
    await Promise.resolve();

    expect(updateActiveTempModel).not.toHaveBeenCalled();

    cellMap.set(pathKey, {
      value: highlightedBox,
      row: new MockBox('#fff'),
    });

    controller.refreshActiveHighlight();
    expect(updateActiveTempModel).not.toHaveBeenCalled();
  });

  it('clears retained logical selection only when the committed snapshot proves its path is absent', async () => {
    const path = ['$', 'library', 'missing'] as any[];
    const pathKey = buildPathKey(path);
    const highlightedBox = new MockBox();
    const updateActiveTempModel = vi.fn();
    const setGraphHighlightTestState = vi.fn();
    const controller = createGraphTextLinkageController(createBaseDeps({
      getCellBoxByPathMap: () => new Map([[pathKey, {
        value: highlightedBox,
        row: new MockBox('#fff'),
      }]]),
      updateActiveTempModel,
      setGraphHighlightTestState,
    }));

    await controller.revealPath(path, { target: 'value', navigate: false });
    vi.mocked(queryPathValue).mockResolvedValue({ status: 'ready', data: null } as any);
    await controller.reconcileActiveHighlight({
      documentKey: 'cache',
      snapshotId: 7 as any,
      graphAppliedRevision: 0,
    });

    expect(highlightedBox.selected).toBe(false);
    expect(setGraphHighlightTestState).toHaveBeenLastCalledWith(null);
    expect(updateActiveTempModel).toHaveBeenCalledTimes(1);
    const update = updateActiveTempModel.mock.calls[0][0];
    expect(update({
      treePath: path,
      graphHighlight: { path, target: 'value', revision: 0, source: 'graph' },
    })).toMatchObject({ treePath: [], graphHighlight: null });
  });

  it('does not clear retained logical selection for an unready snapshot read', async () => {
    const path = ['$', 'library', 'book'] as any[];
    const pathKey = buildPathKey(path);
    const highlightedBox = new MockBox();
    const updateActiveTempModel = vi.fn();
    const controller = createGraphTextLinkageController(createBaseDeps({
      getCellBoxByPathMap: () => new Map([[pathKey, {
        value: highlightedBox,
        row: new MockBox('#fff'),
      }]]),
      updateActiveTempModel,
    }));

    await controller.revealPath(path, { target: 'value', navigate: false });
    vi.mocked(queryPathValue).mockResolvedValue({ status: 'snapshotNotReady' } as any);
    await controller.reconcileActiveHighlight({
      documentKey: 'cache',
      snapshotId: 7 as any,
      graphAppliedRevision: 0,
    });

    expect(updateActiveTempModel).not.toHaveBeenCalled();
  });

  it('discards a path-absence result that became stale while the snapshot changed', async () => {
    const path = ['$', 'library', 'book'] as any[];
    const pathKey = buildPathKey(path);
    const highlightedBox = new MockBox();
    const updateActiveTempModel = vi.fn();
    let activeSnapshot = 7;
    let resolveQuery: ((value: any) => void) | undefined;
    vi.mocked(queryPathValue).mockReturnValue(new Promise((resolve) => {
      resolveQuery = resolve;
    }) as any);
    const controller = createGraphTextLinkageController(createBaseDeps({
      getActiveSnapshotId: () => activeSnapshot,
      getCellBoxByPathMap: () => new Map([[pathKey, {
        value: highlightedBox,
        row: new MockBox('#fff'),
      }]]),
      updateActiveTempModel,
    }));

    await controller.revealPath(path, { target: 'value', navigate: false });
    const reconciliation = controller.reconcileActiveHighlight({
      documentKey: 'cache',
      snapshotId: 7 as any,
      graphAppliedRevision: 0,
    });
    activeSnapshot = 8;
    resolveQuery?.({ status: 'ready', data: null });
    await reconciliation;

    expect(updateActiveTempModel).not.toHaveBeenCalled();
  });

  it('treats renderHandle 0 as a valid graph node handle', async () => {
    const path = [{ tag: 0, key: 'object', index: 0 }] as any[];
    const pathKey = buildPathKey(path);
    const nodeBox = new MockBox();
    const controller = createGraphTextLinkageController({
      getDocumentKey: () => 'cache',
      getSourceText: () => '',
      getLanguageId: () => 'json',
      getActiveSnapshotId: () => 7,
      getEnableNest: () => true,
      getRenderConfig: () =>
        ({
          colors: {
            table: {
              rowBackground: '#fff',
              rowBorder: '#ddd',
              hoverRowBackground: '#eee',
              hoverCellBackground: '#ff0',
            },
          },
        }) as any,
      getNodeDataMap: () => new Map([[0, { renderHandle: 0, path } as any]]),
      getNodeBoxMap: () => new Map([[0, nodeBox as any]]),
      getCellBoxByPathMap: () => new Map(),
      getPathKeyToRenderHandleMap: () => new Map([[pathKey, 0]]),
      getClickTargetProbes: () => [],
      setGraphHighlightTestState: vi.fn(),
      setGraphRevealTestState: vi.fn(),
      setGraphRowScrollTestState: vi.fn(),
      buildPathSegFromCell: () => null,
      upsertCellEntry: vi.fn(),
      centerOnBox: () => false,
      centerOnNode: vi.fn(),
      updateLeafer: vi.fn(),
      updateActiveTempModel: vi.fn(),
      getEditorRevision: () => 0,
      getGraphAppliedRevision: () => 0,
      dispatchReveal: vi.fn(),
      handleError: vi.fn(),
    });

    controller.revealPath(path, { target: 'node', navigate: false });
    await Promise.resolve();

  });

  it('drops a stale materialized reveal before it can publish a visual target', async () => {
    const firstPath = [{ tag: 0, key: 'first', index: 0 }] as any[];
    const secondPath = [{ tag: 0, key: 'second', index: 0 }] as any[];
    const firstMaterialization = deferred<boolean>();
    const setGraphHighlightTestState = vi.fn();
    const controller = createGraphTextLinkageController(createBaseDeps({
      getNodeDataMap: () => new Map([
        [1, { renderHandle: 1, path: firstPath }],
        [2, { renderHandle: 2, path: secondPath }],
      ]),
      getPathKeyToRenderHandleMap: () => new Map([
        [buildPathKey(firstPath), 1],
        [buildPathKey(secondPath), 2],
      ]),
      getCellBoxByPathMap: () => new Map([
        [buildPathKey(firstPath), { value: new MockBox(), row: new MockBox() }],
        [buildPathKey(secondPath), { value: new MockBox(), row: new MockBox() }],
      ]),
      materializeTarget: (handle: number) => handle === 1 ? firstMaterialization.promise : Promise.resolve(true),
      setGraphHighlightTestState,
    }));

    const first = controller.revealPath(firstPath, { target: 'value', navigate: true });
    await Promise.resolve();
    const second = controller.revealPath(secondPath, { target: 'value', navigate: true });
    firstMaterialization.resolve(true);

    await expect(first).resolves.toBe(false);
    await expect(second).resolves.toBe(true);
    expect(setGraphHighlightTestState).toHaveBeenLastCalledWith(secondPath, 'value', expect.any(MockBox));
  });

  it('derives headerless table row scroll fallback directly from the target path', async () => {
    const visiblePath = [{ tag: 0, key: 'rows', index: 0 }, { tag: 1, key: '', index: 0 }] as any[];
    const targetPath = [{ tag: 0, key: 'rows', index: 0 }, { tag: 1, key: '', index: 172 }] as any[];
    const visiblePathKey = buildPathKey(visiblePath);
    const scrollTo = vi.fn();
    const setGraphRowScrollTestState = vi.fn();
    const cellMap = new Map<string, any>([
      [
        visiblePathKey,
        {
          value: new MockBox(),
          row: new MockBox('#fff'),
          cell: { path: visiblePath },
          scrollOwner: { scrollTo },
          bodyHeight: 80,
          contentHeight: 4_000,
        },
      ],
    ]);

    const controller = createGraphTextLinkageController({
      getDocumentKey: () => 'cache',
      getSourceText: () => '',
      getLanguageId: () => 'json',
      getActiveSnapshotId: () => 7,
      getEnableNest: () => true,
      getRenderConfig: () =>
        ({
          colors: {
            table: {
              rowBackground: '#fff',
              rowBorder: '#ddd',
              hoverRowBackground: '#eee',
              hoverCellBackground: '#ff0',
            },
          },
        }) as any,
      getNodeDataMap: () => new Map(),
      getNodeBoxMap: () => new Map(),
      getCellBoxByPathMap: () => cellMap,
      getPathKeyToRenderHandleMap: () => new Map(),
      getClickTargetProbes: () => [],
      setGraphHighlightTestState: vi.fn(),
      setGraphRevealTestState: vi.fn(),
      setGraphRowScrollTestState,
      buildPathSegFromCell: () => null,
      upsertCellEntry: vi.fn(),
      centerOnBox: () => false,
      centerOnNode: vi.fn(),
      updateLeafer: vi.fn(),
      updateActiveTempModel: vi.fn(),
      getEditorRevision: () => 0,
      getGraphAppliedRevision: () => 0,
      dispatchReveal: vi.fn(),
      handleError: vi.fn(),
    });

    controller.revealPath(targetPath, { target: 'value', navigate: true });
    await Promise.resolve();

    expect(scrollTo).toHaveBeenCalled();
    expect(setGraphRowScrollTestState).toHaveBeenCalledWith(targetPath, expect.any(Number));
  });

  it('uses table row geometry for indexed fallback scrolling', async () => {
    const tablePath = [{ tag: 0, key: 'rows', index: 0 }] as any[];
    const visiblePath = [...tablePath, { tag: 1, key: '', index: 0 }] as any[];
    const targetPath = [...tablePath, { tag: 1, key: '', index: 100 }, { tag: 0, key: 'name', index: 0 }] as any[];
    const visiblePathKey = buildPathKey(visiblePath);
    const scrollTo = vi.fn();
    const setGraphRowScrollTestState = vi.fn();
    const cellMap = new Map<string, any>([
      [
        visiblePathKey,
        {
          value: new MockBox(),
          row: new MockBox('#fff'),
          cell: { path: visiblePath },
          scrollOwner: { scrollTo },
          bodyHeight: 240,
          contentHeight: 4_000,
        },
      ],
    ]);
    const nodeDataMap = new Map<number, any>([
      [
        1,
        {
          renderHandle: 1,
          kind: 'table',
          path: tablePath,
          table: {
            rowHeight: 28,
            rows: Array.from({ length: 140 }, (_, index) => ({
              boxArgs: { y: 1 + index * 28, height: 28 },
              cells: [{ path: index === 0 ? visiblePath : [] }],
            })),
          },
        },
      ],
    ]);
    const pathKeyToRenderHandleMap = new Map([[buildPathKey(tablePath), 1]]);

    const controller = createGraphTextLinkageController({
      getDocumentKey: () => 'cache',
      getSourceText: () => '',
      getLanguageId: () => 'json',
      getActiveSnapshotId: () => 7,
      getEnableNest: () => true,
      getRenderConfig: () =>
        ({
          colors: {
            table: {
              rowBackground: '#fff',
              rowBorder: '#ddd',
              hoverRowBackground: '#eee',
              hoverCellBackground: '#ff0',
            },
          },
        }) as any,
      getNodeDataMap: () => nodeDataMap,
      getNodeBoxMap: () => new Map(),
      getCellBoxByPathMap: () => cellMap,
      getPathKeyToRenderHandleMap: () => pathKeyToRenderHandleMap,
      getClickTargetProbes: () => [],
      setGraphHighlightTestState: vi.fn(),
      setGraphRevealTestState: vi.fn(),
      setGraphRowScrollTestState,
      buildPathSegFromCell: () => null,
      upsertCellEntry: vi.fn(),
      centerOnBox: () => false,
      centerOnNode: vi.fn(),
      updateLeafer: vi.fn(),
      updateActiveTempModel: vi.fn(),
      getEditorRevision: () => 0,
      getGraphAppliedRevision: () => 0,
      dispatchReveal: vi.fn(),
      handleError: vi.fn(),
    });

    controller.revealPath(targetPath, { target: 'value', navigate: true });
    await Promise.resolve();

    expect(scrollTo).toHaveBeenCalledWith({ x: 0, y: 2694 });
    expect(setGraphRowScrollTestState).toHaveBeenCalledWith(targetPath, 2694);
  });

  it('preserves explicit table value cell paths during hydration', async () => {
    vi.mocked(resolveTreePathFromTextResult).mockImplementation(async (_text, row, column) => {
      if (row === 10 && column === 2) {
        return { status: 'ready', data: [{ tag: 0, key: 'table_with_header', index: 0 }] as any[] };
      }
      if (row === 11 && column === 2) {
        return {
          status: 'ready',
          data: [
            { tag: 0, key: 'table_with_header', index: 0 },
            { tag: 1, key: '', index: 0 },
          ] as any[],
        };
      }
      if (row === 11 && column === 8) {
        return {
          status: 'ready',
          data: [
            { tag: 0, key: 'table_with_header', index: 0 },
            { tag: 1, key: '', index: 0 },
            { tag: 0, key: 'h1', index: 0 },
          ] as any[],
        };
      }
      return { status: 'snapshotNotReady' };
    });

    const valueCellPath = [
      { tag: 0, key: 'table_with_header', index: 0 },
      { tag: 1, key: '', index: 0 },
      { tag: 0, key: 'h1', index: 0 },
    ] as any[];
    const rowPath = [
      { tag: 0, key: 'table_with_header', index: 0 },
      { tag: 1, key: '', index: 0 },
    ] as any[];
    const nodes = [
      {
        renderHandle: 1,
        kind: 'table',
        path: [{ tag: 0, key: 'table_with_header', index: 0 }],
        meta: {
          valueType: 'object',
          path: [],
        },
        rows: [
          {
            cells: [
              {
                text: '[0]',
                path: [...rowPath],
                isTableCell: true,
                isIndex: true,
              },
              {
                text: '11',
                path: [...valueCellPath],
                isTableCell: true,
              },
            ],
          },
        ],
        table: { columns: [] },
      },
    ] as any[];

    const controller = createGraphTextLinkageController({
      getDocumentKey: () => 'cache',
      getSourceText: () => '{\n  "table_with_header": [{"h1": 11}]\n}',
      getLanguageId: () => 'json',
      getActiveSnapshotId: () => 7,
      getEnableNest: () => true,
      getRenderConfig: () =>
        ({
          colors: {
            table: {
              rowBackground: '#fff',
              rowBorder: '#ddd',
              hoverRowBackground: '#eee',
              hoverCellBackground: '#ff0',
            },
          },
        }) as any,
      getNodeDataMap: () => new Map(),
      getNodeBoxMap: () => new Map(),
      getCellBoxByPathMap: () => new Map(),
      getPathKeyToRenderHandleMap: () => new Map(),
      getClickTargetProbes: () => [],
      setGraphHighlightTestState: vi.fn(),
      setGraphRevealTestState: vi.fn(),
      setGraphRowScrollTestState: vi.fn(),
      buildPathSegFromCell: () => ({ tag: 1, key: '', index: 0 }) as any,
      upsertCellEntry: vi.fn(),
      centerOnBox: () => false,
      centerOnNode: vi.fn(),
      updateLeafer: vi.fn(),
      updateActiveTempModel: vi.fn(),
      getEditorRevision: () => 0,
      getGraphAppliedRevision: () => 0,
      dispatchReveal: vi.fn(),
      handleError: vi.fn(),
    });

    await controller.hydrateResolvedGraphPaths(nodes as any, '{\n  "table_with_header": [{"h1": 11}]\n}');

    expect(nodes[0].rows[0].cells[0].path).toEqual(rowPath);
    expect(nodes[0].rows[0].cells[1].path).toEqual(valueCellPath);
  });

  it('hydrates headerless table rows from node.table.rows for array sequences', async () => {
    const arrayPath = [{ tag: 0, key: 'items', index: 0 }] as any[];
    const rowPath = [...arrayPath, { tag: 1, key: '', index: 0 }] as any[];
    const nodes = [
      {
        renderHandle: 1,
        kind: 'table',
        path: arrayPath,
        meta: {
          valueType: 'array',
          path: [...arrayPath],
        },
        rows: [],
        table: {
          columns: [],
          headerHeight: 0,
          rows: [
            {
              cells: [
                {
                  text: '0',
                  path: [],
                  isTableCell: true,
                  isIndex: true,
                },
                {
                  text: 'alice',
                  path: [],
                  isTableCell: true,
                },
              ],
            },
          ],
        },
      },
    ] as any[];

    const controller = createGraphTextLinkageController({
      getDocumentKey: () => 'cache',
      getSourceText: () => '{\n  "items": ["alice"]\n}',
      getLanguageId: () => 'json',
      getActiveSnapshotId: () => 7,
      getEnableNest: () => true,
      getRenderConfig: () =>
        ({
          colors: {
            table: {
              rowBackground: '#fff',
              rowBorder: '#ddd',
              hoverRowBackground: '#eee',
              hoverCellBackground: '#ff0',
            },
          },
        }) as any,
      getNodeDataMap: () => new Map(),
      getNodeBoxMap: () => new Map(),
      getCellBoxByPathMap: () => new Map(),
      getPathKeyToRenderHandleMap: () => new Map(),
      getClickTargetProbes: () => [],
      setGraphHighlightTestState: vi.fn(),
      setGraphRevealTestState: vi.fn(),
      setGraphRowScrollTestState: vi.fn(),
      buildPathSegFromCell: (_cell, rowIndex) => ({ tag: 1, key: '', index: rowIndex }) as any,
      upsertCellEntry: vi.fn(),
      centerOnBox: () => false,
      centerOnNode: vi.fn(),
      updateLeafer: vi.fn(),
      updateActiveTempModel: vi.fn(),
      getEditorRevision: () => 0,
      getGraphAppliedRevision: () => 0,
      dispatchReveal: vi.fn(),
      handleError: vi.fn(),
    });

    await controller.hydrateResolvedGraphPaths(nodes as any, '{\n  "items": ["alice"]\n}');

    expect(nodes[0].table.rows[0].cells[0].path).toEqual(rowPath);
    expect(nodes[0].table.rows[0].cells[1].path).toEqual(rowPath);
  });
});
