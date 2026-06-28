// 职责：GraphViewer 测试桥 hook：DEV/test 模式下暴露 runtime probe 状态

export function shouldAttachGraphViewerTestHooks(): boolean {
  return import.meta.env.DEV || import.meta.env.MODE === 'test';
}

export function clearGraphViewerTestHooks(deps: {
  clearRuntimeProbeState: () => void;
}): void {
  deps.clearRuntimeProbeState();
}
