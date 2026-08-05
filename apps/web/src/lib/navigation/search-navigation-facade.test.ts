import { describe, expect, it } from 'vitest';
import type { NavigationTransaction } from './navigation-contract';
import { TabSearchNavigationFacade, type SearchNavigationState } from './search-navigation-facade';
import { createTabNavigationStore, type NavigationEntitySlices } from './tab-navigation-store';

type Slices = NavigationEntitySlices & {
  editorState: null;
  graphState: null;
  navigatorState: null;
  searchState: SearchNavigationState;
};

function createHarness() {
  const store = createTabNavigationStore<Slices>({
    workspaceId: 'workspace',
    initialTabs: [{ id: 'tab', documentKey: 'document', generation: 0, revision: 1 }],
    createEntityState: () => ({ editorState: null, graphState: null, navigatorState: null, searchState: { previewId: null } }),
  });
  const target = store.getTarget('tab')!;
  const transaction: NavigationTransaction = { id: 1, target, isCurrent: () => true };
  const facade = new TabSearchNavigationFacade({
    writer: store.entity('searchState'),
    readState: (nextTarget) => store.getTab(nextTarget.tabId)?.state.searchState ?? null,
    targetReader: store,
  });
  return { store, target, transaction, facade };
}

describe('TabSearchNavigationFacade', () => {
  it('owns preview identity and only clears a matching preview on end', async () => {
    const { facade, store, target, transaction } = createHarness();
    const preview = { target, transaction, previewId: 'preview-a' };

    expect(facade.beginPreview(preview)).toEqual({ kind: 'applied' });
    await expect(facade.endPreview({ ...preview, previewId: 'preview-b', reason: 'cancelled' })).resolves.toEqual({ kind: 'no-op' });
    expect(store.getTab('tab')!.state.searchState).toEqual({ previewId: 'preview-a' });

    await expect(facade.endPreview({ ...preview, reason: 'committed' })).resolves.toEqual({ kind: 'applied' });
    expect(store.getTab('tab')!.state.searchState).toEqual({ previewId: null });
  });

  it('discards a preview on a newer entity navigation and rejects stale lifecycle work', async () => {
    const { facade, store, target, transaction } = createHarness();
    facade.beginPreview({ target, transaction, previewId: 'preview-a' });

    await expect(facade.discardPreview({ target, transaction, reason: 'superseded' })).resolves.toEqual({ kind: 'applied' });
    expect(store.getTab('tab')!.state.searchState).toEqual({ previewId: null });

    const staleTransaction: NavigationTransaction = { id: 2, target, isCurrent: () => false };
    expect(facade.beginPreview({ target, transaction: staleTransaction, previewId: 'late' })).toEqual({ kind: 'stale' });
    expect(store.getTab('tab')!.state.searchState).toEqual({ previewId: null });
  });
});
