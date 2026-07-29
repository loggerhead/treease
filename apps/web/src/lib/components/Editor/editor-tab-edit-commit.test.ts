import { beforeEach, describe, expect, it, vi } from 'vitest';
import { commitEditorTabTextChange } from './editor-tab-edit-commit';
import { commitApplyEdits } from '../../services/DocumentCommitService';
import {
  clearActiveDocumentSemanticState,
  getActiveDocumentCommitBaseSnapshotId,
  getActiveDocumentSemanticState,
  markActiveDocumentSemanticValid,
} from '../../store/active-document-authority';

vi.mock('../../services/DocumentCommitService', () => ({
  commitApplyEdits: vi.fn(),
}));

function createOptions(overrides: Partial<Parameters<typeof commitEditorTabTextChange>[0]> = {}) {
  let revision = 0;
  return {
    requestModel: {
      getValue: () => '{"a":',
      getVersionId: () => 1,
    } as any,
    requestLanguage: 'json' as any,
    requestDocumentKey: 'doc-json',
    nextText: '{"a":',
    documentTextEdits: [{ startByte: 5, oldEndByte: 6, newEndByte: 5, text: '' }] as any,
    baseSnapshotId: 7 as any,
    settings: {} as any,
    builderConfig: {} as any,
    commitRevision: () => {
      revision += 1;
      return revision;
    },
    isFresh: () => true,
    applyGraphAnalysis: vi.fn(),
    ...overrides,
  } satisfies Parameters<typeof commitEditorTabTextChange>[0];
}

describe('commitEditorTabTextChange', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    clearActiveDocumentSemanticState();
  });

  it('keeps parseFailed snapshot as the next commit base without binding a successful snapshot', async () => {
    vi.mocked(commitApplyEdits).mockResolvedValueOnce({
      status: 'parseFailed',
      snapshotId: 11 as any,
      analysis: {
        documentKey: 'doc-json',
        tree: null,
        value: null,
        diagnostics: [{ message: 'Syntax error' }],
        semanticTokens: new ArrayBuffer(0),
      } as any,
      sourceText: null,
      jobHandle: 1,
      batch: { requestSeq: 1, events: [], terminal: { type: 'parseFailed' } } as any,
    });
    const options = createOptions();
    markActiveDocumentSemanticValid({
      documentKey: 'doc-json',
      language: 'json',
      revision: 0,
      snapshotId: 7 as any,
    });

    const revision = commitEditorTabTextChange(options);

    expect(revision).toBe(1);
    expect(getActiveDocumentSemanticState('doc-json')).toEqual(
      expect.objectContaining({
        status: 'pendingWholeDocument',
        snapshotId: null,
        revision: 1,
      }),
    );
    await vi.waitFor(() => {
      expect(getActiveDocumentSemanticState('doc-json')).toEqual(
        expect.objectContaining({
          status: 'invalidWholeDocument',
          snapshotId: 11,
          revision: 1,
        }),
      );
    });
    expect(getActiveDocumentCommitBaseSnapshotId('doc-json')).toBe(11);
    expect(options.applyGraphAnalysis).toHaveBeenCalledWith(
      options.requestModel,
      'json',
      'doc-json',
      1,
      expect.objectContaining({ diagnostics: [{ message: 'Syntax error' }] }),
    );
  });

  it('wraps a large text commit in the usage callback', async () => {
    const runUsage = vi.fn(async (_source: string, execute: () => Promise<unknown>) => execute());
    const options = createOptions({ runUsage });
    vi.mocked(commitApplyEdits).mockResolvedValueOnce({ status: 'snapshotReady', snapshotId: 11 } as any);

    commitEditorTabTextChange(options);

    expect(runUsage).toHaveBeenCalledWith(options.nextText, expect.any(Function));
    await vi.waitFor(() => expect(commitApplyEdits).toHaveBeenCalledTimes(1));
  });
});
