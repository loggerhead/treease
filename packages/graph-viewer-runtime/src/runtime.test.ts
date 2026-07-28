import { describe, expect, it, vi } from 'vitest';
import { GraphViewerRuntime } from './runtime';

function host(): HTMLElement {
  return { addEventListener: vi.fn(), removeEventListener: vi.fn() } as unknown as HTMLElement;
}

describe('GraphViewerRuntime delta contract', () => {
  it('rejects a delta before the first graph replacement', async () => {
    const onError = vi.fn();
    const runtime = new GraphViewerRuntime({ host: host(), interaction: { onError } });
    await expect(runtime.applyDelta({})).rejects.toThrow('before replaceGraph');
    expect(onError).toHaveBeenCalledOnce();
  });

  it('requires a host-owned delta reducer', async () => {
    const onError = vi.fn();
    const runtime = new GraphViewerRuntime({ host: host(), interaction: { onError } });
    (runtime as unknown as { graph: { nodes: never[]; edges: never[] } }).graph = { nodes: [], edges: [] };
    await expect(runtime.applyDelta({})).rejects.toThrow('host delta reducer');
    expect(onError).toHaveBeenCalledOnce();
  });
});
