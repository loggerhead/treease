import { describe, expect, it, vi } from 'vitest';
import {
  TabEditorNavigationFacade,
  type EditorNavigationRuntimePort,
  type EditorRuntimeContext,
} from './editor-navigation-facade';
import type { NavigationCommand, NavigationPath, NavigationTarget, NavigationTransaction } from './navigation-contract';
import { createTabNavigationStore, type NavigationEntitySlices } from './tab-navigation-store';

type Slices = NavigationEntitySlices & {
  editorState: {
    selection: { path: NavigationPath; cellTarget: 'key' | 'value' | 'node' } | null;
    lastNavigationSelection: { path: NavigationPath; cellTarget: 'key' | 'value' | 'node' } | null;
  };
  graphState: { untouched: true };
  navigatorState: { untouched: true };
  searchState: { untouched: true };
};

const target: NavigationTarget = {
  workspaceId: 'workspace', tabId: 'tab', documentKey: 'document', generation: 0, revision: 1,
};

function createStore() {
  return createTabNavigationStore<Slices>({
    workspaceId: target.workspaceId,
    initialTabs: [{ id: target.tabId, documentKey: target.documentKey, revision: target.revision }],
    createEntityState: () => ({
      editorState: { selection: null, lastNavigationSelection: null }, graphState: { untouched: true }, navigatorState: { untouched: true }, searchState: { untouched: true },
    }),
  });
}

function transaction() {
  let current = true;
  const value: NavigationTransaction = { id: 1, target, isCurrent: () => current };
  return { value, expire: () => { current = false; } };
}

function command(tx: NavigationTransaction): NavigationCommand {
  return { target, transaction: tx, path: [], cellTarget: 'node', origin: 'editor' };
}

function createFacade(runtime: EditorNavigationRuntimePort, visible = true) {
  const store = createStore();
  const publish = vi.fn();
  return {
    store,
    publish,
    facade: new TabEditorNavigationFacade({
      writer: store.entity('editorState'), runtime, targetReader: store,
      isVisible: () => visible, publish,
    }),
  };
}

const appliedRuntime: EditorNavigationRuntimePort = {
  locate: async () => ({ kind: 'applied' }),
};

describe('TabEditorNavigationFacade', () => {
  it.each(['programmatic', 'restore', 'binding', 'edit', 'scroll', 'unknown'] as const)(
    'does not publish %s selection changes',
    (cause) => {
      const { facade, publish } = createFacade(appliedRuntime);
      expect(facade.recordSelection({ target, cause, path: [], cellTarget: 'node' })).toEqual({ kind: 'no-op' });
      expect(publish).not.toHaveBeenCalled();
    },
  );

  it('publishes only changed, resolved user selections', () => {
    const { facade, publish } = createFacade(appliedRuntime);

    expect(facade.recordSelection({ target, cause: 'user', path: [], cellTarget: 'node' }).kind).toBe('published');
    expect(facade.recordSelection({ target, cause: 'user', path: [], cellTarget: 'node' })).toEqual({ kind: 'no-op' });
    expect(facade.recordSelection({ target, cause: 'user', path: null, cellTarget: 'node' })).toEqual({ kind: 'no-op' });
    expect(publish).toHaveBeenCalledTimes(1);
  });

  it('does not republish a path after an unresolved selection', () => {
    const { facade, publish } = createFacade(appliedRuntime);

    facade.recordSelection({ target, cause: 'user', path: [], cellTarget: 'node' });
    facade.recordSelection({ target, cause: 'unknown', path: null, cellTarget: 'node' });
    expect(facade.recordSelection({ target, cause: 'user', path: [], cellTarget: 'node' })).toEqual({ kind: 'no-op' });
    expect(publish).toHaveBeenCalledTimes(1);
  });

  it('performs full navigation programmatically and persists only editor state', async () => {
    const locate = vi.fn<EditorNavigationRuntimePort['locate']>().mockResolvedValue({ kind: 'applied' });
    const { facade, store } = createFacade({ locate });
    const tx = transaction();

    await expect(facade.navigate(command(tx.value), { focus: true })).resolves.toEqual({ kind: 'applied' });
    expect(locate).toHaveBeenCalledWith(expect.objectContaining({ target, isVisible: true }), expect.anything(), { reveal: true, focus: true });
    expect(store.getTab(target.tabId)?.state).toEqual({
      editorState: { selection: { path: [], cellTarget: 'node' }, lastNavigationSelection: { path: [], cellTarget: 'node' } },
      graphState: { untouched: true }, navigatorState: { untouched: true }, searchState: { untouched: true },
    });
  });

  it('suppresses visible effects for a background tab', async () => {
    const locate = vi.fn<EditorNavigationRuntimePort['locate']>().mockResolvedValue({ kind: 'applied' });
    const { facade } = createFacade({ locate }, false);
    const tx = transaction();

    await facade.navigate(command(tx.value), { focus: true });
    expect(locate).toHaveBeenCalledWith(expect.objectContaining({ isVisible: false }), expect.anything(), { reveal: false, focus: false });
  });

  it('does not write a late runtime result after the transaction becomes stale', async () => {
    let release!: () => void;
    const gate = new Promise<void>((resolve) => { release = resolve; });
    const runtime: EditorNavigationRuntimePort = {
      locate: async (context: EditorRuntimeContext) => {
        await gate;
        return context.isCurrent() ? { kind: 'applied' } : { kind: 'stale' };
      },
    };
    const { facade, store } = createFacade(runtime);
    const tx = transaction();

    const pending = facade.navigate(command(tx.value), { focus: true });
    tx.expire();
    release();

    await expect(pending).resolves.toEqual({ kind: 'stale' });
    expect(store.getTab(target.tabId)?.state.editorState).toEqual({ selection: null, lastNavigationSelection: null });
  });

  it('reveals a visible editor without moving focus when navigation originated elsewhere', async () => {
    const locate = vi.fn<EditorNavigationRuntimePort['locate']>().mockResolvedValue({ kind: 'applied' });
    const { facade } = createFacade({ locate });
    const tx = transaction();

    await facade.navigate(command(tx.value), { focus: false });

    expect(locate).toHaveBeenCalledWith(expect.objectContaining({ isVisible: true }), expect.anything(), { reveal: true, focus: false });
  });
});
