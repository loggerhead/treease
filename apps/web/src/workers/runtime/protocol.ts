// Responsibility: Worker entry protocol facade; re-export shared types and provide WorkerContext, createOkResponse, and createErrorResponse.
import type { PathSpan } from '@core-wasm/index'
import type { WorkerRequest, WorkerResponse } from '../../shared/worker-protocol/protocol';

export type {
  BuildGraphDeltaEvent,
  BuildGraphProgressEvent,
  CompareResponse,
  DocumentAnalysisResult,
  GraphEdge,
  GraphNode,
  GraphSearchTarget,
  GraphStreamDeltaEvent,
  GraphStreamDeltaTransferEvent,
  GraphStreamProgressEvent,
  JsonBlockAtPositionResult,
  NormalizedTableCellPatch,
  NormalizedGraphDelta,
  PathRequestData,
  PlanGraphValueEditResponse,
  RawStreamGraphDeltaChunk,
  ReplaceReason,
  RawTableCellPatch,
  RawGraphDelta,
  StreamProgressPhase,
  StreamGraphDeltaResult,
  WasmErrorLike,
  WorkerRequest,
  WorkerResponse,
  WorkerStreamGraphDeltaResult,
} from '../../shared/worker-protocol/protocol';

export type WorkerContext = {
  postMessage: (message: unknown, transfer?: Transferable[]) => void;
  onmessage: ((event: MessageEvent<WorkerRequest>) => void) | null;
  addEventListener?: (type: string, listener: EventListenerOrEventListenerObject) => void;
};

export function createOkResponse(id: number, data?: any): WorkerResponse {
  return { id, ok: true, data };
}

export function createErrorResponse(id: number, error: string): WorkerResponse {
  return { id, ok: false, error };
}

export function isPathSpan(value: unknown): value is PathSpan {
  return !!value && typeof value === 'object' && 'startByte' in (value as Record<string, unknown>);
}
