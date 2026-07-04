import type {
  DocumentJobAnalysisPayload,
  EventBatch,
  SnapshotId,
} from '@core-wasm/index';
import type { DocumentAnalysisResult } from './worker-protocol/protocol';

export type DocumentJobResultStatus = 'snapshotReady' | 'parseFailed' | 'noSnapshot';

export type DocumentJobResult = {
  status: DocumentJobResultStatus;
  snapshotId: SnapshotId | null;
  analysis: DocumentJobAnalysisPayload | null;
  sourceText: string | null;
};

export function semanticTokensToBuffer(data: readonly number[] | undefined): ArrayBuffer {
  const tokens = data ?? [];
  const buffer = new ArrayBuffer(tokens.length * Uint32Array.BYTES_PER_ELEMENT);
  new Uint32Array(buffer).set(tokens);
  return buffer;
}


export function normalizeDocumentJobAnalysisPayload(
  documentKey: string,
  language: string,
  raw: DocumentJobAnalysisPayload | null | undefined,
): DocumentAnalysisResult | null {
  if (!raw) return null;
  return {
    documentKey,
    tree: null,
    value: null,
    diagnostics: raw.diagnostics ?? [],
    semanticTokens: semanticTokensToBuffer(raw.semanticTokens?.data),
    semanticTokenVersion: raw.semanticTokens?.version ?? 1,
    sourceByteLength: raw.sourceByteLength,
    language: raw.language || language,
  };
}

/**
 * Collect the final DocumentJob result from a merged EventBatch.
 *
 * Scans the batch events in reverse order to find the terminal
 * document event (SnapshotReady or ParseFailed) and returns a
 * structured result. Returns `noSnapshot` when no terminal event
 * is found.
 */
export function collectDocumentJobResult(batch: EventBatch): DocumentJobResult {
  for (let index = batch.events.length - 1; index >= 0; index -= 1) {
    const event = batch.events[index];
    if (event.type === 'snapshotReady') {
      return {
        status: 'snapshotReady',
        snapshotId: event.snapshotId as SnapshotId,
        analysis: event.analysis ?? null,
        sourceText: event.sourceText ?? null,
      };
    }
    if (event.type === 'parseFailed') {
      return {
        status: 'parseFailed',
        snapshotId: event.snapshotId as SnapshotId,
        analysis: event.analysis ?? null,
        sourceText: null,
      };
    }
  }
  return { status: 'noSnapshot', snapshotId: null, analysis: null, sourceText: null };
}
