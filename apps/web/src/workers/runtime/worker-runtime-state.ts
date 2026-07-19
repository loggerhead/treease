// Responsibility: create and clear global Worker runtime state while composing document and graph state.
import { createGraphStateService } from './graph-state-service';
import { clearDocumentRuntimeState, createDocumentRuntimeState, type DocumentAnalysisCacheRuntime } from './document-runtime-state';

export type WorkerRuntimeState = DocumentAnalysisCacheRuntime & {
  graphStateService: ReturnType<typeof createGraphStateService>;
  documentRuntimeState: DocumentAnalysisCacheRuntime;
};

export function createWorkerRuntimeState(encoder: TextEncoder): WorkerRuntimeState {
  const documentRuntimeState = createDocumentRuntimeState(encoder);
  return {
    ...documentRuntimeState,
    documentRuntimeState,
    graphStateService: createGraphStateService(),
  };
}

export function clearWorkerRuntimeState(state: WorkerRuntimeState): void {
  state.graphStateService.clearAllGraphStates();
  clearDocumentRuntimeState(state.documentRuntimeState);
}
