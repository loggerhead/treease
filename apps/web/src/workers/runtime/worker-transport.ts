// Responsibility: own Worker transport lifecycle, readiness gating, serialized dispatch, request correlation, and ok/error responses.
import { TOKEN_TYPES, getSemanticTokens, initWasm } from '@core-wasm/index';
import { guessLanguage } from '@core-wasm/guess-language';

import {
  handleDiagnostics,
  handleFindJsonBlockAtPosition,
  handleParseAndStore,
  handleParseToTree,
  handleParseValueToTree,
  type DocumentParseRuntime,
} from './document-parse';
import {
  handleApplyValueEditCanonical,
  handleParseValueForPath,
  handlePlanGraphValueEdit,
  handleValueToTreeNode,
} from './document-value-edit';
import { handleCompare } from './document-compare';
import { handleCompact, handleConvert, handleFormat, handleMinify, handleRunYq, handleSort } from './document-transform';
import { describeError, postError, postOk } from './logging';
import { handleGraphSearch } from './graph-search';
import {
  handleAdvanceDocumentJob,
  handleBuildHoverSubgraphProjection,
  handleCancelDocumentJob,
  handleQuerySnapshot,
  handleStartDocumentJob,
} from './document-job';
import type { WorkerContext, WorkerRequest } from './protocol';
import { handlePathSpan, handleTreePath } from './tree-path';
import { clearWorkerRuntimeState, createWorkerRuntimeState } from './worker-runtime-state';

const wasmLoadFailureMessage = 'Failed to load WASM. Please refresh the page.';

type RuntimeRequest = Exclude<WorkerRequest, { type: 'init' | 'dispose' }>;

type WorkerOperationMap = {
  [K in RuntimeRequest['type']]: (message: Extract<RuntimeRequest, { type: K }>) => Promise<unknown>;
};

type WorkerTransport = {
  enqueue: (message: WorkerRequest) => void;
};

class TransportRequestError extends Error {
  constructor(message: string, readonly shouldLog: boolean) {
    super(message);
  }
}

export function createWorkerTransport(ctx: WorkerContext): WorkerTransport {
  let initialized = false;
  let initError: string | null = null;
  let initInProgress: Promise<void> | null = null;
  let chunkSizeConfigCache: Record<string, unknown> | null = null;
  let messageQueue: Promise<void> = Promise.resolve();

  const encoder = new TextEncoder();
  const workerRuntimeState = createWorkerRuntimeState(encoder);
  const documentParseRuntime: DocumentParseRuntime = {
    encoder,
    searchIndexByDocumentKey: workerRuntimeState.searchIndexByDocumentKey,
  };

  const operations = {
    guessLanguage: async (message) => guessLanguage(message.text),
    semanticTokensLegend: async () => [...TOKEN_TYPES],
    semanticTokens: async (message) => ({ semanticTokens: await getSemanticTokens(message.language, message.text) }),
    diagnostics: async (message) => handleDiagnostics(message),
    parseAndStore: async (message) => handleParseAndStore(documentParseRuntime, message),
    findJsonBlockAtPosition: async (message) => handleFindJsonBlockAtPosition(message),
    treePath: async (message) => handleTreePath(message),
    pathSpan: async (message) => handlePathSpan(message),
    graphSearch: async (message) =>
      handleGraphSearch(documentParseRuntime, workerRuntimeState.graphStateService.graphStateByDocumentKey, message),
    format: async (message) => handleFormat(message),
    minify: async (message) => handleMinify(message),
    compact: async (message) => handleCompact(message),
    sort: async (message) => handleSort(message),
    convert: async (message) => handleConvert(message),
    runYq: async (message) => handleRunYq(message),
    parseToTree: async (message) => handleParseToTree(message),
    parseValueToTree: async (message) => handleParseValueToTree(message),
    parseValueForPath: async (message) => handleParseValueForPath(message),
    valueToTreeNode: async (message) => handleValueToTreeNode(message),
    applyValueEditCanonical: async (message) => handleApplyValueEditCanonical(message),
    planGraphValueEdit: async (message) => handlePlanGraphValueEdit(message),
    compare: async (message) => handleCompare(message),
    startDocumentJob: async (message) => handleStartDocumentJob(message),
    cancelDocumentJob: async (message) => handleCancelDocumentJob(message),
    querySnapshot: async (message) => handleQuerySnapshot(message),
    buildHoverSubgraphProjection: async (message) => handleBuildHoverSubgraphProjection(message),
    advanceDocumentJob: async (message) => handleAdvanceDocumentJob(message),
  } satisfies WorkerOperationMap;

  async function initialize(message: Extract<WorkerRequest, { type: 'init' }>): Promise<{ chunkSizeConfig: Record<string, unknown> | null }> {
    if (initialized) return { chunkSizeConfig: chunkSizeConfigCache };

    if (!initInProgress) {
      initInProgress = initWasm({ wasmURL: message.wasmURL, wasmBytes: message.wasmBytes })
        .then(() => {
          initialized = true;
          initError = null;
        })
        .catch((error) => {
          initialized = false;
          initError = error instanceof Error ? error.message : String(error);
          console.error('[wasm] init failed', initError);
          throw new Error(`${wasmLoadFailureMessage}: ${initError}`);
        })
        .finally(() => {
          initInProgress = null;
        });
    }

    await initInProgress;
    const mod = await import('@core-wasm/pkg');
    chunkSizeConfigCache = mod.get_chunk_size_config();
    return { chunkSizeConfig: chunkSizeConfigCache };
  }

  function requireReady(): void {
    if (initError) throw new TransportRequestError(`${wasmLoadFailureMessage}: ${initError}`, false);
    if (!initialized) throw new TransportRequestError('WASM not initialized', false);
  }

  async function dispatch(message: WorkerRequest): Promise<unknown> {
    if (message.type === 'init') return initialize(message);
    if (message.type === 'dispose') {
      clearWorkerRuntimeState(workerRuntimeState);
      initialized = false;
      initError = null;
      chunkSizeConfigCache = null;
      return true;
    }

    requireReady();
    const operation = operations[message.type as RuntimeRequest['type']];
    if (!operation) throw new TransportRequestError(`Unhandled worker message type: ${message.type}`, false);
    return operation(message as never);
  }

  async function process(message: WorkerRequest): Promise<void> {
    try {
      postOk(ctx, message.id, await dispatch(message));
    } catch (error) {
      const describedError = describeError(error);
      if (!(error instanceof TransportRequestError) || error.shouldLog) {
        console.error('[worker] message failed', {
          id: message.id,
          type: message.type,
          documentKey: 'documentKey' in message ? message.documentKey : undefined,
          language: 'language' in message ? message.language : undefined,
          row: 'row' in message ? message.row : undefined,
          column: 'column' in message ? message.column : undefined,
          error: describedError,
        });
      }
      postError(ctx, message.id, describedError.message);
    }
  }

  return {
    enqueue(message: WorkerRequest): void {
      messageQueue = messageQueue.then(() => process(message));
    },
  };
}
