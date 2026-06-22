import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  callSharedWasmWorker,
  createWorkerClient,
  getWasmWorkerClient,
  setWorkerFactory,
  shutdownSharedWasmWorker,
} from './wasm-worker-singleton';

const mocked = vi.hoisted(() => ({
  decodeGraphDeltaPayload: vi.fn(),
}));

vi.mock('@core-wasm/index', () => ({
  decodeGraphDeltaPayload: mocked.decodeGraphDeltaPayload,
  SemType: { MAP: 1, SEQ: 2, STR: 3, INT: 4, FLOAT: 5, BOOLEAN: 6, NIL: 7 },
  TreeKind: { MAPPING: 10, SEQUENCE: 11 },
  PathSegTag: { KEY: 0, INDEX: 1 },
  GraphKind: { SCALAR: 0, TABLE: 1, OBJECT: 2 },
}));

type MockWorkerOptions = {
  initDelayMs?: number;
  failInit?: boolean;
  callDelayMs?: number;
};

type MockWorkerState = {
  terminated: boolean;
  posts: Array<{ id: number; type: string; payload: any }>;
};

function createMockWorker(options: MockWorkerOptions = {}) {
  const listeners = new Map<string, Set<(event: any) => void>>();
  const state: MockWorkerState = { terminated: false, posts: [] };

  const emit = (type: string, data: any) => {
    const handlers = listeners.get(type);
    if (!handlers) return;
    handlers.forEach((handler) => handler({ data }));
  };

  const worker = {
    addEventListener(type: string, listener: (event: any) => void) {
      const set = listeners.get(type) ?? new Set();
      set.add(listener);
      listeners.set(type, set);
    },
    removeEventListener(type: string, listener: (event: any) => void) {
      const set = listeners.get(type);
      if (!set) return;
      set.delete(listener);
      if (set.size === 0) listeners.delete(type);
    },
    postMessage(message: any) {
      state.posts.push({ id: message.id, type: message.type, payload: message });
      if (message.type === 'init') {
        const delay = options.initDelayMs ?? 0;
        setTimeout(() => {
          if (options.failInit) {
            emit('message', { id: message.id, ok: false, error: 'init failed' });
            return;
          }
          emit('message', { id: message.id, ok: true, data: true });
        }, delay);
        return;
      }
      const delay = options.callDelayMs ?? 0;
      setTimeout(() => {
        emit('message', { id: message.id, ok: true, data: { echoedType: message.type } });
      }, delay);
    },
    terminate() {
      state.terminated = true;
      listeners.clear();
    },
  };

  return { worker, state };
}

describe('wasm-worker-singleton', () => {
  beforeEach(async () => {
    await shutdownSharedWasmWorker();
    vi.restoreAllMocks();
    mocked.decodeGraphDeltaPayload.mockReset();
    vi.unstubAllGlobals();
  });

  it('reuses one client during concurrent init', async () => {
    let factoryCalls = 0;
    const { worker, state } = createMockWorker({ initDelayMs: 10 });
    setWorkerFactory(() => {
      factoryCalls += 1;
      return worker;
    });

    const p1 = getWasmWorkerClient('file://fake-a.wasm');
    const p2 = getWasmWorkerClient('file://fake-a.wasm');
    const [c1, c2] = await Promise.all([p1, p2]);

    expect(c1).toBe(c2);
    expect(factoryCalls).toBe(1);
    expect(state.posts.filter((item) => item.type === 'init')).toHaveLength(1);
  });

  it('does not debounce diagnostics in test env', async () => {
    const { worker, state } = createMockWorker({ callDelayMs: 20 });
    setWorkerFactory(() => worker);

    const payload = {
      documentKey: 'vitest://cache/1',
      language: 'json',
      text: '{"a":1}',
      nest: true,
    };

    const p1 = callSharedWasmWorker<{ echoedType: string }>('diagnostics', payload);
    const p2 = callSharedWasmWorker<{ echoedType: string }>('diagnostics', payload);
    const [r1, r2] = await Promise.all([p1, p2]);

    expect(r1).toEqual({ echoedType: 'diagnostics' });
    expect(r2).toEqual({ echoedType: 'diagnostics' });
    expect(state.posts.filter((item) => item.type === 'diagnostics')).toHaveLength(2);
  });

  it('sends large format requests as textBytes', async () => {
    const { worker, state } = createMockWorker();
    setWorkerFactory(() => worker);

    await callSharedWasmWorker<{ echoedType: string }>('format', {
      language: 'json',
      text: 'x'.repeat(200_001),
      options: { tabSize: 2, insertSpaces: true },
      nest: false,
    });

    const formatPost = state.posts.find((item) => item.type === 'format');
    expect(formatPost).toBeDefined();
    expect(formatPost?.payload.text).toBeUndefined();
    expect(formatPost?.payload.textBytes).toBeInstanceOf(ArrayBuffer);
    expect((formatPost?.payload?.textBytes as ArrayBuffer | undefined)?.byteLength).toBeGreaterThan(200_000);
  });

  it('deduplicates in-flight compare requests with same payload', async () => {
    const { worker, state } = createMockWorker({ callDelayMs: 20 });
    setWorkerFactory(() => worker);

    const payload = {
      language: 'json',
      leftLanguage: 'json',
      rightLanguage: 'yaml',
      left: '{"a":1}',
      right: 'a: 1\n',
    };

    const p1 = callSharedWasmWorker<{ echoedType: string }>('compare', payload);
    const p2 = callSharedWasmWorker<{ echoedType: string }>('compare', payload);
    const [r1, r2] = await Promise.all([p1, p2]);

    expect(r1).toEqual({ echoedType: 'compare' });
    expect(r2).toEqual({ echoedType: 'compare' });
    expect(state.posts.filter((item) => item.type === 'compare')).toHaveLength(1);
  });

  it('does not deduplicate compare requests with different payloads', async () => {
    const { worker, state } = createMockWorker({ callDelayMs: 20 });
    setWorkerFactory(() => worker);

    const payloadA = {
      language: 'json',
      leftLanguage: 'json',
      rightLanguage: 'yaml',
      left: '{"a":1}',
      right: 'a: 1\n',
    };
    const payloadB = {
      ...payloadA,
      right: 'a: 2\n',
    };

    await Promise.all([
      callSharedWasmWorker<{ echoedType: string }>('compare', payloadA),
      callSharedWasmWorker<{ echoedType: string }>('compare', payloadB),
    ]);

    expect(state.posts.filter((item) => item.type === 'compare')).toHaveLength(2);
  });

  it('clears in-flight compare entry after failure so retry can proceed', async () => {
    const listeners = new Map<string, Set<(event: any) => void>>();
    let compareCalls = 0;
    const worker = {
      addEventListener(type: string, listener: (event: any) => void) {
        const set = listeners.get(type) ?? new Set();
        set.add(listener);
        listeners.set(type, set);
      },
      removeEventListener(type: string, listener: (event: any) => void) {
        const set = listeners.get(type);
        if (!set) return;
        set.delete(listener);
      },
      postMessage(message: any) {
        if (message.type === 'init') {
          listeners.get('message')?.forEach((handler) => handler({ data: { id: message.id, ok: true, data: true } }));
          return;
        }
        if (message.type === 'compare') {
          compareCalls += 1;
          if (compareCalls === 1) {
            listeners.get('message')?.forEach((handler) =>
              handler({ data: { id: message.id, ok: false, error: 'compare failed once' } }),
            );
            return;
          }
          listeners
            .get('message')
            ?.forEach((handler) => handler({ data: { id: message.id, ok: true, data: { echoedType: 'compare' } } }));
        }
      },
      terminate() {},
    };

    setWorkerFactory(() => worker as any);

    const payload = {
      language: 'json',
      leftLanguage: 'json',
      rightLanguage: 'yaml',
      left: '{"a":1}',
      right: 'a: 1\n',
    };

    await expect(callSharedWasmWorker('compare', payload)).rejects.toThrow('compare failed once');
    await expect(callSharedWasmWorker('compare', payload)).resolves.toEqual({ echoedType: 'compare' });
    expect(compareCalls).toBe(2);
  });

  it('does not cache treePath results across calls', async () => {
    const listeners = new Map<string, Set<(event: any) => void>>();
    let treePathCalls = 0;
    const worker = {
      addEventListener(type: string, listener: (event: any) => void) {
        const set = listeners.get(type) ?? new Set();
        set.add(listener);
        listeners.set(type, set);
      },
      removeEventListener(type: string, listener: (event: any) => void) {
        const set = listeners.get(type);
        if (!set) return;
        set.delete(listener);
      },
      postMessage(message: any) {
        if (message.type === 'init') {
          listeners.get('message')?.forEach((handler) => handler({ data: { id: message.id, ok: true, data: true } }));
          return;
        }
        if (message.type === 'treePath') {
          treePathCalls += 1;
          const data = treePathCalls === 1 ? [] : [{ key: 'user', index: 0, tag: 0 }];
          listeners.get('message')?.forEach((handler) => handler({ data: { id: message.id, ok: true, data } }));
        }
      },
      terminate() {},
    };

    setWorkerFactory(() => worker as any);

    const payload = {
      documentKey: 'doc-key-1',
      language: 'json',
      text: '{"user":{"name":"Ada"}}',
      row: 1,
      column: 5,
      snapshotId: null,
      nest: false,
    };

    await expect(callSharedWasmWorker('treePath', payload)).resolves.toEqual([]);
    await expect(callSharedWasmWorker('treePath', payload)).resolves.toEqual([{ key: 'user', index: 0, tag: 0 }]);
    expect(treePathCalls).toBe(2);
  });

  it('recreates worker after shutdown', async () => {
    const created: MockWorkerState[] = [];
    setWorkerFactory(() => {
      const { worker, state } = createMockWorker();
      created.push(state);
      return worker;
    });

    await getWasmWorkerClient('file://fake-c.wasm');
    expect(created).toHaveLength(1);

    await shutdownSharedWasmWorker();
    expect(created[0].terminated).toBe(true);

    setWorkerFactory(() => {
      const { worker, state } = createMockWorker();
      created.push(state);
      return worker;
    });
    await getWasmWorkerClient('file://fake-c.wasm');

    expect(created).toHaveLength(2);
    expect(created[1].terminated).toBe(false);
  });

  it('allows retry after init failure', async () => {
    let factoryCalls = 0;
    setWorkerFactory(() => {
      factoryCalls += 1;
      if (factoryCalls === 1) {
        return createMockWorker({ failInit: true }).worker;
      }
      return createMockWorker().worker;
    });

    await expect(getWasmWorkerClient('file://fake-d.wasm')).rejects.toThrow('init failed');

    const client = await getWasmWorkerClient('file://fake-d.wasm');
    expect(typeof client.call).toBe('function');
    expect(typeof client.onEvent).toBe('function');
    expect(typeof client.dispose).toBe('function');
    expect(factoryCalls).toBe(2);
  });

  it('prefers preloaded wasm bytes during init when fetch is available', async () => {
    const bytes = new Uint8Array([0, 97, 115, 109]).buffer;
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => ({
        ok: true,
        arrayBuffer: async () => bytes,
      })),
    );

    const { worker, state } = createMockWorker();
    setWorkerFactory(() => worker);

    await getWasmWorkerClient('http://127.0.0.1:4173/wasm/core.wasm');

    const initPost = state.posts.find((item) => item.type === 'init');
    expect(initPost?.payload.wasmBytes).toBeInstanceOf(ArrayBuffer);
    expect(initPost?.payload.wasmBytes.byteLength).toBe(bytes.byteLength);
  });


  it('createWorkerClient dispatches worker events to listeners', async () => {
    const listeners = new Map<string, Set<(event: any) => void>>();
    const worker = {
      addEventListener(type: string, listener: (event: any) => void) {
        const set = listeners.get(type) ?? new Set();
        set.add(listener);
        listeners.set(type, set);
      },
      removeEventListener(type: string, listener: (event: any) => void) {
        const set = listeners.get(type);
        if (!set) return;
        set.delete(listener);
        if (set.size === 0) listeners.delete(type);
      },
      postMessage() {},
      terminate() {},
    };

    const client = createWorkerClient(worker as any);
    const onGraphDelta = vi.fn();
    const unsubscribe = client.onEvent('graphDelta', onGraphDelta);

    listeners.get('message')?.forEach((handler) => {
      handler({ data: { event: 'graphDelta', payload: { eventSeq: 1 } } });
    });

    expect(onGraphDelta).toHaveBeenCalledTimes(1);
    expect(onGraphDelta).toHaveBeenCalledWith({ event: 'graphDelta', payload: { eventSeq: 1 } });

    unsubscribe();
    listeners.get('message')?.forEach((handler) => {
      handler({ data: { event: 'graphDelta', payload: { eventSeq: 2 } } });
    });

    expect(onGraphDelta).toHaveBeenCalledTimes(1);
    client.dispose();
  });

  it('createWorkerClient decodes transferable graph stream deltas before dispatch', async () => {
    mocked.decodeGraphDeltaPayload.mockReturnValue({
      clear: 1,
      nodesAdded: [],
      nodesUpdated: [],
      nodesRemoved: [],
      edgesAdded: [],
      edgesRemoved: [],
    });
    const listeners = new Map<string, Set<(event: any) => void>>();
    const worker = {
      addEventListener(type: string, listener: (event: any) => void) {
        const set = listeners.get(type) ?? new Set();
        set.add(listener);
        listeners.set(type, set);
      },
      removeEventListener(type: string, listener: (event: any) => void) {
        const set = listeners.get(type);
        if (!set) return;
        set.delete(listener);
        if (set.size === 0) listeners.delete(type);
      },
      postMessage() {},
      terminate() {},
    };

    const client = createWorkerClient(worker as any);
    const onGraphStreamDelta = vi.fn();
    client.onEvent('graphStreamDelta', onGraphStreamDelta);

    const deltaBytes = new Uint8Array([1, 2, 3]).buffer;
    listeners.get('message')?.forEach((handler) => {
      handler({
        data: {
          event: 'graphStreamDelta',
          sessionId: 'session-1',
          streamKey: 'stream-1',
          streamRunId: 'session-1:run:3',
          eventSeq: 7,
          inputByteLength: 256,
          deltaBytes,
          final: false,
        },
      });
    });

    expect(mocked.decodeGraphDeltaPayload).toHaveBeenCalledWith(new Uint8Array([1, 2, 3]));
    expect(onGraphStreamDelta).toHaveBeenCalledWith({
      event: 'graphStreamDelta',
      sessionId: 'session-1',
      streamKey: 'stream-1',
      documentKey: undefined,
      streamRunId: 'session-1:run:3',
      eventSeq: 7,
      inputByteLength: 256,
      delta: {
        normalized: true,
        clear: 1,
        nodesAdded: [],
        nodesUpdated: [],
        nodesRemoved: [],
        edgesAdded: [],
        edgesRemoved: [],
        tableCellPatches: [],
        tablePatches: [],
        layoutPatches: [],
      },
      final: false,
    });
    client.dispose();
  });

  it('createWorkerClient normalizes streamed graph path keys before dispatch', async () => {
    mocked.decodeGraphDeltaPayload.mockReturnValue({
      clear: 0,
      nodesAdded: [
        {
          renderHandle: 9,
          kind: 1,
          depth: 0,
          boxArgs: { x: 0, y: 0, width: 10, height: 10, cornerRadius: 0 },
          path: [{ tag: 0, key: { deref: () => 'preview' }, index: 0 }],
          meta: {
            text: 'preview',
            value: '{7}',
            semType: 1,
            path: [{ tag: 0, key: { toString: () => 'preview' }, index: 0 }],
          },
          rows: [
            {
              boxArgs: { x: 0, y: 0, width: 10, height: 10, cornerRadius: 0 },
              cellBoxArgs: { x: 0, y: 0, width: 10, height: 10, cornerRadius: 0 },
              cells: [
                {
                  text: 'color',
                  value: 'color',
                  semType: 2,
                  path: [
                    { tag: 0, key: { valueOf: () => 'preview' }, index: 0 },
                    { tag: 0, key: { deref: () => 'color' }, index: 0 },
                  ],
                },
                {
                  text: '#4f46e5',
                  value: '#4f46e5',
                  semType: 2,
                  path: [
                    { tag: 0, key: { deref: () => 'preview' }, index: 0 },
                    { tag: 0, key: { toString: () => 'color' }, index: 0 },
                  ],
                },
              ],
            },
          ],
          nodesUpdated: [],
        },
      ],
      nodesUpdated: [],
      nodesRemoved: [],
      edgesAdded: [],
      edgesRemoved: [],
    });
    const listeners = new Map<string, Set<(event: any) => void>>();
    const worker = {
      addEventListener(type: string, listener: (event: any) => void) {
        const set = listeners.get(type) ?? new Set();
        set.add(listener);
        listeners.set(type, set);
      },
      removeEventListener(type: string, listener: (event: any) => void) {
        const set = listeners.get(type);
        if (!set) return;
        set.delete(listener);
        if (set.size === 0) listeners.delete(type);
      },
      postMessage() {},
      terminate() {},
    };

    const client = createWorkerClient(worker as any);
    const onGraphStreamDelta = vi.fn();
    client.onEvent('graphStreamDelta', onGraphStreamDelta);

    listeners.get('message')?.forEach((handler) => {
      handler({
        data: {
          event: 'graphStreamDelta',
          sessionId: 'session-2',
          streamKey: 'stream-2',
          streamRunId: 'session-2:run:4',
          eventSeq: 8,
          inputByteLength: 512,
          deltaBytes: new Uint8Array([4, 5, 6]).buffer,
          final: false,
        },
      });
    });

    expect(onGraphStreamDelta).toHaveBeenCalledWith(
      expect.objectContaining({
        delta: expect.objectContaining({
          nodesAdded: [
            expect.objectContaining({
              path: [{ tag: 0, key: 'preview', index: 0 }],
              meta: expect.objectContaining({
                path: [{ tag: 0, key: 'preview', index: 0 }],
              }),
              rows: [
                expect.objectContaining({
                  cells: [
                    expect.objectContaining({
                      path: [
                        { tag: 0, key: 'preview', index: 0 },
                        { tag: 0, key: 'color', index: 0 },
                      ],
                    }),
                    expect.objectContaining({
                      path: [
                        { tag: 0, key: 'preview', index: 0 },
                        { tag: 0, key: 'color', index: 0 },
                      ],
                    }),
                  ],
                }),
              ],
            }),
          ],
        }),
      }),
    );
    client.dispose();
  });
});
