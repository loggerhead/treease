import type { BuilderConfig, DocumentJobSettings, DocumentTextEdit, EventBatch, JobTerminal, SnapshotId } from '@core-wasm/index';
import {
  collectGraphDocumentJobResult,
  startSharedGraphDocumentJob,
} from '../graph-stream/document-job-runner';
import type { DocumentAnalysisResult } from '../../shared/worker-protocol/protocol';
import type { DocumentJobResultStatus } from '../../shared/document-job-result';

export type ApplyEditsCommitRequest = {
  documentKey: string;
  language: string;
  edits: DocumentTextEdit[];
  baseSnapshotId?: SnapshotId | null;
  revision: number;
  settings: DocumentJobSettings;
  builderConfig: BuilderConfig;
};

export type DocumentCommitResult = {
  status: DocumentJobResultStatus | 'rejected';
  snapshotId: SnapshotId | null;
  analysis: DocumentAnalysisResult | null;
  sourceText: string | null;
  jobHandle: number;
  batch: EventBatch;
};
function rejectedBatch(code: string, detail: string): EventBatch {
  const terminal: JobTerminal = { type: 'rejected', code, detail };
  return { requestSeq: 0, events: [], terminal };
}

export async function commitApplyEdits(request: ApplyEditsCommitRequest): Promise<DocumentCommitResult> {
  if (request.baseSnapshotId == null) {
    const batch = rejectedBatch(
      'missing_base_snapshot',
      'ApplyEdits requires an authoritative base snapshot',
    );
    return { status: 'rejected', snapshotId: null, analysis: null, sourceText: null, jobHandle: 0, batch };
  }

  const { started, advance } = await startSharedGraphDocumentJob({
    documentKey: request.documentKey,
    language: request.language,
    settings: request.settings,
    outputAnalysis: true,
    outputGraph: true,
    builderConfig: request.builderConfig,
    baseSnapshotId: request.baseSnapshotId,
    edits: request.edits,
  });
  const closeBatch = await advance({
    jobHandle: started.jobHandle,
    kind: 'close',
  });
  const result = collectGraphDocumentJobResult({
    documentKey: request.documentKey,
    language: request.language,
    jobHandle: started.jobHandle,
    batches: [started.batch, closeBatch],
  });
  return {
    status: result.status,
    snapshotId: result.snapshotId,
    analysis: result.analysis,
    sourceText: result.sourceText,
    jobHandle: started.jobHandle,
    batch: result.batch,
  };
}
