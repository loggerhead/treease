// 职责：UI、Worker、test bridge 共享的 Worker 请求/响应与事件类型定义；不含 Worker 入口辅助函数
import type {
  BuildHoverSubgraphProjectionInput,
  CancelDocumentJobInput,
  DiffResult,
  DocumentTextEdit,
  PathSeg,
  QuerySnapshotInput,
  SnapshotId,
  StartDocumentJobInput,
} from '@core-wasm/index';

// ── Document Protocol Types (Phase 3) ───────────────────────────────────────
// Rust-generated canonical document protocol types come from @core-wasm.

export type {
  BuildHoverSubgraphProjectionInput,
  CancelDocumentJobInput,
  DocumentAnchor,
  EventBatch,
  QueryKind,
  QueryResult,
  QuerySnapshotInput,
  QueryTargetKind,
  SnapshotId,
  SnapshotReadResult,
  StartDocumentJobInput,
} from '@core-wasm/index';

export type WasmErrorLike = {
  startLineNumber: number;
  startColumn: number;
  endLineNumber: number;
  endColumn: number;
  kind: number;
};

export type DocumentAnalysisResult = {
  documentKey: string;
  tree: unknown;
  value: unknown;
  diagnostics: WasmErrorLike[];
  semanticTokens: ArrayBuffer;
  semanticTokenVersion: number;
  sourceByteLength: number;
  language: string;
};

export type JsonBlockAtPositionResult = {
  found: boolean;
  text: string;
  startByte: number;
  endByte: number;
  startLineNumber: number;
  startColumn: number;
  endLineNumber: number;
  endColumn: number;
};

export type ReplaceReason =
  | 'no-base'
  | 'stale-base-revision'
  | 'multi-batch-not-supported'
  | 'invalid-edit'
  | 'incremental-entry-missing'
  | 'incremental-path-state-unsafe'
  | 'incremental-apply-failed'
  | 'parse-and-store-failed'
  | 'build-graph-delta-failed'
  | 'graph-edit-not-single-range'
  | 'snapshotNotReady'
  | 'missingAnalysis'
  | 'missingDocument'
  | 'invalidPath'
  | 'invalidReplacement'
  | 'unsupportedLanguage'
  | 'unsupportedEdit'
  | 'unsafeEdit';


export type WorkerRequest =
  | { id: number; type: 'init'; wasmURL: string; wasmBytes?: ArrayBuffer }
  | { id: number; type: 'dispose' }
  | { id: number; type: 'diagnostics'; documentKey: string; language: string; text: string }
  | {
      id: number;
      type: 'parseAndStore';
      documentKey: string;
      language: string;
      text?: string;
      textBytes?: ArrayBuffer | SharedArrayBuffer;
      nest: boolean;
    }
  | {
      id: number;
      type: 'findJsonBlockAtPosition';
      documentKey: string;
      language: string;
      text: string;
      row: number;
      column: number;
    }
  | {
      id: number;
      type: 'treePath';
      documentKey: string;
      language: string;
      row: number;
      column: number;
      snapshotId: SnapshotId | null;
      text?: string;
      nest: boolean;
    }
  | {
      id: number;
      type: 'pathSpan';
      documentKey: string;
      language: string;
      text: string;
      path: PathSeg[];
      target?: GraphSearchTarget;
      snapshotId: SnapshotId | null;
      nest: boolean;
    }
  | { id: number; type: 'graphSearch'; documentKey: string; snapshotId: SnapshotId | null; language: string; text: string; query: string; nest: boolean }
  | {
      id: number;
      type: 'format';
      language: string;
      text?: string;
      textBytes?: ArrayBuffer | SharedArrayBuffer;
      options?: any;
      nest?: boolean;
    }
  | { id: number; type: 'minify'; language: string; text: string; options?: any; nest?: boolean }
  | { id: number; type: 'sort'; language: string; text: string; options?: any; nest?: boolean }
  | { id: number; type: 'convert'; sourceLanguage: string; targetFormat: string; text: string; options?: any }
  | { id: number; type: 'runYq'; language: string; text: string; expression: string; options?: any; nest?: boolean }
  | { id: number; type: 'semanticTokensLegend' }
  | { id: number; type: 'guessLanguage'; text: string }
  | { id: number; type: 'parseToTree'; language: string; text: string; nest: boolean }
  | { id: number; type: 'parseValueToTree'; language: string; text: string; nest: boolean }
  | { id: number; type: 'parseValueToData'; language: string; text: string; nest: boolean }
  | {
      id: number;
      type: 'parseValueForPath';
      language: string;
      documentKey: string;
      text: string;
      path: PathSeg[];
      rawEdit: string;
      preferKey: boolean;
      nest: boolean;
    }
  | { id: number; type: 'valueToTreeNode'; value: unknown }
  | {
      id: number;
      type: 'applyValueEdit';
      language: string;
      text: string;
      path: PathSeg[];
      preferKey: boolean;
      value: any;
    }
  | {
      id: number;
      type: 'applyValueEditCanonical';
      language: string;
      text: string;
      path: PathSeg[];
      preferKey: boolean;
      value: any;
      nest: boolean;
    }
  | {
      id: number;
      type: 'planGraphValueEdit';
      documentKey?: string;
      snapshotId: SnapshotId | null;
      language: string;
      text: string;
      path: PathSeg[];
      preferKey: boolean;
      value: any;
      nest: boolean;
    }
  | {
      id: number;
      type: 'compare';
      language: string;
      leftLanguage?: string;
      rightLanguage?: string;
      left: string;
      right: string;
    }
  // ── Phase 3: Document job API ──
  | ({ id: number; type: 'startDocumentJob' } & StartDocumentJobInput)
  | ({ id: number; type: 'cancelDocumentJob' } & CancelDocumentJobInput)
  | ({ id: number; type: 'querySnapshot' } & QuerySnapshotInput)
  | ({ id: number; type: 'buildHoverSubgraphProjection' } & BuildHoverSubgraphProjectionInput)
  | ({ id: number; type: 'advanceDocumentJob'; jobHandle: number; kind: 'textChunk' | 'close' | 'poll' | 'binaryChunk'; text?: string; data?: Uint8Array });

export type WorkerResponse =
  | { id: number; ok: true; data?: any }
  | { id: number; ok: false; error: string };

export type PlanGraphValueEditResponse =
  | {
      mode: 'edits';
      edits: DocumentTextEdit[];
      text: string;
      tree: unknown;
      value: unknown;
    }
  | {
      mode: 'replace';
      reason: ReplaceReason;
      text: string;
      tree: unknown;
      value: unknown;
    };

export type CompareResponse = {
  mode: 'tree' | 'text';
  equal: boolean;
  result: DiffResult;
};

export type GraphSearchTarget = 'node' | 'key' | 'value';

export type BuildGraphDeltaEvent = {
  event: 'graphDelta';
  documentKey: string;
  streamRunId: string;
  eventSeq: number;
  delta: NormalizedGraphDelta;
  final: boolean;
};

export type StreamProgressPhase = 'start' | 'streaming' | 'flushing' | 'finishing' | 'done' | 'failed';

export type BuildGraphProgressEvent = {
  event: 'graphProgress';
  documentKey: string;
  streamRunId: string;
  eventSeq: number;
  phase: StreamProgressPhase;
  processedBytes: number;
  totalBytes: number;
  value: number;
  final: boolean;
};

export type GraphStreamDeltaEvent = {
  event: 'graphStreamDelta';
  sessionId: string;
  streamKey: string;
  documentKey?: string;
  streamRunId: string;
  eventSeq: number;
  inputByteLength: number;
  delta: NormalizedGraphDelta;
  final: boolean;
};

export type GraphStreamDeltaTransferEvent = {
  event: 'graphStreamDelta';
  sessionId: string;
  streamKey: string;
  documentKey?: string;
  streamRunId: string;
  eventSeq: number;
  inputByteLength: number;
  deltaBytes: ArrayBuffer;
  final: boolean;
};

export type GraphStreamProgressEvent = {
  event: 'graphStreamProgress';
  sessionId: string;
  streamKey: string;
  streamRunId: string;
  eventSeq: number;
  phase: StreamProgressPhase;
  processedBytes: number;
  totalBytes: number;
  value: number;
  final: boolean;
};

export type RawStreamGraphDeltaChunk = {
  payload: Uint8Array;
};

export type StreamGraphDeltaResult = {
  status: number;
  hasChunk?: number;
  hasMore?: number;
  deltaSeq?: number;
  chunkSeq?: number;
  finalChunkOfDelta?: number;
  chunk?: RawStreamGraphDeltaChunk;
};

export type WorkerStreamGraphDeltaResult = Omit<StreamGraphDeltaResult, 'chunk'> & {
  chunk?: RawGraphDelta;
  payload?: Uint8Array;
};

 export type RawGraphDelta = {
   clear?: number;
   nodesAdded: unknown[];
   nodesUpdated: unknown[];
   nodesRemoved: number[];
   edgesAdded: unknown[];
   edgesRemoved: unknown[];
   tableCellPatches?: RawTableCellPatch[];
  tablePatches?: unknown[];
  layoutPatches?: unknown[];
 };

export type RawTableCellPatch = {
  tableRenderHandle: number;
  rowIndex: number;
  columnIndex: number;
  cell: unknown;
};

 export type NormalizedGraphDelta = {
   normalized: true;
   clear: number;
   nodesAdded: GraphNode[];
   nodesUpdated: GraphNode[];
   nodesRemoved: number[];
   edgesAdded: GraphEdge[];
   edgesRemoved: GraphEdge[];
   tableCellPatches: NormalizedTableCellPatch[];
  tablePatches?: unknown[];
  layoutPatches?: unknown[];
 };

export type NormalizedTableCellPatch = {
  tableRenderHandle: number;
  rowIndex: number;
  columnIndex: number;
  cell: unknown;
};

export type GraphNode = {
  renderHandle: number;
  key: {
    kind: string;
    path: PathSeg[];
    pathKey: string;
    stableId: string;
  };
  kind: string;
  depth: number;
  boxArgs: { x: number; y: number; width: number; height: number; cornerRadius: number };
  path: PathSeg[];
  meta: unknown;
  rows: unknown[];
  table?: unknown;
};

export type GraphEdge = {
  fromRenderHandle: number;
  from: GraphNode['key'];
  fromRow: number;
  toRenderHandle: number;
  to: GraphNode['key'];
  toRow: number;
  bezierArgs: {
    fromX: number;
    fromY: number;
    c1x: number;
    c1y: number;
    c2x: number;
    c2y: number;
    toX: number;
    toY: number;
  };
};

export type PathRequestData = Extract<WorkerRequest, { type: 'treePath' | 'pathSpan' }>;
