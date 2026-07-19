// Responsibility: expose GraphViewer runtime-probe state through test hooks in DEV/test mode.

export function shouldAttachGraphViewerTestHooks(): boolean {
  return import.meta.env.DEV || import.meta.env.MODE === 'test';
}

export function clearGraphViewerTestHooks(deps: {
  clearRuntimeProbeState: () => void;
}): void {
  deps.clearRuntimeProbeState();
}
