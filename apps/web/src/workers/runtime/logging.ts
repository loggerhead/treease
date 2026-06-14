export function describeError(error: unknown) {
  if (error instanceof Error) return { name: error.name, message: error.message, stack: error.stack };
  return { name: 'UnknownError', message: String(error), stack: '' };
}

import { createOkResponse, createErrorResponse } from './protocol';

type AnalysisResponseWithTokens = { semanticTokens?: ArrayBuffer } | null;

export function postOk(ctx: any, id: number, data?: any, transfer?: Transferable[]) {
  const response = createOkResponse(id, data);
  if (transfer?.length) {
    ctx.postMessage(response, transfer);
    return;
  }
  ctx.postMessage(response);
}

export function postAnalysisOk<T extends AnalysisResponseWithTokens>(ctx: any, id: number, data: T) {
  if (!data?.semanticTokens) {
    postOk(ctx, id, data);
    return;
  }
  const semanticTokens = data.semanticTokens.slice(0);
  postOk(ctx, id, { ...data, semanticTokens }, [semanticTokens]);
}

export function postError(ctx: any, id: number, error: string) {
  console.warn('[worker] response', { id, ok: false, error });
  ctx.postMessage(createErrorResponse(id, error));
}

