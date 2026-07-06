// 职责：GraphViewer 高亮与 hover prewarm effect 判定：signature 构建、shouldApply、shouldRunPrewarm
import type { GraphHighlightState } from '../../store/graph-selection-store';
import type { PathSeg } from '../../store/tree-path';

export function buildGraphHighlightSignature(
  graphHighlight: GraphHighlightState | null,
  buildPathKey: (path: PathSeg[]) => string,
): string {
  if (!graphHighlight?.path?.length) return '';
  return `${graphHighlight.revision}|${graphHighlight.target ?? 'auto'}|${buildPathKey(graphHighlight.path)}`;
}

export function shouldApplyGraphHighlight(input: {
  hasLeafer: boolean;
  isBlocked: boolean;
  graphHighlight: GraphHighlightState | null;
  graphHighlightSignature: string;
  appliedRevision: number;
  lastAppliedSignature: string;
  lastAppliedRevision: number;
}): boolean {
  if (input.isBlocked || !input.graphHighlightSignature || !input.graphHighlight?.path?.length) return false;
  return (
    input.hasLeafer &&
    input.appliedRevision >= input.graphHighlight.revision &&
    (input.graphHighlightSignature !== input.lastAppliedSignature || input.appliedRevision !== input.lastAppliedRevision)
  );
}

export function shouldRunHoverPanelPrewarm(input: {
  pendingRevision: number;
  editorRevision: number;
  graphAppliedRevision: number;
  isBlocked: boolean;
}): boolean {
  return (
    input.pendingRevision === input.editorRevision &&
    input.graphAppliedRevision >= input.editorRevision &&
    !input.isBlocked
  );
}
