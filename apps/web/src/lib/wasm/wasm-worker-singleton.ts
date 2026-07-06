// 职责：WASM Worker 单例管理：初始化、版本校验、共享 Worker 调度
import type { GraphStreamDeltaTransferEvent, WorkerResponse } from '../../shared/worker-protocol/protocol';
import { decodeGraphStreamDeltaTransferEvent } from '../../shared/worker-protocol/graph-stream-event-codec';
import debounce from 'lodash-es/debounce';
import wasmUrl from '@core-wasm/pkg/core.wasm?url';

const isTestEnv =
  (typeof import.meta !== 'undefined' && import.meta.env?.MODE === 'test') ||
  (typeof process !== 'undefined' && !!process.env?.VITEST);

type PendingRequest = {
  resolve: (value: any) => void;
  reject: (error: Error) => void;
  watchdog?: ReturnType<typeof setInterval>;
  type?: string;
  handle?: number | null;
  chunkBytes?: number | null;
  postedAtMs?: number;
};

type WorkerLike = {
  addEventListener: (type: string, listener: EventListenerOrEventListenerObject) => void;
  removeEventListener: (type: string, listener: EventListenerOrEventListenerObject) => void;
  postMessage: (message: any, transfer?: Transferable[]) => void;
  terminate: () => void;
};




export type WorkerClient = {
  call: <T>(type: string, payload?: Record<string, any>, transfer?: Transferable[]) => Promise<T>;
  onEvent: (event: string, handler: (event: WorkerEvent) => void) => () => void;
  dispose: () => void;
};

export type WorkerEvent = {
  event: string;
  [key: string]: any;
};

function isGraphStreamDeltaTransferEvent(message: Record<string, unknown>): message is GraphStreamDeltaTransferEvent {
  return (
    message.event === 'graphStreamDelta' &&
    typeof message.sessionId === 'string' &&
    typeof message.streamKey === 'string' &&
    typeof message.streamRunId === 'string' &&
    typeof message.eventSeq === 'number' &&
    typeof message.inputByteLength === 'number' &&
    message.deltaBytes instanceof ArrayBuffer &&
    typeof message.final === 'boolean'
  );
}

function isWorkerResponseMessage(message: Record<string, unknown>): message is WorkerResponse {
  return typeof message.id === 'number' && typeof message.ok === 'boolean';
}

let sharedWorker: WorkerLike | null = null;
let sharedClient: WorkerClient | null = null;
let initPromise: Promise<WorkerClient> | null = null;
let initError: Error | null = null;
let alerted = false;
let initializedWasmURL: string | null = null;
let isReady = false;
let chunkSizeConfig: Record<string, number> | null = null;
let sharedWasmBytes: ArrayBuffer | null = null;
let sharedWasmBytesPromise: Promise<ArrayBuffer | null> | null = null;
let workerFactory: (() => WorkerLike | Promise<WorkerLike>) | null = null;
const debouncedTypes = new Set([
  'diagnostics',
  'treePath',
]);
const debounceMs = 60;
type DebounceEntry<T> = {
  start: (() => Promise<T>) & { cancel: () => void };
  cleanup: (() => void) & { cancel: () => void };
};
const debounceMap = new Map<string, DebounceEntry<any>>();
const inFlightMap = new Map<string, Promise<any>>();
const resultCacheMap = new Map<string, any>();
let disposePromise: Promise<void> | null = null;
function getDebounceKey(type: string, payload: Record<string, any>): string | null {
  if (!debouncedTypes.has(type)) return null;
  const language = String(payload?.language ?? '');
  const documentKey = String(payload?.documentKey ?? '');
  const textLength = typeof payload?.text === 'string' ? payload.text.length : 0;
  const nest = payload?.nest ? 1 : 0;
  switch (type) {
    case 'treePath': {
      const row = Number.isFinite(payload?.row) ? payload.row : '';
      const column = Number.isFinite(payload?.column) ? payload.column : '';
      return `treePath|${language}|${documentKey}|${row}|${column}|${textLength}|${nest}`;
    }
    case 'diagnostics':
      return `diagnostics|${language}|${textLength}`;
    default:
      return null;
  }
}

function getRequestKey(type: string, payload: Record<string, any>): string | null {
  const documentKey = String(payload?.documentKey ?? '');
  const nest = payload?.nest ? 1 : 0;
  const text = typeof payload?.text === 'string' ? payload.text : '';
  switch (type) {
    case 'treePath': {
      if (!documentKey) return null;
      const row = Number.isFinite(payload?.row) ? payload.row : '';
      const column = Number.isFinite(payload?.column) ? payload.column : '';
      return `treePath|${documentKey}|${row}|${column}|${nest}|${text}`;
    }
    case 'compare': {
      const language = String(payload?.language ?? '');
      const leftLanguage = String(payload?.leftLanguage ?? language);
      const rightLanguage = String(payload?.rightLanguage ?? language);
      const left = String(payload?.left ?? '');
      const right = String(payload?.right ?? '');
      return `compare|${language}|${leftLanguage}|${rightLanguage}|${left}\u0000${right}`;
    }
    default:
      return null;
  }
}

function getResultCacheKey(type: string, requestKey: string | null): string | null {
  if (!requestKey) return null;
  return requestKey;
}

function normalizeWasmURL(wasmURL: string): string {
  if (wasmURL.startsWith('file:')) {
    return wasmURL.split('?')[0];
  }
  try {
    const base = globalThis.location?.href;
    if (base) {
      const url = new URL(wasmURL, base);
      return `${url.origin}${url.pathname}`;
    }
  } catch {
    return wasmURL.split('?')[0];
  }
  return wasmURL.split('?')[0];
}

export function getDefaultWasmURL(): string {
  return wasmUrl;
}
export function setSharedWasmBytes(bytes: ArrayBuffer): void {
  if (sharedWorker || initPromise || isReady) {
    throw new Error('WASM already initialized; cannot override wasm bytes');
  }
  sharedWasmBytes = bytes;
}

async function ensureSharedWasmBytes(wasmURL: string): Promise<ArrayBuffer | null> {
  if (sharedWasmBytes) return sharedWasmBytes;
  if (sharedWasmBytesPromise) return sharedWasmBytesPromise;
  if (typeof fetch !== 'function') return null;
  sharedWasmBytesPromise = (async () => {
    try {
      const response = await fetch(wasmURL);
      if (!response.ok) {
        throw new Error(`failed to fetch wasm: ${response.status} ${response.statusText}`);
      }
      const bytes = await response.arrayBuffer();
      sharedWasmBytes = bytes;
      return bytes;
    } catch (error) {
      console.warn('[wasm] preload bytes failed; falling back to URL init', error);
      return null;
    } finally {
      sharedWasmBytesPromise = null;
    }
  })();
  return sharedWasmBytesPromise;
}


export function setWorkerFactory(factory: () => WorkerLike | Promise<WorkerLike>): void {
  if (sharedWorker || initPromise || isReady) {
    throw new Error('WASM already initialized; cannot override worker factory');
  }
  workerFactory = factory;
}

export function getSharedWasmWorkerClient(): Promise<WorkerClient> {
  const wasmURL = getDefaultWasmURL();
  return getWasmWorkerClient(wasmURL);
}

export function callSharedWasmWorker<T>(
  type: string,
  payload: Record<string, any> = {},
  transfer: Transferable[] = [],
): Promise<T> {
  const requestKey = getRequestKey(type, payload);
  const resultCacheKey = type === 'treePath' ? null : getResultCacheKey(type, requestKey);
  const inFlightKey = requestKey;
  const cached = getCachedResult<T>(resultCacheKey);
  if (cached) {
    return cached;
  }
  const inFlight = getInFlight<T>(inFlightKey);
  if (inFlight) {
    return inFlight;
  }

  const startWorkerCall = createStartWorkerCall<T>(type, payload, transfer, inFlightKey, resultCacheKey);
  const debounced = maybeCreateDebouncedCall<T>(type, payload, inFlightKey, startWorkerCall);
  if (debounced) {
    return debounced;
  }
  return startWorkerCall();
}


export function getWorkerChunkSizeConfig(): Record<string, number> | null {
  return chunkSizeConfig;
}

function getCachedResult<T>(key: string | null): Promise<T> | null {
  if (!key || !resultCacheMap.has(key)) return null;
  return Promise.resolve(resultCacheMap.get(key) as T);
}

function getInFlight<T>(key: string | null): Promise<T> | null {
  if (!key) return null;
  const inFlight = inFlightMap.get(key) as Promise<T> | undefined;
  return inFlight ?? null;
}

function trackInFlight<T>(key: string | null, promise: Promise<T>): void {
  if (!key) return;
  inFlightMap.set(key, promise);
  const cleanup = () => {
    const current = inFlightMap.get(key);
    if (current === promise) inFlightMap.delete(key);
  };
  promise.then(cleanup).catch(cleanup);
}

function createStartWorkerCall<T>(
  type: string,
  payload: Record<string, any>,
  transfer: Transferable[],
  inFlightKey: string | null,
  resultCacheKey: string | null,
): () => Promise<T> {
  return (): Promise<T> => {
    let payloadForWorker = payload;
    let transferForWorker = transfer;
    const canSendTextAsBytes =
      type === 'format' &&
      typeof payload?.text === 'string' &&
      payload.text.length > 200_000 &&
      typeof TextEncoder === 'function';
    if (canSendTextAsBytes) {
      const { text, ...rest } = payload;
      const bytes = new TextEncoder().encode(text);
      payloadForWorker = { ...rest, textBytes: bytes.buffer, text: undefined };
      transferForWorker = [...transfer, bytes.buffer];
    }
    const clientPromise = getSharedWasmWorkerClient();
    const callPromise = clientPromise.then((client) => {
      return client.call<T>(type, payloadForWorker, transferForWorker);
    });
    trackInFlight(inFlightKey, callPromise);
    if (resultCacheKey) {
      callPromise
        .then((value) => {
          resultCacheMap.set(resultCacheKey, value);
        })
        .catch(() => {});
    }
    return callPromise;
  };
}

function maybeCreateDebouncedCall<T>(
  type: string,
  payload: Record<string, any>,
  inFlightKey: string | null,
  startWorkerCall: () => Promise<T>,
): Promise<T> | null {
  if (isTestEnv) {
    return null;
  }
  const debounceKey = getDebounceKey(type, payload);
  if (!debounceKey) return null;
  let entry = debounceMap.get(debounceKey) as DebounceEntry<T> | undefined;
  if (!entry) {
    const start = debounce(startWorkerCall, debounceMs, {
      leading: true,
      trailing: false,
    }) as unknown as DebounceEntry<T>['start'];
    const cleanup = debounce(
      () => {
        debounceMap.delete(debounceKey);
      },
      debounceMs,
      { leading: false, trailing: true },
    ) as unknown as DebounceEntry<T>['cleanup'];
    entry = { start, cleanup };
    debounceMap.set(debounceKey, entry);
  }
  entry.cleanup();
  const promise = entry.start();
  trackInFlight(inFlightKey, promise);
  return promise;
}

export function createWorkerClient(worker: WorkerLike): WorkerClient {
  let requestId = 0;
  const pending = new Map<number, PendingRequest>();
  const eventListeners = new Map<string, Set<(event: WorkerEvent) => void>>();
  const rejectAll = (error: Error) => {
    pending.forEach((handler) => {
      if (handler.watchdog) clearInterval(handler.watchdog);
      handler.reject(error);
    });
    pending.clear();
  };
  const handleMessage = (event: MessageEvent<WorkerResponse>) => {
    try {
      const message = event.data as Record<string, unknown>;
      if (message && typeof message === 'object' && 'event' in message) {
        const eventName = String((message as WorkerEvent).event ?? '');

        const listeners = eventListeners.get(eventName);
        if (listeners && listeners.size > 0) {
          let workerEvent: WorkerEvent;
          if (eventName === 'graphStreamDelta' && isGraphStreamDeltaTransferEvent(message)) {
            try {
              workerEvent = decodeGraphStreamDeltaTransferEvent(message);
            } catch (error) {
              console.error('[wasm-worker] decodeGraphStreamDeltaTransferEvent failed', error);
              return;
            }
          } else {
            workerEvent = message as WorkerEvent;
          }
          listeners.forEach((listener) => listener(workerEvent));
        }
        return;
      }
      if (!message || typeof message !== 'object') {
        return;
      }
      if (!isWorkerResponseMessage(message)) {
        return;
      }
      const handler = pending.get(message.id);
      if (!handler) {
        return;
      }
      pending.delete(message.id);
      if (handler.watchdog) clearInterval(handler.watchdog);
      if (message.ok) {
        handler.resolve(message.data);
      } else {
        const errorMessage = 'error' in message ? message.error : 'Worker error';
        handler.reject(new Error(errorMessage));
      }
    } catch (error) {
      const err = error instanceof Error ? error : new Error(String(error));
      rejectAll(err);
    }
  };
  const handleError = (event: ErrorEvent) => {
    const message = event.message || 'Worker error';
    const location = event.filename ? ` (${event.filename}:${event.lineno ?? 0}:${event.colno ?? 0})` : '';
    const baseError = event.error instanceof Error ? event.error : new Error(`${message}${location}`);
    console.error('[wasm-worker] error', baseError);
    rejectAll(baseError);
  };
  const handleMessageError = (event: MessageEvent) => {
    const detail = event?.data ? `: ${String(event.data)}` : '';
    console.error('[wasm-worker] message error', detail);
    rejectAll(new Error(`Worker message error${detail}`));
  };
  worker.addEventListener('message', handleMessage);
  worker.addEventListener('error', handleError);
  worker.addEventListener('messageerror', handleMessageError);
  const call = <T>(type: string, payload: Record<string, any> = {}, transfer: Transferable[] = []) => {
    const id = requestId++;
    const message = { id, type, ...payload };
    return new Promise<T>((resolve, reject) => {
      const chunk = payload?.chunk;
      const pendingRequest: PendingRequest = {
        resolve,
        reject,
        type,
        handle: typeof payload?.handle === 'number' ? payload.handle : null,
        chunkBytes: chunk instanceof ArrayBuffer ? chunk.byteLength : null,
        postedAtMs: performance.now(),
      };
      pending.set(id, pendingRequest);
      pendingRequest.watchdog = setInterval(() => {}, 5000);
      try {
        worker.postMessage(message, transfer);
      } catch (error) {
        pending.delete(id);
        reject(error instanceof Error ? error : new Error(String(error)));
      }
    });
  };
  const onEvent = (event: string, handler: (event: WorkerEvent) => void) => {
    const key = String(event ?? '');
    if (!key) return () => {};
    let listeners = eventListeners.get(key);
    if (!listeners) {
      listeners = new Set();
      eventListeners.set(key, listeners);
    }
    listeners.add(handler);
    return () => {
      const nextListeners = eventListeners.get(key);
      if (!nextListeners) return;
      nextListeners.delete(handler);
      if (nextListeners.size === 0) eventListeners.delete(key);
    };
  };
  const dispose = () => {
    rejectAll(new Error('Worker disposed'));
    eventListeners.clear();
    worker.removeEventListener('message', handleMessage);
    worker.removeEventListener('error', handleError);
    worker.removeEventListener('messageerror', handleMessageError);
  };
  return { call, onEvent, dispose };
}

export async function getWasmWorkerClient(wasmURL: string): Promise<WorkerClient> {
  const normalizedURL = normalizeWasmURL(wasmURL);
  if (sharedClient && isReady) {
    if (initializedWasmURL && initializedWasmURL !== normalizedURL) {
      throw new Error('WASM already initialized with a different URL');
    }
    return sharedClient;
  }
  if (initPromise) {
    if (initializedWasmURL && initializedWasmURL !== normalizedURL) {
      throw new Error('WASM initialization already in progress with a different URL');
    }
    return initPromise;
  }
  if (initError && initializedWasmURL && initializedWasmURL !== normalizedURL) {
    throw initError;
  }
  initPromise = (async () => {
    if (!initializedWasmURL) {
      initializedWasmURL = normalizedURL;
    }
    initError = null;
    isReady = false;
    if (!sharedWasmBytes) {
      await ensureSharedWasmBytes(wasmURL);
    }
    if (!sharedWorker) {
      if (workerFactory) {
        sharedWorker = await workerFactory();
      } else {
        const { default: WasmWorker } = await import('../../workers/wasm-runtime.worker?worker');
        sharedWorker = new WasmWorker();
      }
    }
    if (!sharedClient) {
      sharedClient = createWorkerClient(sharedWorker);
    }
    try {
      const initPayload: Record<string, any> = { wasmURL };
      if (sharedWasmBytes) {
        initPayload.wasmBytes = sharedWasmBytes;
      }
      const initCall = sharedClient.call('init', initPayload, []);
      let timeoutHandle: ReturnType<typeof setTimeout> | null = null;
      const timeout = new Promise<never>((_, reject) => {
        timeoutHandle = setTimeout(() => {
          reject(new Error('WASM init timeout'));
        }, 5000);
      });
      try {
        const initResult: any = await Promise.race([initCall, timeout]);
        if (initResult && typeof initResult === 'object' && 'chunkSizeConfig' in initResult) {
          chunkSizeConfig = (initResult as any).chunkSizeConfig as Record<string, number>;
        }
      } finally {
        if (timeoutHandle) clearTimeout(timeoutHandle);
      }
      isReady = true;
      return sharedClient;
    } catch (error) {
      const err = error instanceof Error ? error : new Error(String(error));
      initError = err;
      initPromise = null;
      initializedWasmURL = null;
      isReady = false;
      if (sharedWorker) {
        sharedWorker.terminate();
        sharedWorker = null;
      }
      sharedClient = null;
      if (!alerted) {
        alerted = true;
        console.error('[wasm] init failed', err);
      }
      throw err;
    }
  })();
  return initPromise;
}

function cleanupSharedState(): void {
  debounceMap.forEach((entry) => {
    try {
      entry.start.cancel();
      entry.cleanup.cancel();
    } catch (error) {
      console.warn('[wasm] cleanup debounce entry failed', error);
    }
  });
  debounceMap.clear();
  inFlightMap.clear();
  resultCacheMap.clear();
  initPromise = null;
  initError = null;
  initializedWasmURL = null;
  isReady = false;
  sharedWasmBytes = null;
  sharedWasmBytesPromise = null;
  workerFactory = null;
}

export async function shutdownSharedWasmWorker(): Promise<void> {
  if (disposePromise) return disposePromise;
  disposePromise = (async () => {
    const worker = sharedWorker;
    const client = sharedClient;
    sharedWorker = null;
    sharedClient = null;
    cleanupSharedState();
    if (client) {
      try {
        await Promise.race([
          client.call('dispose', {}),
          new Promise<void>((_, reject) => setTimeout(() => reject(new Error('dispose timeout')), 5000)),
        ]);
      } catch (error) {
        console.warn('[wasm-worker] dispose failed', error);
      }
      client.dispose();
    }
    if (worker) {
      worker.terminate();
    }
  })();
  try {
    await disposePromise;
  } finally {
    disposePromise = null;
  }
}

function scheduleSharedShutdown(): void {
  if (!sharedWorker) return;
  void shutdownSharedWasmWorker();
}

if (import.meta.hot) {
  import.meta.hot.accept(() => {
    void shutdownSharedWasmWorker();
  });
  import.meta.hot.dispose(() => {
    scheduleSharedShutdown();
  });
}

if (typeof window !== 'undefined') {
  window.addEventListener('beforeunload', scheduleSharedShutdown);
}
