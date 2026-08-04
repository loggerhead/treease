import { describe, expect, it } from 'vitest';
import { TabRuntimeRegistry } from './tab-runtime-registry';
import { createTabNavigationStore, type NavigationEntitySlices } from './tab-navigation-store';

type Slices = NavigationEntitySlices & { editorState: null; graphState: null; navigatorState: null; searchState: null };

function createStore() {
  return createTabNavigationStore<Slices>({
    workspaceId: 'workspace',
    initialTabs: [{ id: 'one', documentKey: 'one', revision: 0 }],
    createEntityState: () => ({ editorState: null, graphState: null, navigatorState: null, searchState: null }),
  });
}

describe('TabRuntimeRegistry', () => {
  it('disposes a tab runtime on replacement and prevents it from being read by the new generation', () => {
    const store = createStore();
    const registry = new TabRuntimeRegistry(store);
    const target = store.getTarget('one')!;
    let disposed = 0;

    expect(registry.register(target, 'graph', { value: { scene: 'old' }, dispose: () => disposed++ }).kind).toBe('applied');
    const replacement = store.replaceDocument('one', { documentKey: 'replacement', revision: 0 })!;

    expect(disposed).toBe(1);
    expect(registry.get(target, 'graph')).toBeNull();
    expect(registry.get(replacement, 'graph')).toBeNull();
  });

  it('disposes on close and refuses new bindings for a closed target', () => {
    const store = createStore();
    const registry = new TabRuntimeRegistry(store);
    const target = store.getTarget('one')!;
    let disposed = 0;
    registry.register(target, 'editor', { value: 'editor', dispose: () => disposed++ });

    store.close('one');

    expect(disposed).toBe(1);
    expect(registry.register(target, 'editor', { value: 'late' }).kind).toBe('closed');
  });
});
