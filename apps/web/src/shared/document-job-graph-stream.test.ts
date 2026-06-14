import { describe, it, expect, vi } from 'vitest';
import type { DocumentProjectionDelta, EventBatch } from '@core-wasm/index';
import { processGraphBatchEvents, projectionToRawGraphDelta } from './document-job-graph-stream';

describe('projectionToRawGraphDelta', () => {
  it('returns null for null input', () => {
    expect(projectionToRawGraphDelta(null)).toBeNull();
  });

  it('returns null for empty projection without clear', () => {
    const projection = {} as unknown as DocumentProjectionDelta;
    expect(projectionToRawGraphDelta(projection)).toBeNull();
  });

  it('returns a clear-only delta when clear is true with no graphData', () => {
    const projection = { clear: true } as unknown as DocumentProjectionDelta;
    const result = projectionToRawGraphDelta(projection);
    expect(result).not.toBeNull();
    expect(result!.clear).toBe(1);
    expect(result!.nodesAdded).toEqual([]);
    expect(result!.nodesRemoved).toEqual([]);
  });

  it('converts graphData and clear flag', () => {
    const nodesAdded = [{ id: 1 }];
    const projection = {
      clear: false,
      graphData: {
        nodesAdded,
        nodesUpdated: [],
        nodesRemoved: [],
        edgesAdded: [],
        edgesRemoved: [],
      },
    } as unknown as DocumentProjectionDelta;
    const result = projectionToRawGraphDelta(projection);
    expect(result).not.toBeNull();
    expect(result!.clear).toBe(0);
    expect(result!.nodesAdded).toBe(nodesAdded);
  });
});

describe('processGraphBatchEvents', () => {
  const totalBytes = 1000;

  it('calls onProgress for progress events', async () => {
    const onProgress = vi.fn();
    const batch: EventBatch = {
      requestSeq: 0,
      events: [
        { type: 'progress', processedBytes: 500 },
      ],
      terminal: null,
    };

    await processGraphBatchEvents(batch, totalBytes, { onProgress });
    expect(onProgress).toHaveBeenCalledWith(500, totalBytes);
  });

  it('clamps progress to totalBytes', async () => {
    const onProgress = vi.fn();
    const batch: EventBatch = {
      requestSeq: 0,
      events: [
        { type: 'progress', processedBytes: 999999 },
      ],
      terminal: null,
    };

    await processGraphBatchEvents(batch, totalBytes, { onProgress });
    expect(onProgress).toHaveBeenCalledWith(totalBytes, totalBytes);
  });

  it('calls onProjection for projectionDelta events', async () => {
    const onProjection = vi.fn();
    const batch: EventBatch = {
      requestSeq: 0,
      events: [
        { type: 'projectionDelta' as const, clear: false, graphData: { nodesAdded: [{} as any], nodesUpdated: [], nodesRemoved: [], edgesAdded: [], edgesRemoved: [] } },
      ],
      terminal: null,
    };

     await processGraphBatchEvents(batch, totalBytes, { onProjection });
     expect(onProjection).toHaveBeenCalledTimes(1);
     expect(onProjection.mock.calls[0][0].clear).toBe(0);
    // Verify version info is passed through
    expect(onProjection.mock.calls[0][1]).toEqual({
        patchSeq: 0,
        baseGraphVersion: 0,
        graphVersion: 0,
    });
   });

  it('ignores analysisDelta events', async () => {
    const onProgress = vi.fn();
    const onProjection = vi.fn();
    const batch: EventBatch = {
      requestSeq: 0,
      events: [
        { type: 'analysisDelta' as const, analysis: { tree: null, valueJson: null, diagnostics: [], semanticTokens: { data: [], version: 0 }, sourceByteLength: 0, language: 'json' } },
      ],
      terminal: null,
    };

    await processGraphBatchEvents(batch, totalBytes, { onProgress, onProjection });
    expect(onProgress).not.toHaveBeenCalled();
    expect(onProjection).not.toHaveBeenCalled();
  });

  it('handles mixed events in one batch', async () => {
    const onProgress = vi.fn();
    const onProjection = vi.fn();
    const batch: EventBatch = {
      requestSeq: 0,
      events: [
        { type: 'progress', processedBytes: 100 },
        { type: 'projectionDelta' as const, clear: false, graphData: { nodesAdded: [{} as any], nodesUpdated: [], nodesRemoved: [], edgesAdded: [], edgesRemoved: [] } },
        { type: 'analysisDelta' as const, analysis: { tree: null, valueJson: null, diagnostics: [], semanticTokens: { data: [], version: 0 }, sourceByteLength: 0, language: 'json' } },
        { type: 'progress', processedBytes: 200 },
      ],
      terminal: null,
    };

    await processGraphBatchEvents(batch, totalBytes, { onProgress, onProjection });
    expect(onProgress).toHaveBeenCalledTimes(2);
    expect(onProjection).toHaveBeenCalledTimes(1);
  });
});
