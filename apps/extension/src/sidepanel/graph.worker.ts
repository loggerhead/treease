import wasmUrl from '@core-wasm/pkg/core.wasm?url';
import { advanceDocumentJob, initWasm, startDocumentJob } from '@core-wasm/index';
import { applyProjectionGraphDelta, createProjectionGraphState, projectionGraphSnapshot, projectionToRawGraphDelta } from '@treease-web/shared/graph-projection-state';
import type { GraphData, StructuredLanguage } from '../shared/types';

type Request = { id: number; text: string; language: StructuredLanguage };
type Response = { id: number; ok: true; data: GraphData } | { id: number; ok: false; error: string };

let wasmReady: Promise<void> | null = null;
const workerScope = globalThis as unknown as {
  onmessage: ((event: MessageEvent<Request>) => void) | null;
  postMessage: (message: Response) => void;
};

const documentJobSettings = {
  parser: { enableNest: false, nestMaxDepth: 8 },
  formatting: { indent: 2, smart: true, formatSourceOnClose: true, maxLineLength: 100, maxInlineComplexity: 1, maxArrayInlineItems: 6, alignObjectArrays: true },
};

async function build(text: string, language: StructuredLanguage): Promise<GraphData> {
  if (!wasmReady) wasmReady = initWasm({ wasmURL: wasmUrl });
  await wasmReady;
  const stream = createProjectionGraphState();
  const started = await startDocumentJob({
    documentKey: `extension-${crypto.randomUUID()}`,
    language,
    nest: false,
    settings: documentJobSettings,
    outputGraph: true,
    outputAnalysis: false,
  });
  const process = async (batch: Awaited<ReturnType<typeof advanceDocumentJob>>) => {
    for (const event of batch.events) {
      if (event.type !== 'projectionDelta') continue;
      const delta = projectionToRawGraphDelta({ clear: event.clear, graphData: event.graphData ?? null });
      if (delta) applyProjectionGraphDelta(stream, delta);
    }
    const parseFailure = batch.events.find((entry) => entry.type === 'parseFailed');
    if (parseFailure) throw new Error('Treease Core could not parse this document.');
  };
  await process(started.batch);
  await process(await advanceDocumentJob({ jobHandle: started.jobHandle, kind: 'textChunk', text }));
  await process(await advanceDocumentJob({ jobHandle: started.jobHandle, kind: 'close' }));
  const graph = projectionGraphSnapshot(stream);
  if (graph.nodes.length === 0) throw new Error('Treease Core produced no graph projection.');
  return { nodes: graph.nodes, edges: graph.edges, coreGraphAvailable: true } as GraphData;
}

workerScope.onmessage = (event: MessageEvent<Request>) => {
  void build(event.data.text, event.data.language)
    .then((data): Response => ({ id: event.data.id, ok: true, data }))
    .catch((error): Response => ({ id: event.data.id, ok: false, error: error instanceof Error ? error.message : String(error) }))
    .then((response) => workerScope.postMessage(response));
};
