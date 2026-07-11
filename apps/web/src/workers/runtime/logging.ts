import { createOkResponse, createErrorResponse } from './protocol';
import type { WorkerContext } from './protocol';

export function describeError(error: unknown) {
  if (error instanceof Error) return { name: error.name, message: error.message, stack: error.stack };
  return { name: 'UnknownError', message: String(error), stack: '' };
}

export function postOk(ctx: WorkerContext, id: number, data?: unknown, transfer?: Transferable[]) {
  const response = createOkResponse(id, data);
  if (transfer?.length) {
    ctx.postMessage(response, transfer);
    return;
  }
  ctx.postMessage(response);
}

export function postError(ctx: WorkerContext, id: number, error: string) {
  console.warn('[worker] response', { id, ok: false, error });
  ctx.postMessage(createErrorResponse(id, error));
}
