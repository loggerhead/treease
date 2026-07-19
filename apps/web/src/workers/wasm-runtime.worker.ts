// Responsibility: thin WASM Worker entry point connecting Worker lifecycle to the transport runtime.
import type { WorkerContext, WorkerRequest } from './runtime/protocol';
import { createWorkerTransport } from './runtime/worker-transport';

const ctx = self as unknown as WorkerContext;
const transport = createWorkerTransport(ctx);

ctx.onmessage = (event: MessageEvent<WorkerRequest>) => {
  transport.enqueue(event.data);
};
