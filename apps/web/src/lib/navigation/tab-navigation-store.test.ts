import { describe, expect, it } from 'vitest';
import { createTabNavigationStore, type NavigationEntitySlices } from './tab-navigation-store';

type Slices = NavigationEntitySlices & {
  editorState: { selection: string | null };
  graphState: { highlight: string | null };
  navigatorState: { history: string[] };
  searchState: { query: string };
};

function createStore() {
  return createTabNavigationStore<Slices>({
    workspaceId: 'workspace',
    initialTabs: [
      { id: 'one', documentKey: 'document:one', generation: 0, revision: 3 },
      { id: 'two', documentKey: 'document:two', generation: 0, revision: 7 },
    ],
    createEntityState: () => ({
      editorState: { selection: null },
      graphState: { highlight: null },
      navigatorState: { history: [] },
      searchState: { query: '' },
    }),
  });
}

describe('TabNavigationStore', () => {
  it('isolates entity state per tab and exposes entity-restricted writers', () => {
    const store = createStore();
    const one = store.getTarget('one')!;
    const two = store.getTarget('two')!;
    const editor = store.entity('editorState');
    const graph = store.entity('graphState');

    expect(editor.update(one, () => ({ selection: '$.one' }))).toEqual({ kind: 'applied' });
    expect(graph.update(two, () => ({ highlight: '$.two' }))).toEqual({ kind: 'applied' });

    expect(store.getTab('one')!.state.editorState).toEqual({ selection: '$.one' });
    expect(store.getTab('one')!.state.graphState).toEqual({ highlight: null });
    expect(store.getTab('two')!.state.editorState).toEqual({ selection: null });
    expect(store.getTab('two')!.state.graphState).toEqual({ highlight: '$.two' });
  });

  it('switches only the active projection state and does not mutate entity slices', () => {
    const store = createStore();
    const activeBefore = store.getActiveTarget();

    expect(store.activate('two')).toBe(true);

    expect(store.getActiveTarget()).toMatchObject({ tabId: 'two' });
    expect(store.getTab('one')!.state).toEqual({
      editorState: { selection: null }, graphState: { highlight: null }, navigatorState: { history: [] }, searchState: { query: '' },
    });
    expect(activeBefore).toMatchObject({ tabId: 'one' });
  });

  it('rejects late writes after a document replacement and resets the replacement state', () => {
    const store = createStore();
    const oldTarget = store.getTarget('one')!;
    const editor = store.entity('editorState');
    editor.update(oldTarget, () => ({ selection: '$.before' }));

    const replacement = store.replaceDocument('one', { documentKey: 'document:replacement', generation: 1, revision: 1 })!;

    expect(editor.update(oldTarget, () => ({ selection: '$.late' }))).toEqual({ kind: 'stale' });
    expect(replacement.generation).toBe(oldTarget.generation + 1);
    expect(store.getTab('one')!.state.editorState).toEqual({ selection: null });
  });

  it('advances a semantic revision without resetting tab-local navigation state', () => {
    const store = createStore();
    const before = store.getTarget('one')!;
    store.entity('navigatorState').update(before, () => ({ history: ['$.one'] }));

    const next = store.updateRevision('one', 4)!;

    expect(next).toMatchObject({ documentKey: before.documentKey, generation: before.generation, revision: 4 });
    expect(store.entity('navigatorState').update(before, () => ({ history: ['late'] }))).toEqual({ kind: 'stale' });
    expect(store.getTab('one')!.state.navigatorState).toEqual({ history: ['$.one'] });
  });

  it('rejects late writes after close and reports the lifecycle target', () => {
    const store = createStore();
    const target = store.getTarget('one')!;
    const events: string[] = [];
    store.subscribeLifecycle((event) => events.push(`${event.kind}:${event.kind === 'closed' ? event.target.tabId : event.previous.tabId}`));

    expect(store.close('one')).toBe(true);
    expect(store.entity('searchState').update(target, () => ({ query: 'late' }))).toEqual({ kind: 'closed' });
    expect(events).toEqual(['closed:one']);
  });
});
