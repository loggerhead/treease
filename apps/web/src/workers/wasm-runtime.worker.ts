// 职责：WASM Worker 薄入口：连接 Worker 生命周期与 transport runtime。
import type { WorkerContext, WorkerRequest } from './runtime/protocol';
import { createWorkerTransport } from './runtime/worker-transport';

const ctx = self as unknown as WorkerContext;
const transport = createWorkerTransport(ctx);

ctx.onmessage = (event: MessageEvent<WorkerRequest>) => {
  transport.enqueue(event.data);
};
