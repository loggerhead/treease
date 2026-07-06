// 职责：定义子图工作区在 full-edit 生命周期中的关闭规则。
import type { FullEditUiState } from '../../store/full-edit-ui-store';

const subgraphWorkspaceResetReasonSet = new Set<NonNullable<FullEditUiState['reason']>>([
  'initial-example',
  'language-example',
  'language-switch',
  'whole-document-replacement',
  'import-file',
  'drop-file',
]);

export function shouldResetSubgraphWorkspaceForFullEdit(
  fullEditUiState: FullEditUiState | null | undefined,
): boolean {
  if (!fullEditUiState?.active || !fullEditUiState.sessionId) return false;
  if (fullEditUiState.phase === 'idle') return false;
  return (
    fullEditUiState.reason != null &&
    subgraphWorkspaceResetReasonSet.has(fullEditUiState.reason)
  );
}
