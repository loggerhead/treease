import { describe, it, expect } from 'vitest';
import type { EventBatch, DocumentJobAnalysisPayload, SnapshotId } from '@core-wasm/index';
import { collectDocumentJobResult, normalizeDocumentJobAnalysisPayload } from './document-job-result';

function snapshotReady(snapshotId: SnapshotId, analysis: DocumentJobAnalysisPayload | null, sourceText: string | null = null) {
  return { type: 'snapshotReady' as const, snapshotId, analysis, mainGraph: null, sourceText };
}

function parseFailed(snapshotId: SnapshotId, analysis: DocumentJobAnalysisPayload | null) {
  return { type: 'parseFailed' as const, snapshotId, analysis };
}

function analysisPayload(
  overrides: Partial<DocumentJobAnalysisPayload> & { value?: unknown } = {},
): DocumentJobAnalysisPayload {
  const hasValue = Object.prototype.hasOwnProperty.call(overrides, 'value');
  const valueJson =
    overrides.valueJson ?? (hasValue ? JSON.stringify((overrides as { value?: unknown }).value) : null);
  const { value: _value, ...rest } = overrides as Partial<DocumentJobAnalysisPayload> & { value?: unknown };
  return {
    tree: null,
    valueJson,
    diagnostics: [],
    semanticTokens: { data: [], version: 0 },
    sourceByteLength: 0,
    language: 'json',
    ...rest,
  };
}

function batch(events: any[], terminal?: any): EventBatch {
  return { requestSeq: 0, events: events as any[], terminal: terminal ?? null };
}

describe('collectDocumentJobResult', () => {

describe('normalizeDocumentJobAnalysisPayload', () => {
  it('normalizes semantic tokens and fallback language', () => {
    const result = normalizeDocumentJobAnalysisPayload(
      'doc-1',
      'yaml',
      analysisPayload({
        language: '',
        semanticTokens: { data: [0, 1, 2, 3], version: 2 },
        sourceByteLength: 42,
      }),
    );

    expect(result).toEqual({
      documentKey: 'doc-1',
      tree: null,
      value: null,
      diagnostics: [],
      semanticTokens: expect.any(ArrayBuffer),
      semanticTokenVersion: 2,
      sourceByteLength: 42,
      language: 'yaml',
    });
    expect(Array.from(new Uint32Array(result!.semanticTokens))).toEqual([0, 1, 2, 3]);
  });
  it('ignores full tree and valueJson payloads', () => {
    const result = normalizeDocumentJobAnalysisPayload(
      'doc-2',
      'json',
      analysisPayload({
        tree: { id: 'tree' } as any,
        value: { name: 'Alice' },
      }),
    );

    expect(result?.tree).toBeNull();
    expect(result?.value).toBeNull();
  });
});

  it('returns sourceText from snapshotReady events', () => {
    const result = collectDocumentJobResult(
      batch([snapshotReady(7 as SnapshotId, null, '{\\n  "a": 1\\n}\\n')]),
    );

    expect(result).toEqual({
      status: 'snapshotReady',
      snapshotId: 7,
      analysis: null,
      sourceText: '{\\n  "a": 1\\n}\\n',
    });
  });
  it('returns snapshotReady when SnapshotReady event is present', () => {
    const result = collectDocumentJobResult(
      batch([snapshotReady(1, null)]),
    );
    expect(result.status).toBe('snapshotReady');
    expect(result.snapshotId).toBe(1);
  });

  it('returns parseFailed when only ParseFailed event is present', () => {
    const result = collectDocumentJobResult(
      batch([parseFailed(2, null)]),
    );
    expect(result.status).toBe('parseFailed');
    expect(result.snapshotId).toBe(2);
  });

  it('returns snapshotReady when both SnapshotReady and ParseFailed are present (uses last)', () => {
    const result = collectDocumentJobResult(
      batch([
        parseFailed(3, null),
        snapshotReady(4, null),
      ]),
    );
    expect(result.status).toBe('snapshotReady');
    expect(result.snapshotId).toBe(4);
  });

  it('returns noSnapshot when no terminal event exists', () => {
    const result = collectDocumentJobResult(
      batch([{ type: 'progress' as const, processedBytes: 0 }]),
    );
    expect(result.status).toBe('noSnapshot');
    expect(result.snapshotId).toBeNull();
  });

  it('returns noSnapshot when batch is empty', () => {
    const result = collectDocumentJobResult(batch([]));
    expect(result.status).toBe('noSnapshot');
    expect(result.snapshotId).toBeNull();
  });

  it('returns analysis from SnapshotReady', () => {
    const analysis = analysisPayload({ sourceByteLength: 42 });
    const result = collectDocumentJobResult(
      batch([snapshotReady(1, analysis)]),
    );
    expect(result.status).toBe('snapshotReady');
    expect(result.analysis).toBe(analysis);
  });

  it('returns analysis from ParseFailed', () => {
    const analysis = analysisPayload({
      diagnostics: [{ startLineNumber: 1, startColumn: 1, endLineNumber: 1, endColumn: 1, kind: 1 }],
    });
    const result = collectDocumentJobResult(
      batch([parseFailed(5, analysis)]),
    );
    expect(result.status).toBe('parseFailed');
    expect(result.analysis).toBe(analysis);
  });
});
