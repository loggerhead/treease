import type { BuilderConfig, DocumentJobSettings, SnapshotId } from '@core-wasm/index';
import type { DocumentAnalysisResult } from '../../shared/worker-protocol/protocol';
import type { DocumentJobResultStatus } from '../../shared/document-job-result';
import { createFreshnessScope } from '../guards/freshness-scope';
import { runEditorCommitTransaction, type EditorCommitLanding } from './EditorCommitTransaction';

export type IntakeResultStatus = 'completed' | 'diagnosticsOnly' | 'failed';

export type IntakeResult = {
  status: IntakeResultStatus;
  resultStatus: DocumentJobResultStatus | 'cancelled' | 'jobFailed' | 'rejected';
  documentKey: string;
  revision: number;
  snapshotId: SnapshotId | null;
  analysis: DocumentAnalysisResult | null;
  sourceText: string | null;
  error?: string;
};

export type RunIntakeJobParams = {
  documentKey: string;
  language: string;
  text: string;
  settings: DocumentJobSettings;
  revision: number;
  builderConfig?: BuilderConfig;
  isFresh?: () => boolean;
  landing?: EditorCommitLanding;
};

function createFailedIntakeResult(params: {
  documentKey: string;
  revision: number;
  resultStatus: IntakeResult['resultStatus'];
  analysis?: DocumentAnalysisResult | null;
  sourceText?: string | null;
  error: string;
}): IntakeResult {
  return {
    status: 'failed',
    resultStatus: params.resultStatus,
    documentKey: params.documentKey,
    revision: params.revision,
    snapshotId: null,
    analysis: params.analysis ?? null,
    sourceText: params.sourceText ?? null,
    error: params.error,
  };
}

function createDiagnosticsOnlyIntakeResult(params: {
  documentKey: string;
  revision: number;
  snapshotId?: SnapshotId | null;
  analysis?: DocumentAnalysisResult | null;
  sourceText?: string | null;
}): IntakeResult {
  return {
    status: 'diagnosticsOnly',
    resultStatus: 'parseFailed',
    documentKey: params.documentKey,
    revision: params.revision,
    snapshotId: params.snapshotId ?? null,
    analysis: params.analysis ?? null,
    sourceText: params.sourceText ?? null,
  };
}

/**
 * Run a complete intake lifecycle:
 *   start job → stream text → close → collect result
 *
 * Returns an IntakeResult for the caller to consume as UI effects
 * (snapshot binding, graph update, error display).
 *
 * Freshness: checks `isFresh` before starting the job and after
 * completion. If stale at either point, returns a cancelled result
 * and does not dispatch the job.
 */
export async function runIntakeJob(params: RunIntakeJobParams): Promise<IntakeResult> {
  const { documentKey, language, text, settings, builderConfig, revision, isFresh } = params;
  const freshness = createFreshnessScope(
    { documentKey, languageId: language, revision, token: revision },
    () => ({ documentKey, languageId: language, revision, token: isFresh?.() === false ? -1 : revision }),
  );
  const result = await runEditorCommitTransaction({
    documentKey,
    language,
    revision,
    settings,
    builderConfig,
    intent: { kind: 'analyzeSource', text },
    freshness,
    landing: params.landing,
  });

  if (result.status === 'cancelled') {
    return createFailedIntakeResult({
      documentKey,
      revision,
      resultStatus: 'cancelled',
      error: 'cancelled: operation is no longer fresh',
    });
  }
  if (result.status === 'jobFailed') {
    return createFailedIntakeResult({
      documentKey,
      revision,
      resultStatus: 'jobFailed',
      error: result.error ?? 'DocumentJob failed',
    });
  }
  if (result.status === 'parseFailed') {
    return createDiagnosticsOnlyIntakeResult({
      documentKey,
      revision,
      snapshotId: result.snapshotId,
      analysis: result.analysis,
      sourceText: result.sourceText,
    });
  }

  if (result.status !== 'snapshotReady') {
    return createFailedIntakeResult({
      documentKey,
      revision,
      resultStatus: result.status,
      analysis: result.analysis,
      sourceText: result.sourceText,
      error: 'no snapshot produced',
    });
  }

  return {
    status: 'completed',
    resultStatus: result.status,
    documentKey,
    revision,
    snapshotId: result.snapshotId,
    analysis: result.analysis,
    sourceText: result.sourceText,
  };
}
