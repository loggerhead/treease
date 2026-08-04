import { derived, type Readable } from 'svelte/store';
import type { NavigationEntitySlices, TabNavigationStore, TabNavigationTab } from './tab-navigation-store';

export type ActiveTabProjection<Slices extends NavigationEntitySlices> = Readable<TabNavigationTab<Slices> | null> & {
  getSnapshot(): TabNavigationTab<Slices> | null;
};

/** Read-only active-tab view. UI binding must use restore/binding causes. */
export function createActiveTabProjection<Slices extends NavigationEntitySlices>(
  store: TabNavigationStore<Slices>,
): ActiveTabProjection<Slices> {
  const projection = derived(store, ($store) => ($store.activeTabId ? $store.tabsById[$store.activeTabId] ?? null : null));
  return {
    subscribe: projection.subscribe,
    getSnapshot: () => {
      const state = store.getSnapshot();
      return state.activeTabId ? state.tabsById[state.activeTabId] ?? null : null;
    },
  };
}
