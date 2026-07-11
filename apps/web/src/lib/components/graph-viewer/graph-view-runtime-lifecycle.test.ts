import { describe, expect, it, vi } from 'vitest';
import {
  createGraphViewRuntimeLifecycle,
  disposeGraphViewRuntime,
  isGraphViewRuntimeRenderCurrent,
  type GraphViewRuntimeRenderInput,
} from './graph-view-runtime-lifecycle';

function input(overrides: Partial<GraphViewRuntimeRenderInput> = {}): GraphViewRuntimeRenderInput {
  return {
    fullEditUiState: null,
    jsonBlockSelection: null,
    renderRuntimeReady: true,
    documentKey: 'main',
    language: 'json',
    sourceText: '{"ok":true}',
    editorRevision: 3,
    graphAppliedRevision: 2,
    lastAutoOffset: { x: 12, y: 18 },
    ...overrides,
  };
}

function createLifecycle(progress: { active: boolean }) {
  const calls = {
    attach: vi.fn(),
    json: vi.fn(),
    incremental: vi.fn(),
    complete: vi.fn(),
    clearOffset: vi.fn(),
    cleanup: vi.fn(),
    minimap: vi.fn(),
  };
  const lifecycle = createGraphViewRuntimeLifecycle({
    fullBuildReasons: new Set(['import-file', 'drop-file', 'language-switch', 'whole-document-replacement']),
    setLastAutoOffset: calls.clearOffset,
    isFullEditProgressActive: () => progress.active,
    completeStreamProgress: calls.complete,
    attachFullEditSession: calls.attach,
    renderJsonBlock: calls.json,
    renderIncremental: calls.incremental,
    scheduleFullEditCleanup: calls.cleanup,
    updateMinimap: calls.minimap,
  });
  return { lifecycle, calls };
}

describe('Graph View Runtime lifecycle', () => {
  it('lets a full-edit render own a revision after it settles, so incremental render cannot race it', () => {
    const progress = { active: true };
    const { lifecycle, calls } = createLifecycle(progress);
    const fullEditUiState = {
      active: true,
      documentKey: 'main',
      revision: 3,
      phase: 'streaming',
      reason: 'import-file',
    } as any;

    lifecycle.syncRender(input({ fullEditUiState }));
    expect(calls.clearOffset).toHaveBeenCalledWith(null);
    expect(calls.incremental).toHaveBeenLastCalledWith(expect.objectContaining({ isBlocked: true }));

    progress.active = false;
    lifecycle.syncRender(input({ fullEditUiState: { ...fullEditUiState, active: false, phase: 'idle' } }));
    expect(calls.complete).toHaveBeenCalledTimes(1);
    expect(calls.incremental).toHaveBeenLastCalledWith(expect.objectContaining({ isBlocked: true }));
  });

  it('keeps JSON block rendering mutually exclusive with incremental rendering', () => {
    const progress = { active: false };
    const { lifecycle, calls } = createLifecycle(progress);
    const selection = { blockDocumentKey: 'block', revision: 8, startByte: 0, endByte: 7, text: '{"a":1}' } as any;

    lifecycle.syncRender(input({ jsonBlockSelection: selection }));

    expect(calls.json).toHaveBeenCalledWith(selection, true);
    expect(calls.incremental).toHaveBeenCalledWith(expect.objectContaining({ isBlocked: true }));
  });

  it('settles progress and minimap through one runtime lifecycle', () => {
    const progress = { active: true };
    const { lifecycle, calls } = createLifecycle(progress);
    lifecycle.settle({ active: true, documentKey: 'main', revision: 3, phase: 'settled' } as any, 'main');

    expect(calls.cleanup).toHaveBeenCalledWith('settled', expect.any(Function));
    const task = calls.cleanup.mock.calls[0][1] as () => void;
    task();
    expect(calls.complete).toHaveBeenCalledTimes(1);
    expect(calls.minimap).toHaveBeenCalledTimes(1);
  });

  it('drops stale committed and JSON block render guards before they reach the View Runtime', () => {
    expect(
      isGraphViewRuntimeRenderCurrent(
        { documentKey: 'main', revision: 4, mode: 'committed' },
        { documentKey: 'main', revision: 5, jsonBlockSelection: null },
      ),
    ).toBe(false);
    expect(
      isGraphViewRuntimeRenderCurrent(
        { documentKey: 'block-a', revision: 4, mode: 'json-block' },
        { documentKey: 'main', revision: 5, jsonBlockSelection: { blockDocumentKey: 'block-b', revision: 4 } as any },
      ),
    ).toBe(false);
  });

  it('disposes runtime collaborators in lifecycle order', () => {
    const order: string[] = [];
    const mark = (name: string) => () => order.push(name);
    disposeGraphViewRuntime({
      cleanupHandles: { settled: 1, idle: 2 },
      cancelFrame: (handle) => order.push(`cancel:${handle}`),
      resetCanvasHint: mark('canvas'),
      disposeMeasurement: mark('measurement'),
      disposeRenderEffects: mark('effects'),
      disposeRenderCoordinator: async () => {
        order.push('coordinator');
      },
      disposeScene: mark('scene'),
      resetActiveEditState: mark('edit'),
      disposeSubgraphWorkspace: mark('workspace'),
      unsubscribeStreamProgress: mark('unsubscribe'),
      disposeStreamProgress: mark('progress'),
      resetLifecycle: mark('lifecycle'),
      clearGraphBridge: mark('bridge'),
      resetGraphStreamState: mark('stream-state'),
    });

    expect(order).toEqual([
      'cancel:1', 'cancel:2', 'canvas', 'measurement', 'effects', 'coordinator', 'scene', 'edit', 'workspace',
      'unsubscribe', 'progress', 'lifecycle', 'bridge', 'stream-state',
    ]);
  });
});
