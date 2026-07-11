import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocked = vi.hoisted(() => ({
  commitApplyEdits: vi.fn(),
  runTextDocumentJobForGraph: vi.fn(),
}));

vi.mock('./DocumentCommitService', () => ({
  commitApplyEdits: mocked.commitApplyEdits,
}));

vi.mock('../graph-stream/document-job-runner', () => ({
  runTextDocumentJobForGraph: mocked.runTextDocumentJobForGraph,
}));

import { createFreshnessScope } from '../guards/freshness-scope';
import {
  getActiveDocumentSemanticState,
  getAuthorityWorkspaceSnapshotId,
  resetActiveDocumentAuthority,
} from '../store/active-document-authority';
import { runEditorCommitTransaction, type EditorCommitTransaction } from './EditorCommitTransaction';

function createTransaction(overrides: Partial<EditorCommitTransaction> = {}) {
  let current = true;
  const freshness = createFreshnessScope(
    { documentKey: 'doc', languageId: 'json', revision: 4, token: 1 },
    () => ({ documentKey: 'doc', languageId: 'json', revision: 4, token: current ? 1 : 2 }),
  );
  return {
    transaction: {
      documentKey: 'doc',
      language: 'json',
      revision: 4,
      settings: {} as any,
      builderConfig: {} as any,
      intent: { kind: 'applyEdits', edits: [], baseSnapshotId: 3 as any },
      freshness,
      ...overrides,
    } as EditorCommitTransaction,
    makeStale: () => {
      current = false;
    },
  };
}

describe('EditorCommitTransaction', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetActiveDocumentAuthority();
  });

  it('lands canonical source, DocumentSnapshot binding, then analysis in one order', async () => {
    const order: string[] = [];
    mocked.commitApplyEdits.mockResolvedValueOnce({
      status: 'snapshotReady',
      snapshotId: 9,
      analysis: { documentKey: 'doc', diagnostics: [], semanticTokens: new ArrayBuffer(0) },
      sourceText: '{\n  "a": 1\n}',
      jobHandle: 1,
      batch: { requestSeq: 1, events: [], terminal: { type: 'completed' } },
    });
    const { transaction } = createTransaction({
      landing: {
        writeSourceText: () => {
          order.push('source');
          expect(getAuthorityWorkspaceSnapshotId('doc')).toBeNull();
        },
        applyAnalysis: () => {
          order.push('analysis');
          expect(getAuthorityWorkspaceSnapshotId('doc')).toBe(9);
        },
      },
    });

    const result = await runEditorCommitTransaction(transaction);

    expect(result.status).toBe('snapshotReady');
    expect(order).toEqual(['source', 'analysis']);
    expect(getActiveDocumentSemanticState('doc')).toMatchObject({ status: 'valid', snapshotId: 9, revision: 4 });
  });

  it('preserves a parseFailed snapshot only as the next commit base', async () => {
    mocked.commitApplyEdits.mockResolvedValueOnce({
      status: 'parseFailed',
      snapshotId: 10,
      analysis: null,
      sourceText: null,
      jobHandle: 1,
      batch: { requestSeq: 1, events: [], terminal: { type: 'parseFailed' } },
    });
    const { transaction } = createTransaction();

    await runEditorCommitTransaction(transaction);

    expect(getActiveDocumentSemanticState('doc')).toMatchObject({
      status: 'invalidJsonBlockEligible',
      snapshotId: 10,
      revision: 4,
    });
    expect(getAuthorityWorkspaceSnapshotId('doc')).toBeNull();
  });

  it('rejects missing base snapshots without switching to AnalyzeSource', async () => {
    const { transaction } = createTransaction({
      intent: { kind: 'applyEdits', edits: [], baseSnapshotId: null },
    });
    mocked.commitApplyEdits.mockResolvedValueOnce({
      status: 'rejected',
      snapshotId: null,
      analysis: null,
      sourceText: null,
      jobHandle: 0,
      batch: { requestSeq: 0, events: [], terminal: { type: 'rejected' } },
    });

    const result = await runEditorCommitTransaction(transaction);

    expect(result.status).toBe('rejected');
    expect(mocked.runTextDocumentJobForGraph).not.toHaveBeenCalled();
    expect(getActiveDocumentSemanticState('doc')).toMatchObject({ status: 'rejected', revision: 4 });
  });

  it('classifies a terminal batch without a DocumentSnapshot as noSnapshot', async () => {
    mocked.commitApplyEdits.mockResolvedValueOnce({
      status: 'noSnapshot',
      snapshotId: null,
      analysis: null,
      sourceText: null,
      jobHandle: 1,
      batch: { requestSeq: 1, events: [], terminal: { type: 'completed' } },
    });
    const { transaction } = createTransaction();

    await expect(runEditorCommitTransaction(transaction)).resolves.toMatchObject({ status: 'noSnapshot' });
    expect(getActiveDocumentSemanticState('doc')).toMatchObject({ status: 'noSnapshot', revision: 4 });
    expect(getAuthorityWorkspaceSnapshotId('doc')).toBeNull();
  });

  it('drops stale results before source, semantic, snapshot, or analysis landing', async () => {
    const deferred = Promise.withResolvers<any>();
    mocked.commitApplyEdits.mockReturnValueOnce(deferred.promise);
    const writeSourceText = vi.fn();
    const applyAnalysis = vi.fn();
    const { transaction, makeStale } = createTransaction({ landing: { writeSourceText, applyAnalysis } });

    const pending = runEditorCommitTransaction(transaction);
    makeStale();
    deferred.resolve({
      status: 'snapshotReady',
      snapshotId: 9,
      analysis: { documentKey: 'doc', diagnostics: [], semanticTokens: new ArrayBuffer(0) },
      sourceText: '{}',
      jobHandle: 1,
      batch: { requestSeq: 1, events: [], terminal: { type: 'completed' } },
    });

    await expect(pending).resolves.toMatchObject({ status: 'cancelled' });
    expect(writeSourceText).not.toHaveBeenCalled();
    expect(applyAnalysis).not.toHaveBeenCalled();
    expect(getActiveDocumentSemanticState('doc')).toMatchObject({ status: 'pendingJsonBlockEligible', revision: 4 });
    expect(getAuthorityWorkspaceSnapshotId('doc')).toBeNull();
  });

  it('classifies thrown DocumentJob failures as a terminal jobFailed outcome', async () => {
    mocked.runTextDocumentJobForGraph.mockRejectedValueOnce(new Error('worker gone'));
    const { transaction } = createTransaction({ intent: { kind: 'analyzeSource', text: '{}' } });

    await expect(runEditorCommitTransaction(transaction)).resolves.toMatchObject({ status: 'jobFailed', error: 'worker gone' });
    expect(getActiveDocumentSemanticState('doc')).toMatchObject({ status: 'jobFailed', revision: 4 });
  });
});
