import type { BuilderConfig, DocumentJobSettings, DocumentTextEdit, EventBatch, JobTerminal, SnapshotId } from '@core-wasm/index';
import {
  collectGraphDocumentJobResult,
  startSharedGraphDocumentJob,
} from '../graph-stream/document-job-runner';
import type { DocumentAnalysisResult } from '../../shared/worker-protocol/protocol';

export type DocumentCommitRequest = {
  documentKey: string;
  language: string;
  text: string;
  edits: DocumentTextEdit[];
  baseSnapshotId?: SnapshotId | null;
  revision: number;
  settings: DocumentJobSettings;
  builderConfig: BuilderConfig;
};

export type DocumentCommitResult = {
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


export async function commitDocument(request: DocumentCommitRequest): Promise<DocumentCommitResult> {
  if (request.baseSnapshotId == null) {
    const batch = rejectedBatch(
      'missing_base_snapshot',
      'ApplyEdits requires an authoritative base snapshot',
    );
    return { snapshotId: null, analysis: null, sourceText: null, jobHandle: 0, batch };
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
  return { snapshotId: result.snapshotId, analysis: result.analysis, sourceText: result.sourceText, jobHandle: started.jobHandle, batch: result.batch };
}
