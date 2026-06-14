// 职责：GraphViewer 测试桥 hook：DEV/test 模式下暴露 runtime probe 与 hover panel 调试状态
import type { GraphRuntimeHoverPanelDebugState, GraphRuntimeHoverPreviewState } from './runtime/scene-types';

export function shouldAttachGraphViewerTestHooks(): boolean {
  return import.meta.env.DEV || import.meta.env.MODE === 'test';
}

export function clearGraphViewerTestHooks(deps: {
  clearRuntimeProbeState: () => void;
  setRuntimeHoverPreviewState: (state: GraphRuntimeHoverPreviewState | null) => void;
  setRuntimeHoverPanelDebugState: (state: GraphRuntimeHoverPanelDebugState) => void;
}): void {
  deps.clearRuntimeProbeState();
  deps.setRuntimeHoverPreviewState(null);
  deps.setRuntimeHoverPanelDebugState({ phase: 'idle', error: '' });
}
