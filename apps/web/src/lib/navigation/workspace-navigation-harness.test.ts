import { describe, expect, it, vi } from 'vitest';
import { TabEditorNavigationFacade, type EditorNavigationState } from './editor-navigation-facade';
import { GraphNavigationFacade, type GraphNavigationRuntimePort } from './graph-navigation-facade';
import { NavigationCoordinator } from './navigation-coordinator';
import { TabNavigatorNavigationFacade, type NavigatorNavigationState } from './navigator-navigation-facade';
import { TabSearchNavigationFacade, type SearchNavigationState } from './search-navigation-facade';
import type { NavigationEntitySlices } from './tab-navigation-store';
import { createTabNavigationStore } from './tab-navigation-store';

type Slices = NavigationEntitySlices & {
  editorState: EditorNavigationState;
  graphState: { untouched: true };
  navigatorState: NavigatorNavigationState;
  searchState: SearchNavigationState;
};

const path = (key: string) => [{ tag: 0, key, index: 0 }] as const;

function createHarness() {
  const store = createTabNavigationStore<Slices>({
    workspaceId: 'workspace',
    initialTabs: [
      { id: 'left', documentKey: 'left-document', revision: 1 },
      { id: 'right', documentKey: 'right-document', revision: 1 },
    ],
    createEntityState: () => ({
      editorState: { selection: null, lastNavigationSelection: null },
      graphState: { untouched: true },
      navigatorState: { activePath: [], history: [], historyIndex: -1, columnsMaterialized: false, expanded: false },
      searchState: { previewId: null },
    }),
  });
  const editorLocate = vi.fn(async () => ({ kind: 'applied' as const }));
  const graph = new Map<string, { selection: string; viewport: string }>();
  const graphRuntime: GraphNavigationRuntimePort = {
    isInteractive: () => true,
    capturePreviewBaseline: async ({ target }) => ({ ...(graph.get(target.tabId) ?? { selection: '', viewport: '' }) }),
    highlight: async ({ target }) => {
      const state = graph.get(target.tabId) ?? { selection: '', viewport: '' };
      graph.set(target.tabId, { ...state, selection: 'highlighted' });
      return { kind: 'applied' };
    },
    reveal: async ({ target }) => {
      const state = graph.get(target.tabId) ?? { selection: '', viewport: '' };
      graph.set(target.tabId, { ...state, viewport: 'revealed' });
      return { kind: 'applied' };
    },
    restoreSelection: async ({ target }, baseline) => {
      const state = graph.get(target.tabId) ?? { selection: '', viewport: '' };
      graph.set(target.tabId, { ...state, selection: String(baseline.selection) });
      return { kind: 'applied' };
    },
    restoreViewport: async ({ target }, baseline) => {
      const state = graph.get(target.tabId) ?? { selection: '', viewport: '' };
      graph.set(target.tabId, { ...state, viewport: String(baseline.viewport) });
      return { kind: 'applied' };
    },
    cancelViewportTransition: async () => ({ kind: 'applied' }),
  };
  const editor = new TabEditorNavigationFacade({
    writer: store.entity('editorState'), runtime: { locate: editorLocate }, targetReader: store,
    isVisible: (target) => store.getSnapshot().activeTabId === target.tabId, publish: () => {},
  });
  const graphFacade = new GraphNavigationFacade({ runtime: graphRuntime, targetReader: store });
  const navigator = new TabNavigatorNavigationFacade({
    writer: store.entity('navigatorState'), targetReader: store,
    readState: (target) => store.getTab(target.tabId)?.state.navigatorState ?? null,
    port: { apply: async () => ({ kind: 'applied' }) },
  });
  const search = new TabSearchNavigationFacade({
    writer: store.entity('searchState'), targetReader: store,
    readState: (target) => store.getTab(target.tabId)?.state.searchState ?? null,
  });
  const coordinator = new NavigationCoordinator({
    facades: { editor, graph: graphFacade, navigator, search }, targetReader: store,
    getSettings: () => ({ completeNavigationEnabled: false }),
  });
  return { store, coordinator, graph, editorLocate };
}

describe('workspace navigation harness', () => {
  it('keeps lightweight navigation tab-local and leaves editor/columns untouched', async () => {
    const { store, coordinator, graph, editorLocate } = createHarness();
    const target = store.getTarget('left')!;

    await expect(coordinator.dispatch({ kind: 'editor-selection', target, path: path('a'), cellTarget: 'key' }))
      .resolves.toMatchObject({ behavior: 'locate', outcome: 'applied' });

    expect(editorLocate).not.toHaveBeenCalled();
    expect(graph.get('left')?.selection).toBe('highlighted');
    expect(store.getTab('left')?.state.navigatorState).toMatchObject({ activePath: path('a'), columnsMaterialized: false });
    expect(store.getTab('right')?.state.navigatorState.activePath).toEqual([]);
  });

  it('commits a search result as complete navigation and never restores its preview', async () => {
    const { store, coordinator, graph, editorLocate } = createHarness();
    const target = store.getTarget('left')!;
    graph.set('left', { selection: 'before', viewport: 'before' });

    await coordinator.dispatch({ kind: 'search-preview', target, path: path('a'), cellTarget: 'value', previewId: 'preview' });
    await expect(coordinator.dispatch({ kind: 'search-commit', target, path: path('b'), cellTarget: 'value', previewId: 'preview' }))
      .resolves.toMatchObject({ behavior: 'navigate', outcome: 'applied' });

    expect(editorLocate).toHaveBeenCalledOnce();
    expect(editorLocate).toHaveBeenCalledWith(expect.anything(), expect.anything(), { reveal: true, focus: false });
    expect(graph.get('left')).toEqual({ selection: 'highlighted', viewport: 'revealed' });
    expect(store.getTab('left')?.state.navigatorState).toMatchObject({ activePath: path('b'), columnsMaterialized: true, expanded: true });
    expect(store.getTab('left')?.state.searchState.previewId).toBeNull();
  });

  it('rejects a closed tab before any facade writes', async () => {
    const { store, coordinator, graph } = createHarness();
    const target = store.getTarget('left')!;
    store.close('left');

    await expect(coordinator.dispatch({ kind: 'graph-cell', target, path: path('a'), cellTarget: 'node' }))
      .resolves.toMatchObject({ outcome: 'closed' });
    expect(graph.has('left')).toBe(false);
  });
});
