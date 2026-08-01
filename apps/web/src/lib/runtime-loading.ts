export type RuntimeStateEventDetail = {
  ready: boolean;
  loading: boolean;
  error: boolean;
  phase?: string;
};

export type RuntimeViewMode = 'graph' | 'text';

export function computeSynchronizedRuntimeLoading(input: {
  viewMode: RuntimeViewMode;
  editorRuntimeLoading: boolean;
  graphRuntimeLoading: boolean;
}): boolean {
  return input.editorRuntimeLoading || (input.viewMode === 'graph' && input.graphRuntimeLoading);
}

export function resolveEditorRuntimeOverlay(input: {
  editorRuntimeReady: boolean;
  editorRuntimePhase: string;
  /** Runtime failures are terminal for this capability, not a page-wide load. */
  editorRuntimeError?: boolean;
  // Kept in the input for compatibility with callers; Editor readiness must not
  // depend on Graph or any other peer runtime.
  synchronizedRuntimeLoading?: boolean;
}): { loading: boolean; phase: string } {
  if (input.editorRuntimeReady || input.editorRuntimeError) {
    return {
      loading: false,
      phase: input.editorRuntimeError ? input.editorRuntimePhase : '',
    };
  }

  return {
    loading: true,
    phase: input.editorRuntimePhase || 'Loading editor runtime...',
  };
}

export function shouldShowGraphRuntimeLoading(input: {
  graphRuntimeReady: boolean;
  synchronizedRuntimeLoading: boolean;
  errorMessage: string;
}): boolean {
  if (input.errorMessage) return false;
  return !input.graphRuntimeReady || input.synchronizedRuntimeLoading;
}
