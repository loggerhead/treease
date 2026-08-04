import { describe, expect, it, vi } from 'vitest';
import type { GraphNavigationRuntimePort } from './graph-navigation-facade';
import { createWorkspaceNavigationRuntime } from './workspace-navigation-runtime';

const path = [{ tag: 0, key: 'node', index: 0 }] as const;

function createRuntime(
  graphInteractive: () => boolean,
  highlight: GraphNavigationRuntimePort['highlight'],
) {
  return createWorkspaceNavigationRuntime('workspace', [
    { id: 'tab', documentKey: 'document-a', revision: 1 },
  ], 'tab', {
    completeNavigationEnabled: () => false,
    isVisible: () => true,
    editor: { locate: async () => ({ kind: 'applied' }) },
    graph: {
      isInteractive: () => graphInteractive(),
      capturePreviewBaseline: async () => ({ selection: null, viewport: null }),
      highlight,
      reveal: async () => ({ kind: 'applied' }),
      restoreSelection: async () => ({ kind: 'applied' }),
      restoreViewport: async () => ({ kind: 'applied' }),
      cancelViewportTransition: async () => ({ kind: 'applied' }),
    },
    navigator: { apply: async () => ({ kind: 'applied' }) },
  });
}

describe('workspace graph runtime readiness binding', () => {
  it('reports readiness only for the target captured when the graph runtime mounted', async () => {
    let interactive = false;
    const highlight = vi.fn(async () => ({ kind: 'applied' as const }));
    const runtime = createRuntime(() => interactive, highlight);
    const originalTarget = runtime.target('tab')!;
    const staleBinding = runtime.bindGraphRuntime(originalTarget);

    await expect(runtime.dispatch({ kind: 'editor-selection', target: originalTarget, path, cellTarget: 'node' }))
      .resolves.toMatchObject({ results: expect.arrayContaining([{ kind: 'deferred' }]) });

    runtime.sync([{ id: 'tab', documentKey: 'document-b', revision: 1 }], 'tab');
    expect(staleBinding.reportInteractive()).toBeUndefined();
    expect(highlight).not.toHaveBeenCalled();

    const currentTarget = runtime.target('tab')!;
    const currentBinding = runtime.bindGraphRuntime(currentTarget);
    await expect(runtime.dispatch({ kind: 'editor-selection', target: currentTarget, path, cellTarget: 'node' }))
      .resolves.toMatchObject({ results: expect.arrayContaining([{ kind: 'deferred' }]) });

    interactive = true;
    await expect(currentBinding.reportInteractive()).resolves.toMatchObject({ outcome: 'applied' });

    expect(highlight).toHaveBeenCalledOnce();
  });
});
