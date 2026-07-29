// @vitest-environment jsdom

import { describe, expect, it, vi } from 'vitest';
import { PathSegTag } from '@core-wasm/index';

const mocks = vi.hoisted(() => ({
  queryPathValue: vi.fn(),
  queryNodePreview: vi.fn(async () => ({ status: 'ready', data: { semType: 4 } })),
  cacheClear: vi.fn(),
  prepareGraph: vi.fn(),
  destroyRuntime: vi.fn(),
  renderGraph: vi.fn(),
}));

vi.mock('../../../services/SnapshotProjectionService', () => ({
  queryPathValue: mocks.queryPathValue,
  queryNodePreview: mocks.queryNodePreview,
}));

vi.mock('../graph-subgraph-workspace', () => ({
  createSubgraphWorkspaceGraphCache: () => ({
    clear: mocks.cacheClear,
    prepareGraph: mocks.prepareGraph,
  }),
  destroySubgraphWorkspaceRuntime: mocks.destroyRuntime,
  formatSubgraphWorkspacePath: (path: Array<unknown>) => (path.length ? '$.value' : '$'),
  renderSubgraphWorkspaceGraph: mocks.renderGraph,
  shouldIgnoreSubgraphOpenCell: () => false,
  shouldOpenSubgraphWorkspaceContent: (value: { valueType?: string }) =>
    value.valueType !== 'object' && value.valueType !== 'array',
  buildSubgraphWorkspaceRenderSignature: () => 'render-config',
}));

import { createSubgraphWorkspaceController } from './controller';

function keyPath(key: string) {
  return [{ tag: PathSegTag.KEY, key: key as any, index: 0 }];
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((nextResolve) => {
    resolve = nextResolve;
  });
  return { promise, resolve };
}

function readyPathValue(valueType = 'number', displayText = '1') {
  return { status: 'ready', data: { valueType, displayText } };
}

function createController(overrides: Record<string, unknown> = {}) {
  const states: unknown[] = [];
  const controller = createSubgraphWorkspaceController({
    defaultHeightPx: 220,
    getActiveSnapshotId: () => 'snapshot-active' as any,
    getWorkspaceSnapshotId: () => 'snapshot-workspace' as any,
    getDocumentKey: () => 'document-1',
    getLanguageId: () => 'json' as any,
    getRevision: () => 1,
    getRenderConfig: () => ({}) as any,
    getEnableNest: () => false,
    getReadonly: () => false,
    getShellHeight: () => 800,
    getConstructors: () => ({}),
    getValueTypeToSemType: () => ({}),
    inferGraphPaths: vi.fn(),
    clearSearchHighlight: vi.fn(),
    clearActiveGraphSelection: vi.fn(),
    emitReveal: vi.fn(),
    handleError: vi.fn(),
    applyStructuredValueEdit: vi.fn(async () => true),
    waitForCommittedDocument: vi.fn(async () => true),
    markSubgraphRequested: vi.fn(),
    markSubgraphMaterialized: vi.fn(),
    bindGraphEditorLifecycle: vi.fn(),
    bindPointerClick: vi.fn(),
    getMoveEventName: () => undefined,
    bindVerticalScrollGesture: vi.fn(),
    bindPointerDown: vi.fn(),
    getPointFromEvent: vi.fn(),
    resolveInteractiveCellPath: vi.fn(async (_cell, path) => path),
    onState: (state) => states.push(state),
    ...overrides,
  } as any);
  return { controller, states };
}

describe('Subgraph Workspace Module', () => {
  it('binds Content Pane reads to the active Workspace snapshot', async () => {
    mocks.queryPathValue.mockResolvedValueOnce(readyPathValue());
    const { controller } = createController();

    await controller.openPath(keyPath('count'), -1);

    expect(mocks.queryPathValue).toHaveBeenCalledWith({
      documentKey: 'document-1',
      snapshotId: 'snapshot-workspace',
      path: keyPath('count'),
    });
    expect(mocks.queryNodePreview).toHaveBeenCalledWith({
      documentKey: 'document-1',
      snapshotId: 'snapshot-workspace',
      path: keyPath('count'),
    });
    expect(controller.getChain()[0]).toMatchObject({ kind: 'content', status: 'ready' });
  });

  it('drops a stale open result when a later path wins', async () => {
    const first = deferred<ReturnType<typeof readyPathValue>>();
    const second = deferred<ReturnType<typeof readyPathValue>>();
    mocks.queryPathValue.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
    const { controller } = createController();

    const openA = controller.openPath(keyPath('a'), -1);
    const openB = controller.openPath(keyPath('b'), -1);
    second.resolve(readyPathValue());
    await openB;
    first.resolve(readyPathValue());
    await openA;

    expect(controller.getChain()).toHaveLength(1);
    expect(controller.getChain()[0]?.path).toEqual(keyPath('b'));
  });

  it('invalidates the projection cache only when the projection context changes', async () => {
    mocks.queryPathValue.mockResolvedValue(readyPathValue());
    const { controller } = createController();
    await controller.openPath(keyPath('count'), -1);
    const input = {
      documentKey: 'document-1',
      languageId: 'json' as any,
      revision: 1,
      graphAppliedRevision: 1,
      snapshotId: 'snapshot-workspace' as any,
      enableNest: false,
      renderConfig: {} as any,
    };

    await controller.syncProjection(input);
    await controller.syncProjection(input);
    await controller.syncProjection({ ...input, snapshotId: 'snapshot-next' as any });

    expect(mocks.cacheClear).toHaveBeenCalledTimes(2);
  });

  it('keeps only the latest queued Content Pane draft', async () => {
    mocks.queryPathValue.mockResolvedValueOnce(readyPathValue('number'));
    const commit = deferred<boolean>();
    const applyStructuredValueEdit = vi.fn((_intent: unknown) => commit.promise);
    const { controller } = createController({ applyStructuredValueEdit });
    await controller.openPath(keyPath('count'), -1);
    const pane = controller.getChain()[0]!;

    const first = controller.commitValueEdit(pane, '2');
    await Promise.resolve();
    const second = controller.commitValueEdit(pane, '3');
    const third = controller.commitValueEdit(pane, '4');
    commit.resolve(true);
    await Promise.all([first, second, third]);

    expect(applyStructuredValueEdit.mock.calls.map((call) => (call[0] as { raw: string }).raw)).toEqual(['2', '4']);
  });

  it('does not plan the latest queued draft until the prior Commit Transaction is terminal', async () => {
    mocks.queryPathValue.mockResolvedValueOnce(readyPathValue('number'));
    const commit = deferred<boolean>();
    const terminal = deferred<boolean>();
    const applyStructuredValueEdit = vi.fn((_intent: unknown) => commit.promise);
    const waitForCommittedDocument = vi.fn(() => terminal.promise);
    const { controller } = createController({ applyStructuredValueEdit, waitForCommittedDocument });
    await controller.openPath(keyPath('count'), -1);
    const pane = controller.getChain()[0]!;

    const first = controller.commitValueEdit(pane, '2');
    await Promise.resolve();
    const second = controller.commitValueEdit(pane, '3');
    commit.resolve(true);
    await Promise.resolve();
    expect(applyStructuredValueEdit).toHaveBeenCalledTimes(1);
    terminal.resolve(true);
    await Promise.all([first, second]);

    expect(applyStructuredValueEdit.mock.calls.map((call) => (call[0] as { raw: string }).raw)).toEqual(['2', '3']);
  });

  it('preserves a string pane literal and the projection snapshot in its structured intent', async () => {
    mocks.queryPathValue.mockResolvedValueOnce(readyPathValue('string', '"6837"'));
    const applyStructuredValueEdit = vi.fn(async () => true);
    const { controller } = createController({ applyStructuredValueEdit });
    await controller.openPath(keyPath('duration'), -1);

    await controller.commitValueEdit(controller.getChain()[0]!, '"42"');

    expect(applyStructuredValueEdit).toHaveBeenCalledWith(expect.objectContaining({
      raw: '"42"',
      valueType: 'string',
      snapshotId: 'snapshot-workspace',
    }));
  });

  it('releases pane runtimes and transient state on reset and dispose', async () => {
    mocks.queryPathValue.mockResolvedValueOnce(readyPathValue('object', '{1}'));
    mocks.prepareGraph.mockResolvedValueOnce({ pathKey: 'graph', path: keyPath('graph'), nodes: [], edges: [] });
    mocks.renderGraph.mockResolvedValueOnce({ mount: document.createElement('div'), tableRuntimes: [] });
    const { controller } = createController();
    const host = document.createElement('div');
    const action = controller.hostAction(host, 'k:graph');

    await controller.openPath(keyPath('graph'), -1);
    controller.reset();
    controller.dispose();
    action.destroy();

    expect(mocks.destroyRuntime).toHaveBeenCalled();
    expect(controller.getChain()).toEqual([]);
  });

  it('releases a stale runtime that finishes after reset', async () => {
    mocks.queryPathValue.mockResolvedValueOnce(readyPathValue('object', '{1}'));
    mocks.prepareGraph.mockResolvedValueOnce({ pathKey: 'graph', path: keyPath('graph'), nodes: [], edges: [] });
    const lateRuntime = { mount: document.createElement('div'), tableRuntimes: [] };
    const render = deferred<typeof lateRuntime | null>();
    mocks.renderGraph.mockReturnValueOnce(render.promise);
    const { controller } = createController();
    controller.hostAction(document.createElement('div'), 'k:graph');

    const open = controller.openPath(keyPath('graph'), -1);
    await vi.waitFor(() => expect(mocks.renderGraph).toHaveBeenCalledTimes(1));
    controller.reset();
    render.resolve(lateRuntime);
    await open;

    expect(mocks.destroyRuntime).toHaveBeenCalledWith(expect.objectContaining({ mount: lateRuntime.mount }));
    expect(controller.hasRuntime('k:graph')).toBe(false);
  });
});
