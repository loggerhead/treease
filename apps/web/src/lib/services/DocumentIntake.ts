import type { BuilderConfig, DocumentJobSettings, SnapshotId } from '@core-wasm/index';
import type { DocumentAnalysisResult } from '../../shared/worker-protocol/protocol';
import { runTextDocumentJobForGraph } from '../graph-stream/document-job-runner';
import type { DocumentJobResultStatus } from '../../shared/document-job-result';

export type IntakeResultStatus = 'completed' | 'failed';

export type IntakeResult = {
  status: IntakeResultStatus;
  resultStatus: DocumentJobResultStatus | 'cancelled' | 'jobFailed';
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
};

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

  if (isFresh && !isFresh()) {
    return {
      status: 'failed',
      resultStatus: 'cancelled',
      documentKey,
      revision,
      snapshotId: null,
      analysis: null,
      sourceText: null,
      error: 'cancelled: operation is no longer fresh',
    };
  }

  let result;
  try {
    result = await runTextDocumentJobForGraph({
      documentKey,
      language,
      text,
      settings,
      builderConfig,
    });
  } catch (error) {
    return {
      status: 'failed',
      resultStatus: 'jobFailed',
      documentKey,
      revision,
      snapshotId: null,
      analysis: null,
      sourceText: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }

  if (isFresh && !isFresh()) {
    return {
      status: 'failed',
      resultStatus: 'cancelled',
      documentKey,
      revision,
      snapshotId: null,
      analysis: null,
      sourceText: null,
      error: 'cancelled: result is stale',
    };
  }

  if (result.status !== 'snapshotReady') {
    return {
      status: 'failed',
      resultStatus: result.status,
      documentKey,
      revision,
      snapshotId: null,
      analysis: result.analysis,
      sourceText: result.sourceText,
      error: result.status === 'parseFailed' ? 'parse failed' : 'no snapshot produced',
    };
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
