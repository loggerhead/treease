import { get } from 'svelte/store';
import { describe, expect, it } from 'vitest';
import { createActiveTabProjection } from './active-tab-projection';
import { createTabNavigationStore, type NavigationEntitySlices } from './tab-navigation-store';

type Slices = NavigationEntitySlices & {
  editorState: { tab: string };
  graphState: null;
  navigatorState: null;
  searchState: null;
};

describe('ActiveTabProjection', () => {
  it('projects only the active tab and follows activation without writing state', () => {
    const store = createTabNavigationStore<Slices>({
      workspaceId: 'workspace',
      initialTabs: [
        { id: 'one', documentKey: 'one', generation: 0, revision: 0 },
        { id: 'two', documentKey: 'two', generation: 0, revision: 0 },
      ],
      createEntityState: (target) => ({ editorState: { tab: target.tabId }, graphState: null, navigatorState: null, searchState: null }),
    });
    const projection = createActiveTabProjection(store);

    expect(get(projection)?.state.editorState).toEqual({ tab: 'one' });
    store.activate('two');

    expect(get(projection)?.state.editorState).toEqual({ tab: 'two' });
    expect(store.getTab('one')?.state.editorState).toEqual({ tab: 'one' });
  });
});
