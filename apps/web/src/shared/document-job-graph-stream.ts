import type { DocumentProjectionDelta, EventBatch } from '@core-wasm/index';
import type { RawGraphDelta } from './worker-protocol/protocol';
import { isRawGraphDelta } from './worker-protocol/graph-delta-normalize';
export { projectionToRawGraphDelta } from './graph-projection-state';

/**
 * Convert a DocumentProjectionDelta (from a streaming DocumentJob) to a
 * RawGraphDelta consumable by the Graph renderer.
 *
 * Handles the clear-only, graphData-only, and combined cases.
 */
function legacyProjectionToRawGraphDelta(
  projection: DocumentProjectionDelta | null,
): RawGraphDelta | null {
  const graphData = projection?.graphData ?? null;
  if (!graphData) {
    if (!projection?.clear) return null;
    return {
      clear: 1,
      nodesAdded: [],
      nodesUpdated: [],
      nodesRemoved: [],
      edgesAdded: [],
      edgesRemoved: [],
      tablePatches: [],
      layoutPatches: [],
    };
  }
  return {
    clear: projection.clear ? 1 : 0,
    nodesAdded: graphData.nodesAdded,
    nodesUpdated: graphData.nodesUpdated,
    nodesRemoved: graphData.nodesRemoved,
    edgesAdded: graphData.edgesAdded,
    edgesRemoved: graphData.edgesRemoved,
    tablePatches: graphData.tablePatches ?? [],
    layoutPatches: graphData.layoutPatches ?? [],
  };
}

export type GraphBatchEventHandler = {
    onProgress?: (processedBytes: number, totalBytes: number) => void;
    onProjection?: (delta: RawGraphDelta, version: {
        patchSeq: number;
        baseGraphVersion: number;
        graphVersion: number;
    }) => Promise<void>;
};
/**
 * Process a single EventBatch for graph rendering.
 *
 * Handles progress parsing, projection delta conversion, and skips
 * non-graph events (analysisDelta, snapshotReady, parseFailed).
 *
 * The totalBytes argument is used to clamp progress values.
 */
export async function processGraphBatchEvents(
    batch: EventBatch,
    totalBytes: number,
    handler: GraphBatchEventHandler,
): Promise<void> {
    for (const event of batch.events) {
        if (event.type === 'progress') {
            const clamped = Math.max(0, Math.min(totalBytes, event.processedBytes));
            handler.onProgress?.(clamped, totalBytes);
            continue;
        }
        if (event.type === 'analysisDelta') continue;
        if (event.type !== 'projectionDelta') continue;
        const delta = legacyProjectionToRawGraphDelta({
            clear: event.clear,
            graphData: event.graphData ?? null,
        });
        if (!delta) continue;
        if (!isRawGraphDelta(delta)) {
            throw new Error('graph projection decode failed');
        }
        await handler.onProjection?.(delta, {
            patchSeq: event.patchSeq ?? 0,
            baseGraphVersion: event.baseGraphVersion ?? 0,
            graphVersion: event.graphVersion ?? 0,
        });
    }
}
