// @vitest-environment jsdom

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { PathSegTag } from '@core-wasm/index';

const mocks = vi.hoisted(() => ({
  queryPathValue: vi.fn(),
  cacheClear: vi.fn(),
  prepareGraph: vi.fn(),
}));

vi.mock('../../../services/SnapshotProjectionService', () => ({
  queryPathValue: mocks.queryPathValue,
}));

vi.mock('../column-navigator-graph', () => ({
  createColumnNavigatorGraphCache: () => ({
    clear: mocks.cacheClear,
    prepareGraph: mocks.prepareGraph,
  }),
  buildColumnNavigatorColumnItems: (graph: { items?: unknown[] }) => graph.items ?? [],
  formatColumnNavigatorPath: (path: Array<{ key?: string; index?: number }>) =>
    path.length ? `$.${path.map((segment) => segment.key ?? `[${segment.index}]`).join('.')}` : '$',
  shouldOpenColumnNavigatorContent: (value: { valueType?: string; displayText?: string }) =>
    (value.valueType !== 'object' && value.valueType !== 'array') ||
    value.displayText === '{}' ||
    value.displayText === '[]',
  buildColumnNavigatorRenderSignature: () => 'render-config',
}));

import { buildWorkspacePathPrefixes, createColumnNavigatorController } from './controller';

function keySeg(key: string) {
  return { tag: PathSegTag.KEY, key: key as any, index: 0 };
}

function keyPath(...keys: string[]) {
  return keys.map(keySeg);
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((nextResolve) => {
    resolve = nextResolve;
  });
  return { promise, resolve };
}

function readyPathValue(
  valueType = 'number',
  displayText = '1',
  semanticTokens = { data: [0, 0, displayText.length, 4, 0], version: 1 },
) {
  return { status: 'ready', data: { valueType, sourceText: displayText, displayText, semanticTokens } };
}

function item(path: ReturnType<typeof keyPath>, valueType: 'object' | 'array' | 'string' | 'number' = 'object') {
  return {
    path,
    pathKey: path.map((segment) => `k:${segment.key}`).join('|'),
    label: path.at(-1)?.key ?? '$',
    preview: valueType === 'object' ? '{1}' : valueType === 'array' ? '[1]' : 'value',
    valueType,
    semType: 0,
    isContainer: valueType === 'object' || valueType === 'array',
  };
}

function pathKey(path: Array<{ key?: string }>): string {
  return path.map((segment) => segment.key).join('.');
}

function installDocument(values: Record<string, ReturnType<typeof readyPathValue>>, items: Record<string, unknown[]>) {
  mocks.queryPathValue.mockImplementation(async ({ path }: { path: Array<{ key?: string }> }) =>
    values[pathKey(path)] ?? { status: 'ready', data: null },
  );
  mocks.prepareGraph.mockImplementation(async (path: Array<{ key?: string }>) => ({
    path,
    pathKey: pathKey(path),
    nodes: [],
    edges: [],
    items: items[pathKey(path)] ?? [],
  }));
}

function createController(overrides: Record<string, unknown> = {}) {
  const states: any[] = [];
  const controller = createColumnNavigatorController({
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
    inferGraphPaths: vi.fn(),
    clearSearchHighlight: vi.fn(),
    clearActiveGraphSelection: vi.fn(),
    emitReveal: vi.fn(),
    handleError: vi.fn(),
    applyStructuredValueEdit: vi.fn(async () => true),
    waitForCommittedDocument: vi.fn(async () => true),
    markSubgraphRequested: vi.fn(),
    markSubgraphMaterialized: vi.fn(),
    onState: (state) => states.push(state),
    ...overrides,
  } as any);
  return { controller, states };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe('path-driven column navigator controller', () => {
  it('builds every container column from the single active path and terminates leaves in Monaco content', async () => {
    installDocument(
      {
        '': readyPathValue('object', '{2}'),
        user: readyPathValue('object', '{1}'),
        'user.name': readyPathValue('string', '"Alice"'),
      },
      {
        '': [item(keyPath('user')), item(keyPath('settings'))],
        user: [item(keyPath('user', 'name'), 'string')],
      },
    );
    const { controller } = createController();

    await controller.openPath(keyPath('user', 'name'));

    expect(controller.getActivePath()).toEqual(keyPath('user', 'name'));
    expect(controller.getChain().map((pane) => [pane.pathKey, pane.kind])).toEqual([
      ['$', 'column'],
      ['k:user', 'column'],
      ['k:user|k:name', 'content'],
    ]);
    expect(controller.getChain().at(-1)?.content?.sourceText).toBe('"Alice"');
    expect(Array.from(new Uint32Array(controller.getChain().at(-1)?.content?.semanticTokens ?? new ArrayBuffer(0)))).toEqual([
      0, 0, 7, 4, 0,
    ]);
  });

  it('keeps ancestor columns and replaces every descendant when a sibling wins', async () => {
    installDocument(
      {
        '': readyPathValue('object', '{1}'),
        user: readyPathValue('object', '{2}'),
        'user.profile': readyPathValue('object', '{1}'),
        'user.profile.name': readyPathValue('string', '"A"'),
        'user.settings': readyPathValue('object', '{1}'),
      },
      {
        '': [item(keyPath('user'))],
        user: [item(keyPath('user', 'profile')), item(keyPath('user', 'settings'))],
        'user.profile': [item(keyPath('user', 'profile', 'name'), 'string')],
        'user.settings': [item(keyPath('user', 'settings', 'theme'), 'string')],
      },
    );
    const { controller } = createController();
    await controller.openPath(keyPath('user', 'profile', 'name'));

    await controller.selectPath(keyPath('user', 'settings'));

    expect(controller.getActivePath()).toEqual(keyPath('user', 'settings'));
    expect(controller.getChain().map((pane) => [pane.pathKey, pane.kind])).toEqual([
      ['$', 'column'],
      ['k:user', 'column'],
      ['k:user|k:settings', 'column'],
      ['k:user|k:settings', 'content'],
    ]);
    expect(controller.getChain().some((pane) => pane.pathKey.includes('profile'))).toBe(false);
    expect(controller.getChain().at(-1)?.content?.sourceText).toBe('{1}');
  });

  it('binds subtree text to the detail editor while the selected container keeps its column', async () => {
    installDocument(
      {
        '': readyPathValue('object', '{1}'),
        settings: readyPathValue('object', '{1}'),
      },
      { '': [item(keyPath('settings'))], settings: [item(keyPath('settings', 'theme'), 'string')] },
    );
    const { controller } = createController();

    await controller.openPath(keyPath('settings'));

    expect(controller.getChain().map((pane) => pane.kind)).toEqual(['column', 'column', 'content']);
    expect(controller.getChain().at(-1)?.content).toMatchObject({ valueType: 'object', sourceText: '{1}' });
  });

  it('restores active paths through back and forward history', async () => {
    installDocument(
      {
        '': readyPathValue('object', '{2}'),
        alpha: readyPathValue('object', '{1}'),
        beta: readyPathValue('object', '{1}'),
      },
      {
        '': [item(keyPath('alpha')), item(keyPath('beta'))],
        alpha: [item(keyPath('alpha', 'child'), 'string')],
        beta: [item(keyPath('beta', 'child'), 'string')],
      },
    );
    const { controller, states } = createController();
    await controller.openPath(keyPath('alpha'));
    await controller.selectPath(keyPath('beta'));

    await controller.goBack();
    expect(controller.getActivePath()).toEqual(keyPath('alpha'));
    expect(states.at(-1)).toMatchObject({ canGoBack: false, canGoForward: true });

    await controller.goForward();
    expect(controller.getActivePath()).toEqual(keyPath('beta'));
  });

  it('waits for the current Monaco draft transaction before rebinding the selected path', async () => {
    installDocument(
      {
        '': readyPathValue('object', '{2}'),
        count: readyPathValue('number', '1'),
        other: readyPathValue('number', '2'),
      },
      { '': [item(keyPath('count'), 'number'), item(keyPath('other'), 'number')] },
    );
    const terminal = deferred<boolean>();
    const { controller } = createController({
      waitForCommittedDocument: vi.fn(() => terminal.promise),
    });
    await controller.openPath(keyPath('count'));
    const edit = controller.commitValueEdit(controller.getChain().at(-1)!, '3');

    const navigation = controller.selectPath(keyPath('other'));
    await Promise.resolve();
    expect(controller.getActivePath()).toEqual(keyPath('count'));

    terminal.resolve(true);
    await Promise.all([edit, navigation]);
    expect(controller.getActivePath()).toEqual(keyPath('other'));
  });

  it('keeps only the newest draft queued behind a pending commit', async () => {
    installDocument(
      { '': readyPathValue('object', '{1}'), count: readyPathValue('number', '1') },
      { '': [item(keyPath('count'), 'number')] },
    );
    const firstApply = deferred<boolean>();
    const applyStructuredValueEdit = vi.fn()
      .mockReturnValueOnce(firstApply.promise)
      .mockResolvedValue(true);
    const { controller } = createController({ applyStructuredValueEdit });
    await controller.openPath(keyPath('count'));
    const pane = controller.getChain().at(-1)!;

    const first = controller.commitValueEdit(pane, '2');
    const second = controller.commitValueEdit(pane, '3');
    const third = controller.commitValueEdit(pane, '4');
    firstApply.resolve(true);
    await Promise.all([first, second, third]);

    expect(applyStructuredValueEdit.mock.calls.map(([intent]) => intent.raw)).toEqual(['2', '4']);
  });

  it('drops an older asynchronous navigation result when a later path wins', async () => {
    const staleLeaf = deferred<ReturnType<typeof readyPathValue>>();
    mocks.queryPathValue.mockImplementation(async ({ path }: { path: Array<{ key?: string }> }) => {
      if (!path.length) return readyPathValue('object', '{2}');
      if (pathKey(path) === 'a') return staleLeaf.promise;
      return readyPathValue('number', '2');
    });
    mocks.prepareGraph.mockResolvedValue({
      items: [item(keyPath('a'), 'number'), item(keyPath('b'), 'number')],
    });
    const { controller } = createController();

    const openA = controller.openPath(keyPath('a'));
    const openB = controller.openPath(keyPath('b'));
    await openB;
    staleLeaf.resolve(readyPathValue('number', '1'));
    await openA;

    expect(controller.getActivePath()).toEqual(keyPath('b'));
  });

  it('refreshes active columns and Monaco content when the projection revision changes', async () => {
    let value = '"old"';
    installDocument(
      { '': readyPathValue('object', '{1}'), value: readyPathValue('string', value) },
      { '': [item(keyPath('value'), 'string')] },
    );
    mocks.queryPathValue.mockImplementation(async ({ path }: { path: Array<{ key?: string }> }) =>
      path.length ? readyPathValue('string', value) : readyPathValue('object', '{1}'),
    );
    const { controller } = createController();
    await controller.openPath(keyPath('value'));
    value = '"external"';

    await controller.syncProjection({
      documentKey: 'document-1',
      languageId: 'json' as any,
      revision: 2,
      graphAppliedRevision: 2,
      snapshotId: 2 as any,
      enableNest: false,
      renderConfig: {} as any,
    });

    expect(controller.getChain().at(-1)?.content?.sourceText).toBe('"external"');
    expect(mocks.cacheClear).toHaveBeenCalled();
  });

  it('resets all navigation and history state on lifecycle teardown', async () => {
    installDocument(
      { '': readyPathValue('object', '{1}'), value: readyPathValue('number', '1') },
      { '': [item(keyPath('value'), 'number')] },
    );
    const { controller, states } = createController();
    await controller.openPath(keyPath('value'));

    controller.reset();

    expect(controller.getChain()).toEqual([]);
    expect(controller.getActivePath()).toEqual([]);
    expect(states.at(-1)).toMatchObject({ open: false, canGoBack: false, canGoForward: false });
  });
});

describe('buildWorkspacePathPrefixes', () => {
  it('keeps the complete root-to-selection chain without truncation', () => {
    const path = keyPath('a', 'b', 'c', 'd', 'e');
    expect(buildWorkspacePathPrefixes(path)).toEqual([
      [],
      keyPath('a'),
      keyPath('a', 'b'),
      keyPath('a', 'b', 'c'),
      keyPath('a', 'b', 'c', 'd'),
      path,
    ]);
  });
});
