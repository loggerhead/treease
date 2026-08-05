import { describe, expect, it } from 'vitest';
import {
  GraphNavigationFacade,
  type GraphNavigationRuntimePort,
  type GraphPreviewBaseline,
  type GraphRuntimeContext,
} from './graph-navigation-facade';
import type { NavigationCommand, NavigationTarget, NavigationTransaction } from './navigation-contract';

const target: NavigationTarget = {
  workspaceId: 'workspace', tabId: 'tab', documentKey: 'document', generation: 1, revision: 1,
};

function transaction() {
  let current = true;
  const value: NavigationTransaction = { id: 1, target, isCurrent: () => current };
  return { value, expire: () => { current = false; } };
}

function command(tx: NavigationTransaction): NavigationCommand {
  return { target, transaction: tx, path: [], cellTarget: 'node', origin: 'graph' };
}

function createRuntime() {
  const graph = { selection: 'before', viewport: 'before', calls: [] as string[] };
  const baseline: GraphPreviewBaseline = { selection: 'before', viewport: 'before' };
  const write = (context: GraphRuntimeContext, name: string, apply: () => void) => {
    if (!context.isCurrent()) return Promise.resolve({ kind: 'stale' } as const);
    graph.calls.push(name);
    apply();
    return Promise.resolve({ kind: 'applied' } as const);
  };
  const runtime: GraphNavigationRuntimePort = {
    isInteractive: () => true,
    capturePreviewBaseline: async () => baseline,
    highlight: (context) => write(context, 'highlight', () => { graph.selection = 'preview'; }),
    reveal: (context) => write(context, 'reveal', () => { graph.viewport = 'preview'; }),
    restoreSelection: (context, value) => write(context, 'restore-selection', () => { graph.selection = value.selection as string; }),
    restoreViewport: (context, value) => write(context, 'restore-viewport', () => { graph.viewport = value.viewport as string; }),
    cancelViewportTransition: (context) => write(context, 'cancel-transition', () => {}),
  };
  return { graph, runtime };
}

describe('GraphNavigationFacade', () => {
  it('transfers preview baseline ownership to the latest preview identity', async () => {
    const { graph, runtime } = createRuntime();
    const tx = transaction();
    const facade = new GraphNavigationFacade({ runtime, targetReader: { status: () => 'current' } });

    await facade.preview({ ...command(tx.value), previewId: 'first', mode: 'viewport' });
    await facade.preview({ ...command(tx.value), previewId: 'latest', mode: 'highlight' });

    await expect(facade.cancelPreview({ target, transaction: tx.value, previewId: 'first' })).resolves.toEqual({ kind: 'no-op' });
    await expect(facade.cancelPreview({ target, transaction: tx.value, previewId: 'latest' })).resolves.toEqual({ kind: 'applied' });
    expect(graph.selection).toBe('before');
    expect(graph.viewport).toBe('before');
    expect(graph.calls.slice(-3)).toEqual(['cancel-transition', 'restore-selection', 'restore-viewport']);
  });

  it('drops a preview without restoring it when explicit navigation takes ownership', async () => {
    const { graph, runtime } = createRuntime();
    const tx = transaction();
    const facade = new GraphNavigationFacade({ runtime, targetReader: { status: () => 'current' } });

    await facade.preview({ ...command(tx.value), previewId: 'preview', mode: 'viewport' });
    await facade.navigate(command(tx.value));

    expect(graph.calls).not.toContain('restore-selection');
    expect(graph.calls).not.toContain('restore-viewport');
    expect(graph.calls).toContain('cancel-transition');
  });

  it('does not restore a viewport after the user takes it over', async () => {
    const { graph, runtime } = createRuntime();
    const tx = transaction();
    const facade = new GraphNavigationFacade({ runtime, targetReader: { status: () => 'current' } });

    await facade.preview({ ...command(tx.value), previewId: 'preview', mode: 'viewport' });
    graph.viewport = 'user';
    await facade.releasePreviewViewport(target);
    await facade.cancelPreview({ target, transaction: tx.value, previewId: 'preview' });

    expect(graph.selection).toBe('before');
    expect(graph.viewport).toBe('user');
  });

  it('does not write after a preview becomes stale while its baseline is loading', async () => {
    let release!: () => void;
    const baselineGate = new Promise<GraphPreviewBaseline>((resolve) => { release = () => resolve({ selection: 'before', viewport: 'before' }); });
    const { graph, runtime } = createRuntime();
    runtime.capturePreviewBaseline = () => baselineGate;
    const tx = transaction();
    const facade = new GraphNavigationFacade({ runtime, targetReader: { status: () => 'current' } });

    const pending = facade.preview({ ...command(tx.value), previewId: 'preview', mode: 'viewport' });
    tx.expire();
    release();

    await expect(pending).resolves.toEqual({ kind: 'stale' });
    expect(graph.calls).toEqual([]);
  });

  it('retains only the latest graph command until the scene is interactive', async () => {
    const { graph, runtime } = createRuntime();
    let interactive = false;
    runtime.isInteractive = () => interactive;
    const tx = transaction();
    const facade = new GraphNavigationFacade({ runtime, targetReader: { status: () => 'current' } });

    await expect(facade.locate(command(tx.value))).resolves.toEqual({ kind: 'deferred' });
    await expect(facade.navigate(command(tx.value))).resolves.toEqual({ kind: 'deferred' });
    expect(graph.calls).toEqual([]);

    interactive = true;
    await expect(facade.flush({ target, transaction: tx.value })).resolves.toEqual({ kind: 'applied' });
    expect(graph.calls).toEqual(['highlight', 'reveal']);
  });

  it('discards a deferred search preview when it is cancelled before graph readiness', async () => {
    const { graph, runtime } = createRuntime();
    runtime.isInteractive = () => false;
    const tx = transaction();
    const facade = new GraphNavigationFacade({ runtime, targetReader: { status: () => 'current' } });

    await facade.preview({ ...command(tx.value), previewId: 'preview', mode: 'viewport' });
    await expect(facade.cancelPreview({ target, transaction: tx.value, previewId: 'preview' })).resolves.toEqual({ kind: 'applied' });
    expect(graph.calls).toEqual([]);
  });
});
