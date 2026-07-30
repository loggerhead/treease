import type { BuilderConfig, DocumentTextEdit } from './types';
import type {
  DocumentJobSettings,
  EventBatch,
  GraphPathSeg,
  GraphValueEditFallbackReason,
  GraphValueEditPlan as GeneratedGraphValueEditPlan,
  OutputPlan,
  ProjectionDelta,
  QueryResult,
  SnapshotId,
  SnapshotQuery,
  SnapshotReadResult,
} from '../document-protocol.generated';
import { callWasm, getChunkSizeConfig, initWasm, type ChunkSizeConfig } from './shared-api';

type DocumentJobDocumentRef = { documentKey: string };
type DocumentJobLanguageRef = DocumentJobDocumentRef & { language: string };
type SnapshotQuerySpanValue = NonNullable<SnapshotQuery['span']>[number];

export type StartDocumentJobInput = DocumentJobLanguageRef & {
  settings: DocumentJobSettings;
  nest?: boolean;
  outputGraph: OutputPlan['graph'];
  outputAnalysis: OutputPlan['analysis'];
  builderConfig?: BuilderConfig;
  baseSnapshotId?: SnapshotId | null;
  edits?: DocumentTextEdit[];
};

export type CancelDocumentJobInput = {
  jobHandle: StartDocumentJobResult['jobHandle'];
};

export type QuerySnapshotInput = DocumentJobDocumentRef & {
  snapshotId: SnapshotQuery['snapshotId'];
  queryKind: SnapshotQuery['kind'];
  pathPattern?: NonNullable<SnapshotQuery['pathPattern']>;
  spanStart?: SnapshotQuerySpanValue;
  spanEnd?: SnapshotQuerySpanValue;
  target?: NonNullable<SnapshotQuery['target']>;
};

export type BuildHoverSubgraphProjectionInput = DocumentJobDocumentRef & {
  snapshotId: SnapshotId;
  path: string;
};

export type PlanGraphValueEditInput = DocumentJobLanguageRef & {
  snapshotId: SnapshotId;
  path: GraphPathSeg[];
  preferKey: boolean;
  value: unknown;
  rawReplacement?: string;
};

export type GraphValueEditPlan = {
  mode: GeneratedGraphValueEditPlan['mode'];
  edits: DocumentTextEdit[];
  reason?: GraphValueEditFallbackReason | null;
};

export type DocumentProjectionDelta = ProjectionDelta;

export type StartDocumentJobResult = {
  jobHandle: number;
  batch: EventBatch;
};

export type DocumentJobAnalysisPayload = NonNullable<
  Extract<EventBatch['events'][number], { type: 'snapshotReady' }>['analysis']
>;

const graphDeltaPayloadDecoder = new TextDecoder();

export function decodeGraphDeltaPayload(payload: Uint8Array): unknown {
  if (!(payload instanceof Uint8Array) || payload.byteLength === 0) return null;
  return JSON.parse(graphDeltaPayloadDecoder.decode(payload));
}

function normalizeDocumentTextEdit(edit: Record<string, unknown>): DocumentTextEdit {
  const text =
    (typeof edit.text === 'string' ? edit.text : null) ??
    (typeof edit.replacement === 'string' ? edit.replacement : null) ??
    '';
  return {
    startByte: Number(edit.startByte ?? 0),
    oldEndByte: Number(edit.oldEndByte ?? 0),
    newEndByte: Number(edit.newEndByte ?? 0),
    startRow: Number(edit.startRow ?? 0),
    startColumn: Number(edit.startColumn ?? 0),
    oldEndRow: Number(edit.oldEndRow ?? 0),
    oldEndColumn: Number(edit.oldEndColumn ?? 0),
    newEndRow: Number(edit.newEndRow ?? 0),
    newEndColumn: Number(edit.newEndColumn ?? 0),
    text,
  };
}

export { initWasm, getChunkSizeConfig };
export type { ChunkSizeConfig };

export async function startDocumentJob(input: StartDocumentJobInput): Promise<StartDocumentJobResult> {
  return callWasm((mod) =>
    mod.start_document_job({
      documentKey: input.documentKey,
      language: input.language,
      text: '',
      nest: input.nest ?? input.settings.parser.enableNest,
      settings: input.settings,
      outputGraph: input.outputGraph,
      outputAnalysis: input.outputAnalysis,
      builderConfig: input.builderConfig ?? null,
      baseSnapshotId: input.baseSnapshotId ?? null,
      edits: input.edits ?? [],
    } as any),
  );
}

export async function cancelDocumentJob(input: CancelDocumentJobInput): Promise<EventBatch> {
  return callWasm((mod) => mod.cancel_document_job(input as any));
}

export type AdvanceDocumentJobInput = {
  jobHandle: number;
  kind: 'textChunk' | 'close' | 'poll' | 'binaryChunk';
  text?: string;
  data?: Uint8Array;
};

export async function advanceDocumentJob(input: AdvanceDocumentJobInput): Promise<EventBatch> {
  return callWasm((mod) =>
    mod.advance_document_job({
      jobHandle: input.jobHandle,
      advanceKind: input.kind,
      text: input.text ?? null,
      data: input.data ?? null,
    } as any),
  );
}

export async function querySnapshot(input: QuerySnapshotInput): Promise<SnapshotReadResult<QueryResult>> {
  return callWasm((mod) =>
    mod.query_snapshot({
      documentKey: input.documentKey,
      snapshotId: input.snapshotId,
      queryKind: input.queryKind,
      pathPattern: input.pathPattern ?? null,
      spanStart: input.spanStart ?? null,
      spanEnd: input.spanEnd ?? null,
      target: input.target ?? null,
    } as any),
  );
}

export async function buildHoverSubgraphProjection(
  input: BuildHoverSubgraphProjectionInput,
): Promise<SnapshotReadResult<DocumentProjectionDelta>> {
  const result = await callWasm((mod) => mod.build_hover_subgraph_projection(input as any));
  if (result?.status !== 'ready') return { status: 'snapshotNotReady' };
  return {
    status: 'ready',
    data: {
      clear: result.data.clear,
      graphData: result.data.graphData ?? null,
    },
  };
}

export async function planGraphValueEdit(
  input: PlanGraphValueEditInput,
): Promise<SnapshotReadResult<GraphValueEditPlan>> {
  const result = await callWasm((mod) =>
    mod.plan_graph_value_edit({
      documentKey: input.documentKey,
      snapshotId: input.snapshotId,
      language: input.language,
      path: input.path,
      preferKey: input.preferKey,
      value: input.value,
      rawReplacement: input.rawReplacement ?? null,
    } as any),
  );
  if (result?.status !== 'ready') return { status: 'snapshotNotReady' };
  return {
    status: 'ready',
    data: {
      mode: result.data.mode,
      edits: Array.isArray(result.data.edits)
        ? result.data.edits.map((edit: Record<string, unknown>) => normalizeDocumentTextEdit(edit))
        : [],
      reason: result.data.reason ?? null,
    },
  };
}
