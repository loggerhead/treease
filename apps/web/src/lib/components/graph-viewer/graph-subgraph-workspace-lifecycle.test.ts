import { describe, expect, it } from 'vitest';
import type { FullEditUiState } from '../../store/editor-store';
import { shouldResetSubgraphWorkspaceForFullEdit } from './graph-subgraph-workspace-lifecycle';

function createFullEditUiState(
  overrides: Partial<FullEditUiState> = {},
): FullEditUiState {
  return {
    active: true,
    sessionId: 'session-1',
    ownerKey: 'owner-1',
    documentKey: 'doc-1',
    revision: 1,
    streamSeq: 0,
    inputByteLength: 0,
    modelVersionId: null,
    byteLength: 0,
    language: 'json',
    phase: 'streaming',
    sessionKind: 'full-edit',
    transportKind: 'memory',
    reason: 'whole-document-replacement',
    ...overrides,
  };
}

describe('shouldResetSubgraphWorkspaceForFullEdit', () => {
  it('resets workspace for full document replacement sessions', () => {
    expect(
      shouldResetSubgraphWorkspaceForFullEdit(
        createFullEditUiState({ reason: 'whole-document-replacement' }),
      ),
    ).toBe(true);
    expect(
      shouldResetSubgraphWorkspaceForFullEdit(
        createFullEditUiState({ reason: 'import-file' }),
      ),
    ).toBe(true);
    expect(
      shouldResetSubgraphWorkspaceForFullEdit(
        createFullEditUiState({ reason: 'drop-file' }),
      ),
    ).toBe(true);
  });

  it('resets workspace for language-driven rebuild sessions', () => {
    expect(
      shouldResetSubgraphWorkspaceForFullEdit(
        createFullEditUiState({ reason: 'language-switch' }),
      ),
    ).toBe(true);
    expect(
      shouldResetSubgraphWorkspaceForFullEdit(
        createFullEditUiState({ reason: 'language-example' }),
      ),
    ).toBe(true);
    expect(
      shouldResetSubgraphWorkspaceForFullEdit(
        createFullEditUiState({ reason: 'initial-example' }),
      ),
    ).toBe(true);
  });

  it('does not reset workspace after the rebuild has settled, or without a live session', () => {
    expect(
      shouldResetSubgraphWorkspaceForFullEdit(
        createFullEditUiState({ phase: 'settled' }),
      ),
    ).toBe(false);
    expect(
      shouldResetSubgraphWorkspaceForFullEdit(
        createFullEditUiState({ phase: 'idle' }),
      ),
    ).toBe(false);
    expect(
      shouldResetSubgraphWorkspaceForFullEdit(
        createFullEditUiState({ sessionId: null }),
      ),
    ).toBe(false);
    expect(
      shouldResetSubgraphWorkspaceForFullEdit(
        createFullEditUiState({ active: false }),
      ),
    ).toBe(false);
  });

  it('does not reset workspace for tab reactivation', () => {
    expect(
      shouldResetSubgraphWorkspaceForFullEdit(
        createFullEditUiState({ reason: 'tab-reactivate' }),
      ),
    ).toBe(false);
  });

  it('does not reset a Workspace after the full-edit revision has a visible main graph', () => {
    expect(
      shouldResetSubgraphWorkspaceForFullEdit(
        createFullEditUiState({ revision: 4, phase: 'streaming' }),
        4,
      ),
    ).toBe(false);
    expect(
      shouldResetSubgraphWorkspaceForFullEdit(
        createFullEditUiState({ revision: 4, phase: 'streaming' }),
        3,
      ),
    ).toBe(true);
  });
});
