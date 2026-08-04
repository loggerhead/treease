import { describe, expect, it, vi } from 'vitest';
import { NavigationCoordinator } from './navigation-coordinator';
import type {
  NavigationFacades,
  NavigationResult,
  NavigationTarget,
  NavigationTargetReader,
  NavigationUserEvent,
} from './navigation-contract';

const target = (tabId = 'tab-a'): NavigationTarget => ({
  workspaceId: 'workspace', tabId, documentKey: `document-${tabId}`, generation: 1, revision: 2,
});

function resolved(): Promise<NavigationResult> {
  return Promise.resolve({ kind: 'applied' });
}

function createFacades(): NavigationFacades {
  return {
    editor: { navigate: vi.fn(resolved) },
    graph: { locate: vi.fn(resolved), navigate: vi.fn(resolved), preview: vi.fn(resolved), cancelPreview: vi.fn(resolved), flush: vi.fn(resolved) },
    navigator: { locate: vi.fn(resolved), navigate: vi.fn(resolved) },
    search: { beginPreview: vi.fn((): NavigationResult => ({ kind: 'applied' })), endPreview: vi.fn(resolved), discardPreview: vi.fn(resolved) },
  };
}

function createCoordinator(facades = createFacades(), reader: NavigationTargetReader = { status: () => 'current' }) {
  return new NavigationCoordinator({ facades, targetReader: reader, getSettings: () => ({ completeNavigationEnabled: false }) });
}

const selection = (tabId = 'tab-a'): NavigationUserEvent => ({ kind: 'editor-selection', target: target(tabId), path: [], cellTarget: 'node' });

describe('NavigationCoordinator', () => {
  it('executes lightweight editor selection without changing editor or navigator columns', async () => {
    const facades = createFacades();
    const result = await createCoordinator(facades).dispatch(selection());

    expect(result).toMatchObject({ behavior: 'locate', outcome: 'applied' });
    expect(facades.editor.navigate).not.toHaveBeenCalled();
    expect(facades.graph.locate).toHaveBeenCalledOnce();
    expect(facades.navigator.locate).toHaveBeenCalledWith(expect.objectContaining({ history: 'merge', target: target() }));
    expect(facades.navigator.navigate).not.toHaveBeenCalled();
  });

  it('always fully navigates a tree-path click and reports explicit facade outcomes', async () => {
    const facades = createFacades();
    vi.mocked(facades.graph.navigate).mockResolvedValue({ kind: 'no-op' });
    vi.mocked(facades.navigator.navigate).mockResolvedValue({ kind: 'cancelled' });
    const coordinator = createCoordinator(facades);
    const result = await coordinator.dispatch({ kind: 'navigator-tree-path', target: target(), path: [], cellTarget: 'value' });

    expect(result).toMatchObject({ behavior: 'navigate', outcome: 'cancelled' });
    expect(facades.editor.navigate).toHaveBeenCalledOnce();
    expect(facades.editor.navigate).toHaveBeenCalledWith(expect.anything(), { focus: false });
    expect(facades.graph.navigate).toHaveBeenCalledOnce();
    expect(facades.navigator.navigate).toHaveBeenCalledWith(expect.objectContaining({ history: 'push' }));
  });

  it('does not feed editor-originated complete navigation back into the editor', async () => {
    const facades = createFacades();
    const coordinator = new NavigationCoordinator({
      facades,
      targetReader: { status: () => 'current' },
      getSettings: () => ({ completeNavigationEnabled: true }),
    });

    await coordinator.dispatch(selection());

    expect(facades.editor.navigate).not.toHaveBeenCalled();
    expect(facades.graph.navigate).toHaveBeenCalledOnce();
    expect(facades.navigator.navigate).toHaveBeenCalledOnce();
  });

  it('uses per-tab latest-wins without cancelling a different tab', async () => {
    let releaseFirst!: () => void;
    const firstGate = new Promise<void>((resolve) => { releaseFirst = resolve; });
    const facades = createFacades();
    vi.mocked(facades.graph.locate).mockImplementation(async (command) => {
      if (command.target.tabId === 'tab-a') await firstGate;
      return command.transaction.isCurrent() ? { kind: 'applied' } : { kind: 'stale' };
    });
    const coordinator = createCoordinator(facades);

    const first = coordinator.dispatch(selection('tab-a'));
    const otherTab = coordinator.dispatch(selection('tab-b'));
    const replacement = coordinator.dispatch({ kind: 'graph-cell', target: target('tab-a'), path: [], cellTarget: 'key' });
    releaseFirst();

    await expect(first).resolves.toMatchObject({ outcome: 'stale' });
    await expect(otherTab).resolves.toMatchObject({ outcome: 'applied' });
    await expect(replacement).resolves.toMatchObject({ outcome: 'applied' });
  });

  it('does not invoke facades after its captured tab target has closed', async () => {
    const facades = createFacades();
    const result = await createCoordinator(facades, { status: () => 'closed' }).dispatch(selection());

    expect(result).toMatchObject({ outcome: 'closed', results: [{ kind: 'closed' }] });
    expect(facades.graph.locate).not.toHaveBeenCalled();
  });

  it('flushes only the graph command when the graph scene becomes interactive', async () => {
    const facades = createFacades();
    const result = await createCoordinator(facades).dispatch({ kind: 'graph-ready', target: target() });

    expect(result).toMatchObject({ behavior: 'none', outcome: 'applied' });
    expect(facades.graph.flush).toHaveBeenCalledOnce();
    expect(facades.editor.navigate).not.toHaveBeenCalled();
    expect(facades.navigator.navigate).not.toHaveBeenCalled();
  });

  it('does not stale an in-flight navigation when graph readiness is observed', async () => {
    let release!: () => void;
    const gate = new Promise<void>((resolve) => { release = resolve; });
    const facades = createFacades();
    vi.mocked(facades.graph.locate).mockImplementation(async () => {
      await gate;
      return { kind: 'applied' };
    });
    const coordinator = createCoordinator(facades);

    const pending = coordinator.dispatch(selection());
    await coordinator.dispatch({ kind: 'graph-ready', target: target() });
    release();

    await expect(pending).resolves.toMatchObject({ outcome: 'applied' });
  });
});
