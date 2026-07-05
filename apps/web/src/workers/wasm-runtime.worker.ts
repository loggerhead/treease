// 职责：WASM Worker 入口：init/dispose 生命周期、消息分发、runtime state 组装、freeStreams 管理
import {
  initWasm,
  TOKEN_TYPES,
} from '@core-wasm/index';
import { guessLanguage } from '@core-wasm/guess-language';
import {
  handleDiagnostics,
  handleParseAndStore,
  handleFindJsonBlockAtPosition,
  handleParseToTree,
  handleParseValueToTree,
  type DocumentParseRuntime,
} from './runtime/document-parse';
import {
  handleApplyValueEditCanonical,
  handlePlanGraphValueEdit,
  handleParseValueForPath,
  handleValueToTreeNode,
} from './runtime/document-value-edit';
import { handleCompare } from './runtime/document-compare';
import { handleConvert, handleFormat, handleMinify, handleRunYq, handleSort } from './runtime/document-transform';
import { describeError, postOk, postError } from './runtime/logging';
import { handleGraphSearch } from './runtime/graph-search';

import {
  createOkResponse,
  type WorkerContext,
  type WorkerRequest,
  type WorkerResponse,
} from './runtime/protocol';
import { clearWorkerRuntimeState, createWorkerRuntimeState } from './runtime/worker-runtime-state';
import { handlePathSpan, handleTreePath } from './runtime/tree-path';
import {
  handleStartDocumentJob,
  handleCancelDocumentJob,
  handleQuerySnapshot,
  handleBuildHoverSubgraphProjection,
  handleAdvanceDocumentJob,
} from './runtime/document-job';

const ctx = self as unknown as WorkerContext;

let initialized = false;
let initError: string | null = null;
let initInProgress: Promise<void> | null = null;
const wasmLoadFailureMessage = 'WASM 加载失败，请刷新页面';
let chunkSizeConfigCache: Record<string, any> | null = null;
const encoder = new TextEncoder();
const workerRuntimeState = createWorkerRuntimeState(encoder);
const {
  graphStateService,
  searchIndexByDocumentKey,
} = workerRuntimeState;

let messageQueue: Promise<void> = Promise.resolve();

function ensureReady(message: WorkerRequest): boolean {
  if (initError) {
    postError(ctx, message.id, `${wasmLoadFailureMessage}: ${initError}`);
    return false;
  }
  if (!initialized) {
    postError(ctx, message.id, 'WASM not initialized');
    return false;
  }
  return true;
}

function handleWorkerError(message: WorkerRequest, error: unknown): void {
  const describedError = describeError(error);
  console.error('[worker] message failed', {
    id: message.id,
    type: message.type,
    documentKey: 'documentKey' in message ? message.documentKey : undefined,
    language: 'language' in message ? message.language : undefined,
    row: 'row' in message ? message.row : undefined,
    column: 'column' in message ? message.column : undefined,
    error: describedError,
  });
  const response: WorkerResponse = {
    id: message.id,
    ok: false,
    error: error instanceof Error ? error.message : String(error),
  };
  ctx.postMessage(response);
}

async function handleInit(message: Extract<WorkerRequest, { type: 'init' }>): Promise<void> {
  if (initialized && !initError) {
    postOk(ctx, message.id, { chunkSizeConfig: chunkSizeConfigCache });
    return;
  }
  try {
    if (!initInProgress) {
      initInProgress = (async () => {
        await initWasm({ wasmURL: message.wasmURL, wasmBytes: message.wasmBytes });
        initialized = true;
        initError = null;
      })();
    }
    await initInProgress;
  } catch (error) {
    initialized = false;
    initError = error instanceof Error ? error.message : String(error);
    console.error('[wasm] init failed', initError);
    postError(ctx, message.id, `${wasmLoadFailureMessage}: ${initError}`);
    return;
  } finally {
    if (initInProgress && (initialized || initError)) {
      initInProgress = null;
    }
  }
  const mod = await import('@core-wasm/pkg');
  chunkSizeConfigCache = mod.get_chunk_size_config();
  const response: WorkerResponse = createOkResponse(message.id, { chunkSizeConfig: chunkSizeConfigCache });
  ctx.postMessage(response);
}

async function handleSemanticTokensLegend(
  message: Extract<WorkerRequest, { type: 'semanticTokensLegend' }>,
): Promise<void> {
  postOk(ctx, message.id, [...TOKEN_TYPES]);
}

async function handleGuessLanguage(message: Extract<WorkerRequest, { type: 'guessLanguage' }>): Promise<void> {
  const language = await guessLanguage(message.text);
  postOk(ctx, message.id, language);
}



const documentParseRuntime: DocumentParseRuntime = {
  ctx,
  encoder,
  searchIndexByDocumentKey,
};


async function handleDispose(message: Extract<WorkerRequest, { type: 'dispose' }>): Promise<void> {
  clearWorkerRuntimeState(workerRuntimeState);

  initialized = false;
  initError = null;
  postOk(ctx, message.id, true);
}

type WorkerRequestMap = {
  [K in WorkerRequest['type']]: Extract<WorkerRequest, { type: K }>;
};

function typedHandler<K extends WorkerRequest['type']>(
  handler: (msg: WorkerRequestMap[K]) => Promise<void>,
): (msg: WorkerRequest) => Promise<void> {
  return handler as (msg: WorkerRequest) => Promise<void>;
}

const messageHandlers: Record<string, (msg: WorkerRequest) => Promise<void>> = {
  init: typedHandler<'init'>(handleInit),
  guessLanguage: typedHandler<'guessLanguage'>(handleGuessLanguage),
  semanticTokensLegend: typedHandler<'semanticTokensLegend'>(handleSemanticTokensLegend),
  diagnostics: typedHandler<'diagnostics'>((msg) => handleDiagnostics(ctx, msg)),
  parseAndStore: typedHandler<'parseAndStore'>((msg) =>
    handleParseAndStore(documentParseRuntime, msg),
  ),
  findJsonBlockAtPosition: typedHandler<'findJsonBlockAtPosition'>((msg) =>
    handleFindJsonBlockAtPosition(ctx, msg),
  ),
  treePath: typedHandler<'treePath'>((msg) => handleTreePath(ctx, documentParseRuntime, msg)),
  pathSpan: typedHandler<'pathSpan'>((msg) => handlePathSpan(ctx, documentParseRuntime, msg)),
  graphSearch: typedHandler<'graphSearch'>((msg) =>
    handleGraphSearch(ctx, documentParseRuntime, graphStateService.graphStateByDocumentKey, msg),
  ),
  format: typedHandler<'format'>((msg) => handleFormat(ctx, msg)),
  minify: typedHandler<'minify'>((msg) => handleMinify(ctx, msg)),
  sort: typedHandler<'sort'>((msg) => handleSort(ctx, msg)),
  convert: typedHandler<'convert'>((msg) => handleConvert(ctx, msg)),
  runYq: typedHandler<'runYq'>((msg) => handleRunYq(ctx, msg)),
  parseToTree: typedHandler<'parseToTree'>((msg) => handleParseToTree(ctx, msg)),
  parseValueToTree: typedHandler<'parseValueToTree'>((msg) => handleParseValueToTree(ctx, msg)),
  parseValueForPath: typedHandler<'parseValueForPath'>((msg) => handleParseValueForPath(ctx, msg)),
  valueToTreeNode: typedHandler<'valueToTreeNode'>((msg) => handleValueToTreeNode(ctx, msg)),
  applyValueEditCanonical: typedHandler<'applyValueEditCanonical'>((msg) => handleApplyValueEditCanonical(ctx, msg)),
  planGraphValueEdit: typedHandler<'planGraphValueEdit'>((msg) => handlePlanGraphValueEdit(ctx, msg)),
  compare: typedHandler<'compare'>((msg) => handleCompare(ctx, msg)),
  dispose: typedHandler<'dispose'>(handleDispose),
  // ── Phase 3: Document job API ──
  startDocumentJob: typedHandler<'startDocumentJob'>((msg) => handleStartDocumentJob(ctx, msg)),
  cancelDocumentJob: typedHandler<'cancelDocumentJob'>((msg) => handleCancelDocumentJob(ctx, msg)),
  querySnapshot: typedHandler<'querySnapshot'>((msg) => handleQuerySnapshot(ctx, msg)),
  buildHoverSubgraphProjection: typedHandler<'buildHoverSubgraphProjection'>((msg) =>
    handleBuildHoverSubgraphProjection(ctx, msg),
  ),
  advanceDocumentJob: typedHandler<'advanceDocumentJob'>((msg) => handleAdvanceDocumentJob(ctx, msg)),
};

async function handleWorkerMessage(message: WorkerRequest): Promise<void> {
  try {
    if (message.type === 'init') {
      await handleInit(message);
      return;
    }
    if (!ensureReady(message)) {
      return;
    }
    const handler = messageHandlers[message.type];
    if (!handler) {
      console.warn('[worker] unhandled message', { id: message.id, type: message.type });
      postError(ctx, message.id, `Unhandled worker message type: ${message.type}`);
      return;
    }
    await handler(message);
  } catch (error) {
    handleWorkerError(message, error);
  }
}

ctx.onmessage = (event: MessageEvent<WorkerRequest>) => {
  const message = event.data;
  messageQueue = messageQueue
    .then(async () => {
      await handleWorkerMessage(message);
    })
    .catch((error) => {
      console.error('[worker] queue error', error);
    });
};
