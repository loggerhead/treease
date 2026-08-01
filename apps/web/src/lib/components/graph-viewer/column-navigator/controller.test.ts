// @vitest-environment jsdom

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { PathSegTag } from '@core-wasm/index';

const mocks = vi.hoisted(() => ({
  queryPathValue: vi.fn(),
  queryDirectChildren: vi.fn(),
}));

vi.mock('../../../services/SnapshotProjectionService', () => ({
  queryPathValue: mocks.queryPathValue,
  queryDirectChildren: mocks.queryDirectChildren,
}));

vi.mock('../column-navigator-graph', () => ({
  buildColumnNavigatorDirectItems: (_path: unknown, children: unknown[]) => children,
  formatColumnNavigatorPath: (path: Array<{ key?: string; index?: number }>) =>
    path.length ? `$.${path.map((segment) => segment.key ?? `[${segment.index}]`).join('.')}` : '$',
  shouldOpenColumnNavigatorContent: (value: { valueType?: string; displayText?: string }) =>
    (value.valueType !== 'object' && value.valueType !== 'array') ||
    value.displayText === '{}' ||
    value.displayText === '[]',
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
  mocks.queryDirectChildren.mockImplementation(async ({ path }: { path: Array<{ key?: string }> }) => ({
    status: 'ready',
    data: items[pathKey(path)] ?? [],
  }));
}

function createController(overrides: Record<string, unknown> = {}) {
  const states: any[] = [];
  const controller = createColumnNavigatorController({
    defaultHeightPx: 220,
    getWorkspaceSnapshotId: () => 'snapshot-workspace' as any,
    getDocumentKey: () => 'document-1',
    getLanguageId: () => 'json' as any,
    getRevision: () => 1,
    getEnableNest: () => false,
    getReadonly: () => false,
    getShellHeight: () => 800,
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

  it('keeps the complete committed workspace visible while navigation is loading', async () => {
    installDocument(
      {
        '': readyPathValue('object', '{2}'),
        user: readyPathValue('object', '{1}'),
        'user.name': readyPathValue('string', '"Alice"'),
      },
      {
        '': [item(keyPath('user'))],
        user: [item(keyPath('user', 'name'), 'string')],
      },
    );
    const pendingLeaf = deferred<ReturnType<typeof readyPathValue>>();
    mocks.queryPathValue.mockImplementation(async ({ path }: { path: Array<{ key?: string }> }) => {
      if (pathKey(path) === 'user.name') return pendingLeaf.promise;
      return ({ '': readyPathValue('object', '{2}'), user: readyPathValue('object', '{1}') } as any)[pathKey(path)] ?? {
        status: 'ready',
        data: null,
      };
    });
    const { controller, states } = createController();
    await controller.openPath(keyPath('user'));

    const navigation = controller.selectPath(keyPath('user', 'name'));
    await Promise.resolve();

    expect(states.at(-1)?.isLoading).toBe(true);
    expect(states.at(-1)?.chain.map((pane: any) => [pane.pathKey, pane.kind, pane.status])).toEqual([
      ['$', 'column', 'ready'],
      ['k:user', 'column', 'ready'],
      ['k:user', 'content', 'ready'],
    ]);

    pendingLeaf.resolve(readyPathValue('string', '"Alice"'));
    await navigation;
  });

  it('reuses path values shared by sibling navigations within the same snapshot', async () => {
    installDocument(
      {
        '': readyPathValue('object', '{2}'),
        alpha: readyPathValue('number', '1'),
        beta: readyPathValue('number', '2'),
      },
      { '': [item(keyPath('alpha'), 'number'), item(keyPath('beta'), 'number')] },
    );
    const { controller } = createController();
    await controller.openPath(keyPath('alpha'));
    await controller.selectPath(keyPath('beta'));

    const queriedPaths = mocks.queryPathValue.mock.calls.map(([input]) => pathKey(input.path));
    expect(queriedPaths.filter((path) => path === '')).toHaveLength(1);
    expect(queriedPaths.filter((path) => path === 'alpha')).toHaveLength(1);
    expect(queriedPaths.filter((path) => path === 'beta')).toHaveLength(1);
  });

  it('wraps sibling navigation from the first item to the last and back again', async () => {
    installDocument(
      {
        '': readyPathValue('object', '{3}'),
        alpha: readyPathValue('number', '1'),
        beta: readyPathValue('number', '2'),
        gamma: readyPathValue('number', '3'),
      },
      {
        '': [
          item(keyPath('alpha'), 'number'),
          item(keyPath('beta'), 'number'),
          item(keyPath('gamma'), 'number'),
        ],
      },
    );
    const { controller } = createController();
    await controller.openPath(keyPath('alpha'));

    await controller.moveSibling(-1);
    expect(controller.getActivePath()).toEqual(keyPath('gamma'));

    await controller.moveSibling(1);
    expect(controller.getActivePath()).toEqual(keyPath('alpha'));
  });

  it('moves through every sibling while coalescing their graph reveals', async () => {
    installDocument(
      {
        '': readyPathValue('object', '{3}'),
        alpha: readyPathValue('number', '1'),
        beta: readyPathValue('number', '2'),
        gamma: readyPathValue('number', '3'),
      },
      {
        '': [
          item(keyPath('alpha'), 'number'),
          item(keyPath('beta'), 'number'),
          item(keyPath('gamma'), 'number'),
        ],
      },
    );
    const emitReveal = vi.fn();
    const { controller, states } = createController({ emitReveal });
    await controller.openPath(keyPath('alpha'));
    emitReveal.mockClear();
    const stateCountBeforeMoves = states.length;

    vi.useFakeTimers();
    try {
      const moves = Array.from({ length: 30 }, () => controller.moveSibling(1));
      await Promise.all(moves);
      expect(states.slice(stateCountBeforeMoves).filter((state) => state.isLoading)).toHaveLength(30);
      expect(emitReveal).not.toHaveBeenCalled();
      await vi.advanceTimersByTimeAsync(48);
    } finally {
      vi.useRealTimers();
    }

    expect(controller.getActivePath()).toEqual(keyPath('alpha'));
    expect(emitReveal).toHaveBeenCalledOnce();
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

  it('clears forward history when a new path is selected after going back', async () => {
    installDocument(
      {
        '': readyPathValue('object', '{3}'),
        alpha: readyPathValue('number', '1'),
        beta: readyPathValue('number', '2'),
        gamma: readyPathValue('number', '3'),
      },
      { '': [item(keyPath('alpha'), 'number'), item(keyPath('beta'), 'number'), item(keyPath('gamma'), 'number')] },
    );
    const { controller, states } = createController();

    await controller.openPath(keyPath('alpha'));
    await controller.selectPath(keyPath('beta'));
    await controller.goBack();
    await controller.selectPath(keyPath('gamma'));

    expect(controller.getActivePath()).toEqual(keyPath('gamma'));
    expect(states.at(-1)).toMatchObject({ canGoBack: true, canGoForward: false });
    await controller.goForward();
    expect(controller.getActivePath()).toEqual(keyPath('gamma'));
  });

  it('navigates to the parent path without creating a second navigation state', async () => {
    installDocument(
      {
        '': readyPathValue('object', '{1}'),
        root: readyPathValue('object', '{1}'),
        'root.child': readyPathValue('object', '{1}'),
        'root.child.leaf': readyPathValue('string', '"value"'),
      },
      {
        '': [item(keyPath('root'))],
        root: [item(keyPath('root', 'child'))],
        'root.child': [item(keyPath('root', 'child', 'leaf'), 'string')],
      },
    );
    const { controller } = createController();

    await controller.openPath(keyPath('root', 'child', 'leaf'));
    await controller.navigateParent();

    expect(controller.getActivePath()).toEqual(keyPath('root', 'child'));
    expect(controller.getChain().filter((pane) => pane.pathKey === 'k:root|k:child')).toHaveLength(2);
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

  it('stops a failed commit without replacing the current detail projection', async () => {
    installDocument(
      { '': readyPathValue('object', '{1}'), count: readyPathValue('number', '1') },
      { '': [item(keyPath('count'), 'number')] },
    );
    const applyStructuredValueEdit = vi.fn().mockResolvedValue(false);
    const { controller } = createController({ applyStructuredValueEdit });
    await controller.openPath(keyPath('count'));

    await controller.commitValueEdit(controller.getChain().at(-1)!, 'not committed');

    expect(applyStructuredValueEdit).toHaveBeenCalledOnce();
    expect(controller.getChain().at(-1)?.content?.sourceText).toBe('1');
  });

  it('drops an older asynchronous navigation result when a later path wins', async () => {
    const staleLeaf = deferred<ReturnType<typeof readyPathValue>>();
    mocks.queryPathValue.mockImplementation(async ({ path }: { path: Array<{ key?: string }> }) => {
      if (!path.length) return readyPathValue('object', '{2}');
      if (pathKey(path) === 'a') return staleLeaf.promise;
      return readyPathValue('number', '2');
    });
    mocks.queryDirectChildren.mockResolvedValue({
      status: 'ready',
      data: [item(keyPath('a'), 'number'), item(keyPath('b'), 'number')],
    });
    const { controller } = createController();

    const openA = controller.openPath(keyPath('a'));
    const openB = controller.openPath(keyPath('b'));
    await openB;
    staleLeaf.resolve(readyPathValue('number', '1'));
    await openA;

    expect(controller.getActivePath()).toEqual(keyPath('b'));
  });

  it('does not rematerialize a pending navigation after reset', async () => {
    const pending = deferred<ReturnType<typeof readyPathValue>>();
    mocks.queryPathValue.mockImplementation(async ({ path }: { path: Array<{ key?: string }> }) => {
      if (!path.length) return readyPathValue('object', '{1}');
      if (pathKey(path) === 'slow') return pending.promise;
      return readyPathValue('number', '1');
    });
    mocks.queryDirectChildren.mockResolvedValue({ status: 'ready', data: [item(keyPath('slow'), 'number')] });
    const { controller } = createController();

    const navigation = controller.openPath(keyPath('slow'));
    controller.reset();
    pending.resolve(readyPathValue('number', '1'));
    await navigation;

    expect(controller.getActivePath()).toEqual([]);
    expect(controller.getChain()).toEqual([]);
  });

  it('defers transient snapshot-not-ready errors until the next projection refresh', async () => {
    mocks.queryPathValue.mockImplementation(async ({ path }: { path: Array<{ key?: string }> }) =>
      path.length ? readyPathValue('string', '"value"') : readyPathValue('object', '{1}'),
    );
    mocks.queryDirectChildren.mockResolvedValueOnce({ status: 'snapshotNotReady' });
    const handleError = vi.fn();
    const { controller } = createController({ handleError });

    await controller.openPath(keyPath('value'));

    expect(handleError).not.toHaveBeenCalled();
    expect(controller.getChain().at(-1)?.status).toBe('loading');

    mocks.queryDirectChildren.mockResolvedValue({ status: 'ready', data: [item(keyPath('value'), 'string')] });
    await controller.syncProjection({
      documentKey: 'document-1',
      languageId: 'json' as any,
      revision: 1,
      graphAppliedRevision: 1,
      snapshotId: 'snapshot-workspace' as any,
      enableNest: false,
    });

    expect(controller.getChain().at(-1)?.status).toBe('ready');
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
    });

    expect(controller.getChain().at(-1)?.content?.sourceText).toBe('"external"');
    expect(mocks.queryDirectChildren).toHaveBeenCalled();
  });

  it('refreshes projection state when nesting changes', async () => {
    installDocument(
      { '': readyPathValue('object', '{1}'), value: readyPathValue('string', '"old"') },
      { '': [item(keyPath('value'), 'string')] },
    );
    let enableNest = false;
    const { controller } = createController({
      getEnableNest: () => enableNest,
    });
    await controller.openPath(keyPath('value'));
    mocks.queryDirectChildren.mockClear();
    enableNest = true;

    await controller.syncProjection({
      documentKey: 'document-1',
      languageId: 'json' as any,
      revision: 1,
      graphAppliedRevision: 1,
      snapshotId: 1 as any,
      enableNest,
    });

    expect(mocks.queryDirectChildren).toHaveBeenCalled();
  });

  it('clamps divider height to the shell bounds and clears drag state on release', () => {
    const { controller, states } = createController();

    controller.setHeight(1);
    expect(states.at(-1)?.heightPx).toBe(100);
    controller.setHeight(10_000);
    expect(states.at(-1)?.heightPx).toBe(600);

    controller.startDividerDrag(400);
    expect(states.at(-1)?.isDraggingDivider).toBe(true);
    controller.moveDividerDrag(1_000);
    expect(states.at(-1)?.heightPx).toBe(100);
    controller.endDividerDrag();
    expect(states.at(-1)?.isDraggingDivider).toBe(false);
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
