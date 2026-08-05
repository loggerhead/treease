import { describe, expect, it, vi } from 'vitest';
import type { NavigationCommand, NavigationPath, NavigationResult, NavigationTransaction } from './navigation-contract';
import { TabNavigatorNavigationFacade, type NavigatorNavigationState } from './navigator-navigation-facade';
import { createTabNavigationStore, type NavigationEntitySlices } from './tab-navigation-store';

type Slices = NavigationEntitySlices & {
  editorState: null;
  graphState: null;
  navigatorState: NavigatorNavigationState;
  searchState: null;
};

const initialNavigatorState: NavigatorNavigationState = {
  activePath: [], history: [], historyIndex: -1, columnsMaterialized: false, expanded: false,
};
const path = (key: string): NavigationPath => [{ tag: 0, key, index: 0 }];

function createHarness() {
  const store = createTabNavigationStore<Slices>({
    workspaceId: 'workspace',
    initialTabs: [{ id: 'tab', documentKey: 'document', generation: 0, revision: 1 }],
    createEntityState: () => ({ editorState: null, graphState: null, navigatorState: initialNavigatorState, searchState: null }),
  });
  const port = { apply: vi.fn<(_: unknown) => Promise<NavigationResult>>().mockResolvedValue({ kind: 'applied' }) };
  const facade = new TabNavigatorNavigationFacade({
    writer: store.entity('navigatorState'),
    readState: (target) => store.getTab(target.tabId)?.state.navigatorState ?? null,
    targetReader: store,
    port,
  });
  const target = store.getTarget('tab')!;
  const transaction: NavigationTransaction = { id: 1, target, isCurrent: () => true };
  const command = (nextPath: NavigationPath): NavigationCommand & { history: 'merge' | 'push' } => (
    { target, transaction, path: nextPath, cellTarget: 'value', origin: 'navigator', history: 'push' }
  );
  return { store, port, facade, target, command };
}

describe('TabNavigatorNavigationFacade', () => {
  it('locates without materializing columns or expanding, while atomically forwarding path and history', async () => {
    const { store, port, facade, command } = createHarness();

    await expect(facade.locate({ ...command(path('one')), history: 'merge' })).resolves.toEqual({ kind: 'applied' });

    expect(port.apply).toHaveBeenCalledWith(expect.objectContaining({
      path: path('one'), history: [path('one')], historyIndex: 0, materializeColumns: false, expanded: false,
    }));
    expect(store.getTab('tab')!.state.navigatorState).toEqual({
      activePath: path('one'), history: [path('one')], historyIndex: 0, columnsMaterialized: false, expanded: false,
    });
  });

  it('materializes and expands only for full navigation, and pushes history', async () => {
    const { store, port, facade, command } = createHarness();
    await facade.locate({ ...command(path('one')), history: 'push' });

    await expect(facade.navigate(command(path('two')))).resolves.toEqual({ kind: 'applied' });

    expect(port.apply).toHaveBeenLastCalledWith(expect.objectContaining({
      path: path('two'), history: [path('one'), path('two')], historyIndex: 1, materializeColumns: true, expanded: true,
    }));
    expect(store.getTab('tab')!.state.navigatorState.columnsMaterialized).toBe(true);
  });

  it('returns stale without calling the runtime or writing when its transaction loses freshness', async () => {
    const { store, port, facade, target, command } = createHarness();
    const stale = { ...command(path('one')), transaction: { id: 2, target, isCurrent: () => false } };

    await expect(facade.navigate(stale)).resolves.toEqual({ kind: 'stale' });

    expect(port.apply).not.toHaveBeenCalled();
    expect(store.getTab('tab')!.state.navigatorState).toEqual(initialNavigatorState);
  });

  it('does not commit its slice when the target becomes stale during runtime work', async () => {
    const { store, port, facade, command } = createHarness();
    port.apply.mockResolvedValue({ kind: 'stale' });

    await expect(facade.navigate(command(path('one')))).resolves.toEqual({ kind: 'stale' });

    expect(store.getTab('tab')!.state.navigatorState).toEqual(initialNavigatorState);
  });
});
