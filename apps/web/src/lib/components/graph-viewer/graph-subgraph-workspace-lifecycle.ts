// Responsibility: define subgraph-workspace close rules during the full-edit lifecycle.
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
  graphAppliedRevision?: number,
): boolean {
  if (!fullEditUiState?.active || !fullEditUiState.sessionId) return false;
  if (fullEditUiState.phase === 'idle' || fullEditUiState.phase === 'settled') return false;
  if (graphAppliedRevision != null && graphAppliedRevision >= fullEditUiState.revision) return false;
  return (
    fullEditUiState.reason != null &&
    subgraphWorkspaceResetReasonSet.has(fullEditUiState.reason)
  );
}
