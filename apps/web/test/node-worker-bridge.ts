import { parentPort } from 'node:worker_threads';
import { readFileSync } from 'node:fs';

if (!parentPort) {
  throw new Error('parentPort not available');
}

// oxlint-disable-next-line no-shadow-restricted-names
declare const globalThis: typeof global & { self?: any };
const ctx = {
  postMessage: (data: any) => {
    parentPort.postMessage(data);
  },
  onmessage: null as null | ((event: MessageEvent) => void),
};

(globalThis as any).self = ctx;

process.on('uncaughtException', (error) => {
  parentPort.postMessage({
    __fatal: true,
    error: error instanceof Error ? error.message : String(error),
    stack: error instanceof Error ? error.stack : undefined,
  });
});
process.on('unhandledRejection', (reason) => {
  parentPort.postMessage({
    __fatal: true,
    error: reason instanceof Error ? reason.message : String(reason),
    stack: reason instanceof Error ? reason.stack : undefined,
  });
});

const boot = async (): Promise<void> => {
  try {
    const originalFetch = typeof fetch === 'function' ? fetch : null;
    if (originalFetch) {
      globalThis.fetch = (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === 'string' ? input : input.toString();
        if (url.startsWith('file://')) {
          const fileURL = new URL(url);
          const bytes = readFileSync(fileURL);
          const u8 = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes as any);
          const ab = u8.buffer.slice(u8.byteOffset, u8.byteOffset + u8.byteLength);
          const response = { ok: true, status: 200, arrayBuffer: async () => ab };
          return Promise.resolve(response as any);
        }
        return originalFetch(input as any, init);
      };
    }
    await import('../src/workers/wasm-runtime.worker');
    parentPort.postMessage({ __ready: true });
  } catch (error) {
    parentPort.postMessage({
      __fatal: true,
      error: error instanceof Error ? error.message : String(error),
      stack: error instanceof Error ? error.stack : undefined,
    });
    throw error;
  }
};

void boot();

parentPort.on('message', (data) => {
  if (!ctx.onmessage) {
    return;
  }
  ctx.onmessage({ data } as MessageEvent);
});

parentPort.on('messageerror', (error) => {
  console.error('[node-worker-bridge] messageerror', error);
});
