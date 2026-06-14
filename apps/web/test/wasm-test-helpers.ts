import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import {
  getSharedWasmWorkerClient,
  setSharedWasmBytes,
  setWorkerFactory,
  shutdownSharedWasmWorker,
} from '../src/lib/wasm/wasm-worker-singleton';

function shouldUseNodeWorker(): boolean {
  return process.env.WASM_TEST_USE_NODE_WORKER === 'true';
}

type WorkerListener = (event: any) => void;

type ListenerMap = Map<string, Set<WorkerListener>>;

type InProcessMessage = {
  id: number;
  type: string;
  [key: string]: any;
};

type InProcessWorker = {
  addEventListener: (type: string, listener: WorkerListener) => void;
  removeEventListener: (type: string, listener: WorkerListener) => void;
  postMessage: (message: InProcessMessage, _transfer?: Transferable[]) => void;
  terminate: () => void;
};

type NodeWorkerLike = InProcessWorker;

const pkgDir = join(process.cwd(), '..', '..', 'packages', 'core', 'wasm', 'pkg');
const wasmFile = join(pkgDir, 'core.wasm');

function resolveWasmPath(): string {
  if (!existsSync(wasmFile)) {
    throw new Error('core.wasm not found in wasm/pkg/; run pnpm wasm:bindgen first');
  }
  return wasmFile;
}

let workerModulePromise: Promise<void> | null = null;
let fetchShimInstalled = false;
let originalFetch: typeof fetch | null = null;

function emit(listeners: ListenerMap, type: string, data: any): void {
  const handlers = listeners.get(type);
  if (!handlers || handlers.size === 0) return;
  if (type === 'error') {
    const message = typeof data?.message === 'string' ? data.message : 'Worker error';
    const baseError = data?.error instanceof Error ? data.error : new Error(message);
    handlers.forEach((handler) =>
      handler({
        message,
        filename: typeof data?.filename === 'string' ? data.filename : '',
        lineno: typeof data?.lineno === 'number' ? data.lineno : 0,
        colno: typeof data?.colno === 'number' ? data.colno : 0,
        error: baseError,
      }),
    );
    return;
  }
  handlers.forEach((handler) => handler({ data } as MessageEvent));
}

function ensureFetchShim(): void {
  if (fetchShimInstalled) return;
  if (typeof fetch !== 'function') return;
  originalFetch = fetch;
  globalThis.fetch = (input: RequestInfo | URL, init?: RequestInit) => {
    const url = typeof input === 'string' ? input : input.toString();
    if (url.startsWith('file://')) {
      const filePath = new URL(url);
      const bytes = readFileSync(filePath);
      const u8 = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes as any);
      const ab = u8.buffer.slice(u8.byteOffset, u8.byteOffset + u8.byteLength);
      const response = {
        ok: true,
        status: 200,
        arrayBuffer: async () => ab,
      };
      return Promise.resolve(response as any);
    }
    return originalFetch ? originalFetch(input, init) : fetch(input, init);
  };
  fetchShimInstalled = true;
}

function restoreFetchShim(): void {
  if (!fetchShimInstalled) return;
  if (originalFetch) {
    globalThis.fetch = originalFetch;
  }
  originalFetch = null;
  fetchShimInstalled = false;
}

async function ensureWorkerModule(ctx: any): Promise<void> {
  if (workerModulePromise) return workerModulePromise;
  ensureFetchShim();
  const previousSelf = (globalThis as any).self;
  (globalThis as any).self = ctx;
  workerModulePromise = import('../src/workers/wasm-runtime.worker')
    .then(() => undefined)
    .finally(() => {
      (globalThis as any).self = previousSelf;
    });
  return workerModulePromise;
}

function createInProcessWorker(): InProcessWorker {
  const listeners: ListenerMap = new Map();
  const ctx: any = {
    postMessage: (data: any) => {
      emit(listeners, 'message', data);
    },
    onmessage: null as null | ((event: MessageEvent) => void),
  };

  function addEventListener(type: string, listener: WorkerListener): void {
    const key = String(type);
    const set = listeners.get(key) ?? new Set();
    set.add(listener);
    listeners.set(key, set);
  }

  function removeEventListener(type: string, listener: WorkerListener): void {
    const set = listeners.get(type);
    if (!set) return;
    set.delete(listener);
    if (set.size === 0) listeners.delete(type);
  }

  function postMessage(message: InProcessMessage, _transfer?: Transferable[]): void {
    void ensureWorkerModule(ctx)
      .then(() => {
        if (!ctx.onmessage) {
          throw new Error('worker onmessage is not initialized');
        }
        ctx.onmessage({ data: message } as MessageEvent);
      })
      .catch((error) => {
        emit(listeners, 'error', { message: error instanceof Error ? error.message : String(error) });
      });
  }

  function terminate(): void {
    listeners.clear();
  }

  return { addEventListener, removeEventListener, postMessage, terminate };
}

function createNodeWorker(): NodeWorkerLike {
  const listeners: ListenerMap = new Map();
  const pendingMessages: InProcessMessage[] = [];
  let worker: any | null = null;
  let ready = false;
  let failed = false;

  const emitMessage = (data: any) => emit(listeners, 'message', data);
  const emitError = (error: unknown) => {
    const message = error instanceof Error ? error.message : String(error);
    const stack = error instanceof Error ? error.stack : undefined;
    const err = new Error(message);
    if (stack) err.stack = stack;
    emit(listeners, 'error', { message: err.message, error: err, stack: err.stack });
  };

  const handleFatalError = (data: any) => {
    failed = true;
    pendingMessages.length = 0;
    const err = new Error(String(data.error ?? 'Worker fatal error'));
    if (typeof data.stack === 'string') {
      err.stack = data.stack;
    }
    emitError(err);
  };

  const ensureWorker = async (): Promise<any> => {
    if (worker) return worker;
    try {
      const { Worker } = await import('node:worker_threads');
      const { fileURLToPath } = await import('node:url');
      const workerURL = new URL('./node-worker-bridge.ts', import.meta.url);
      const loaderURL = new URL('./node-worker-loader.mjs', import.meta.url);
      worker = new Worker(workerURL, {
        type: 'module',
        execArgv: [...process.execArgv, '--loader', fileURLToPath(loaderURL)],
      } as any);
      worker.on('message', (data: any) => {
        if (data && typeof data === 'object' && data.__fatal) {
          handleFatalError(data);
          return;
        }
        emitMessage(data);
      });
      worker.on('messageerror', (error: unknown) => {
        emitError(error);
      });
      worker.on('error', emitError);
      worker.on('exit', (code: number) => {
        if (code !== 0) emitError(new Error(`Worker exited with code ${code}`));
      });
      worker.on('message', (data: any) => {
        if (data && typeof data === 'object' && data.__ready) {
          ready = true;
          while (pendingMessages.length) {
            const msg = pendingMessages.shift();
            if (msg) {
              worker.postMessage(msg);
            }
          }
        }
      });
      return worker;
    } catch (error) {
      emitError(error);
      throw error;
    }
  };

  function addEventListener(type: string, listener: WorkerListener): void {
    const key = String(type);
    const set = listeners.get(key) ?? new Set();
    set.add(listener);
    listeners.set(key, set);
  }

  function removeEventListener(type: string, listener: WorkerListener): void {
    const set = listeners.get(type);
    if (!set) return;
    set.delete(listener);
    if (set.size === 0) listeners.delete(type);
  }

  function postMessage(message: InProcessMessage, transfer?: Transferable[]): void {
    if (failed) {
      emitError(new Error('Worker has failed and cannot accept messages'));
      return;
    }
    void ensureWorker()
      .then((w) => {
        if (failed) {
          emitError(new Error('Worker has failed and cannot accept messages'));
          return;
        }
        if (!ready) {
          pendingMessages.push(message);
          return;
        }
        if (transfer && transfer.length > 0) {
          w.postMessage(message, transfer as any);
        } else {
          w.postMessage(message);
        }
      })
      .catch(emitError);
  }

  function terminate(): void {
    if (worker) {
      worker.terminate();
    }
    worker = null;
    ready = false;
    pendingMessages.length = 0;
    listeners.clear();
  }

  return { addEventListener, removeEventListener, postMessage, terminate };
}

export async function initWasmWorkerForTests(): Promise<void> {
  await shutdownSharedWasmWorker();
  const wasmBytes = readFileSync(resolveWasmPath());
  setSharedWasmBytes(new Uint8Array(wasmBytes).buffer.slice(
    new Uint8Array(wasmBytes).byteOffset,
    new Uint8Array(wasmBytes).byteOffset + new Uint8Array(wasmBytes).byteLength,
  ));
  if (shouldUseNodeWorker()) {
    setWorkerFactory(createNodeWorker);
  } else {
    setWorkerFactory(createInProcessWorker);
  }
  await getSharedWasmWorkerClient();
}

export async function shutdownWasmWorkerForTests(): Promise<void> {
  await shutdownSharedWasmWorker();
  restoreFetchShim();
}
