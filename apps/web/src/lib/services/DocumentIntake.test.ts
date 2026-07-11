import { beforeEach, describe, it, expect, vi } from 'vitest';
import type { SnapshotId } from '@core-wasm/index';
import type { DocumentAnalysisResult } from '../../shared/worker-protocol/protocol';

// Mock the dependency
vi.mock('../graph-stream/document-job-runner', () => ({
  runTextDocumentJobForGraph: vi.fn(),
}));

import { runIntakeJob } from './DocumentIntake';
import { runTextDocumentJobForGraph } from '../graph-stream/document-job-runner';
const mockSnapshotId = 1 as SnapshotId;
const mockAnalysis: DocumentAnalysisResult = {
  documentKey: 'test',
  tree: null,
  value: null,
  diagnostics: [],
  semanticTokens: new ArrayBuffer(0),
  semanticTokenVersion: 0,
  sourceByteLength: 0,
  language: 'json',
};
const documentJobSettings = {
  parser: { enableNest: false, nestMaxDepth: 8 },
  formatting: {
    indent: 2,
    smart: true,
    formatSourceOnClose: true,
    maxLineLength: 100,
    maxInlineComplexity: 1,
    maxArrayInlineItems: 6,
    alignObjectArrays: true,
  },
};


describe('runIntakeJob', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('returns completed when job succeeds', async () => {
    vi.mocked(runTextDocumentJobForGraph).mockResolvedValue({
      status: 'snapshotReady',
      batch: { requestSeq: 0, events: [], terminal: null },
      jobHandle: 1,
      snapshotId: mockSnapshotId,
      analysis: mockAnalysis,
      sourceText: null,
    });

    const result = await runIntakeJob({
      documentKey: 'test',
      language: 'json',
      text: '{"a":1}',
      settings: documentJobSettings,
      revision: 0,
    });

    expect(result.status).toBe('completed');
    expect(result.snapshotId).toBe(mockSnapshotId);
    expect(result.error).toBeUndefined();
  });

  it('returns failed when no snapshot is produced', async () => {
    vi.mocked(runTextDocumentJobForGraph).mockResolvedValue({
      status: 'noSnapshot',
      batch: { requestSeq: 0, events: [], terminal: null },
      jobHandle: 0,
      snapshotId: null,
      analysis: null,
      sourceText: null,
    });

    const result = await runIntakeJob({
      documentKey: 'test',
      language: 'json',
      text: 'invalid{',
      settings: documentJobSettings,
      revision: 0,
    });

    expect(result.status).toBe('failed');
    expect(result.snapshotId).toBeNull();
  });

  it('returns diagnostics-only when parseFailed carries diagnostics', async () => {
    vi.mocked(runTextDocumentJobForGraph).mockResolvedValue({
      status: 'parseFailed',
      batch: { requestSeq: 0, events: [], terminal: null },
      jobHandle: 0,
      snapshotId: mockSnapshotId,
      analysis: mockAnalysis,
      sourceText: null,
    });

    const result = await runIntakeJob({
      documentKey: 'test',
      language: 'json',
      text: 'invalid{',
      settings: documentJobSettings,
      revision: 0,
    });

    expect(result.status).toBe('diagnosticsOnly');
    expect(result.resultStatus).toBe('parseFailed');
    expect(result.snapshotId).toBe(mockSnapshotId);
    expect(result.analysis).toBe(mockAnalysis);
    expect(result.error).toBeUndefined();
  });

  it('cancels early when isFresh returns false before job', async () => {
    const result = await runIntakeJob({
      documentKey: 'test',
      language: 'json',
      text: '{"a":1}',
      settings: documentJobSettings,
      revision: 0,
      isFresh: () => false,
    });

    expect(result.status).toBe('failed');
    expect(result.error).toContain('cancelled');
    expect(runTextDocumentJobForGraph).not.toHaveBeenCalled();
  });

  it('passes correct parameters to runTextDocumentJobForGraph', async () => {
    vi.mocked(runTextDocumentJobForGraph).mockResolvedValue({
      status: 'snapshotReady',
      batch: { requestSeq: 0, events: [], terminal: null },
      jobHandle: 1,
      snapshotId: mockSnapshotId,
      analysis: null,
      sourceText: null,
    });

    await runIntakeJob({
      documentKey: 'doc-1',
      language: 'yaml',
      text: 'key: value',
      settings: { ...documentJobSettings, parser: { enableNest: true, nestMaxDepth: 8 } },
      revision: 0,
    });

    expect(runTextDocumentJobForGraph).toHaveBeenCalledWith({
      builderConfig: undefined,
      documentKey: 'doc-1',
      language: 'yaml',
      text: 'key: value',
      settings: { ...documentJobSettings, parser: { enableNest: true, nestMaxDepth: 8 } },
      outputAnalysis: true,
      outputGraph: true,
    });
  });
});
