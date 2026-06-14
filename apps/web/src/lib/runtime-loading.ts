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
  synchronizedRuntimeLoading: boolean;
}): { loading: boolean; phase: string } {
  if (!input.editorRuntimeReady) {
    return {
      loading: true,
      phase: input.editorRuntimePhase || 'Loading editor runtime...',
    };
  }

  if (input.synchronizedRuntimeLoading) {
    return {
      loading: true,
      phase: 'Waiting for graph runtime...',
    };
  }

  return {
    loading: false,
    phase: '',
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
