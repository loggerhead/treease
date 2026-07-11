import type { BuilderConfig, DocumentJobSettings, DocumentTextEdit, EventBatch, SnapshotId } from '@core-wasm/index';

import type { DocumentAnalysisResult } from '../../shared/worker-protocol/protocol';
import type { FreshnessScope } from '../guards/freshness-scope';
import { createViewRuntimeOperationFromFreshnessScope } from '../guards/view-runtime-operation';
import { runTextDocumentJobForGraph } from '../graph-stream/document-job-runner';
import { applyActiveDocumentJobOutcome, beginActiveDocumentJob } from '../store/active-document-authority';
import { commitApplyEdits } from './DocumentCommitService';

export type EditorCommitIntent =
  | { kind: 'analyzeSource'; text: string }
  | { kind: 'applyEdits'; edits: DocumentTextEdit[]; baseSnapshotId: SnapshotId | null };

export type EditorCommitResultStatus =
  | 'snapshotReady'
  | 'parseFailed'
  | 'rejected'
  | 'noSnapshot'
  | 'cancelled'
  | 'jobFailed';

export type EditorCommitResult = {
  status: EditorCommitResultStatus;
  documentKey: string;
  language: string;
  revision: number;
  snapshotId: SnapshotId | null;
  analysis: DocumentAnalysisResult | null;
  sourceText: string | null;
  batch: EventBatch | null;
  error?: string;
};

export type EditorCommitLanding = {
  writeSourceText?: (sourceText: string) => void;
  applyAnalysis?: (analysis: DocumentAnalysisResult) => Promise<void> | void;
};

export type EditorCommitTransaction = {
  documentKey: string;
  language: string;
  revision: number;
  settings: DocumentJobSettings;
  builderConfig?: BuilderConfig;
  intent: EditorCommitIntent;
  freshness: FreshnessScope;
  landing?: EditorCommitLanding;
};

export type EditorCommitStart = Pick<EditorCommitTransaction, 'documentKey' | 'language' | 'revision' | 'freshness'>;

function cancelled(transaction: EditorCommitTransaction): EditorCommitResult {
  return {
    status: 'cancelled',
    documentKey: transaction.documentKey,
    language: transaction.language,
    revision: transaction.revision,
    snapshotId: null,
    analysis: null,
    sourceText: null,
    batch: null,
  };
}

function failed(transaction: EditorCommitTransaction, error: unknown): EditorCommitResult {
  return {
    status: 'jobFailed',
    documentKey: transaction.documentKey,
    language: transaction.language,
    revision: transaction.revision,
    snapshotId: null,
    analysis: null,
    sourceText: null,
    batch: null,
    error: error instanceof Error ? error.message : String(error),
  };
}

function isAuthorityOutcome(status: EditorCommitResultStatus): status is Extract<EditorCommitResultStatus, 'snapshotReady' | 'parseFailed' | 'rejected' | 'noSnapshot' | 'jobFailed'> {
  return status !== 'cancelled';
}

/** Starts the Web-visible side of a Commit Transaction before a streamed job settles. */
export function beginEditorCommitTransaction(transaction: EditorCommitStart): boolean {
  return transaction.freshness.isCurrent() && beginActiveDocumentJob(transaction);
}

async function landEditorCommitTransaction(
  transaction: EditorCommitTransaction,
  result: EditorCommitResult,
): Promise<void> {
  if (result.status === 'cancelled') return;
  if (result.sourceText != null) transaction.landing?.writeSourceText?.(result.sourceText);

  if (isAuthorityOutcome(result.status)) {
    applyActiveDocumentJobOutcome({
      documentKey: result.documentKey,
      language: result.language,
      revision: result.revision,
      status: result.status,
      snapshotId: result.snapshotId,
    });
  }

  if (result.analysis != null) await transaction.landing?.applyAnalysis?.(result.analysis);
}

/** Lands an already completed DocumentJob, including readable stream jobs. */
export async function settleEditorCommitTransaction(
  transaction: EditorCommitTransaction,
  result: EditorCommitResult,
): Promise<EditorCommitResult> {
  const operation = createViewRuntimeOperationFromFreshnessScope(transaction.freshness);
  const outcome = await operation.run({
    execute: async () => result,
    land: (settled) => landEditorCommitTransaction(transaction, settled),
  });
  if (outcome.status === 'completed') return outcome.value;
  if (outcome.status === 'stale') return cancelled(transaction);
  return failed(transaction, outcome.error);
}

/**
 * Executes one editor Commit Transaction and lands its terminal result in a
 * fixed order: canonical source text, semantic state / DocumentSnapshot
 * binding, then analysis. Core remains authoritative for every terminal value.
 */
export async function runEditorCommitTransaction(
  transaction: EditorCommitTransaction,
): Promise<EditorCommitResult> {
  if (!beginEditorCommitTransaction(transaction)) return cancelled(transaction);

  let result: EditorCommitResult;
  try {
    if (transaction.intent.kind === 'applyEdits') {
      const committed = await commitApplyEdits({
        documentKey: transaction.documentKey,
        language: transaction.language,
        edits: transaction.intent.edits,
        baseSnapshotId: transaction.intent.baseSnapshotId,
        revision: transaction.revision,
        settings: transaction.settings,
        builderConfig: transaction.builderConfig as BuilderConfig,
      });
      result = {
        ...committed,
        documentKey: transaction.documentKey,
        language: transaction.language,
        revision: transaction.revision,
      };
    } else {
      const analyzed = await runTextDocumentJobForGraph({
        documentKey: transaction.documentKey,
        language: transaction.language,
        text: transaction.intent.text,
        settings: transaction.settings,
        builderConfig: transaction.builderConfig,
        outputAnalysis: true,
        outputGraph: true,
      });
      result = {
        ...analyzed,
        documentKey: transaction.documentKey,
        language: transaction.language,
        revision: transaction.revision,
      };
    }
  } catch (error) {
    result = failed(transaction, error);
  }

  return settleEditorCommitTransaction(transaction, result);
}
