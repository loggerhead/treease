import type * as Monaco from 'monaco-editor';
import { createDocumentColorRegistrar, type DocumentColorRegistrar } from './editor-color-provider';
import { resolveDocumentAnalysis } from '../services/DocumentAnalysisResolver';
import type { MonacoApi } from './public-types';

type CallWasmWorker = <T>(type: string, payload?: Record<string, any>, transfer?: Transferable[]) => Promise<T>;

type SemanticTokensRegistrarOptions = {
  monaco: MonacoApi;
  callWasmWorker: CallWasmWorker;
  tokenTypes: readonly string[];
  includeUri?: boolean;
  logRegister?: boolean;
  isImportActive?: () => boolean;
};

type DocumentColorRegistrarOptions = {
  monaco: MonacoApi;
};

const SEMANTIC_TOKENS_PRIME_WAIT_MS = 2_000;

function validateSemanticTokensData(
  data: Uint32Array,
  lineCount: number,
  getLineLength: (line: number) => number,
): boolean {
  if (data.length === 0 || data.length % 5 !== 0) return false;
  let currentLine = 0;
  let currentChar = 0;
  for (let i = 0; i < data.length; i += 5) {
    currentLine += data[i];
    if (data[i] === 0) {
      currentChar += data[i + 1];
    } else {
      currentChar = data[i + 1];
    }
    const lineNumber = currentLine + 1;
    if (lineNumber > lineCount) return false;
    const lineLength = getLineLength(lineNumber);
    const endChar = currentChar + data[i + 2];
    if (endChar > lineLength) return false;
  }
  return true;
}

export function ensureLanguageRegistered(
  monaco: MonacoApi,
  languageId: string,
) {
  const registered = monaco.languages.getLanguages().some((lang) => lang.id === languageId);
  if (!registered) {
    monaco.languages.register({ id: languageId });
  }
}

export function createSemanticTokensRegistrar(options: SemanticTokensRegistrarOptions) {
  const { monaco, tokenTypes } = options;
  const registeredLanguages = new Set<string>();
  const activeRefreshListeners = new Set<() => void>();
  const semanticTokensByDocumentKey = new Map<string, ArrayBuffer>();
  const pendingByDocumentKey = new Map<string, Set<(tokens: ArrayBuffer | null) => void>>();

  const settlePending = (documentKey: string, tokens: ArrayBuffer | null) => {
    const pending = pendingByDocumentKey.get(documentKey);
    if (!pending) return;
    pendingByDocumentKey.delete(documentKey);
    pending.forEach((resolve) => resolve(tokens ? tokens.slice(0) : null));
  };

  const settleAllPending = () => {
    pendingByDocumentKey.forEach((pending) => pending.forEach((resolve) => resolve(null)));
    pendingByDocumentKey.clear();
  };

  const waitForPrimedSemanticTokens = (
    documentKey: string,
    token: Monaco.CancellationToken,
  ): Promise<ArrayBuffer | null> => {
    const primed = semanticTokensByDocumentKey.get(documentKey);
    if (primed) return Promise.resolve(primed.slice(0));
    if (token?.isCancellationRequested) return Promise.resolve(null);

    return new Promise((resolve) => {
      let settled = false;
      const finish = (tokens: ArrayBuffer | null) => {
        if (settled) return;
        settled = true;
        clearTimeout(timeout);
        pendingByDocumentKey.get(documentKey)?.delete(finish);
        if (pendingByDocumentKey.get(documentKey)?.size === 0) pendingByDocumentKey.delete(documentKey);
        resolve(tokens);
      };
      const timeout = setTimeout(() => finish(null), SEMANTIC_TOKENS_PRIME_WAIT_MS);
      let pending = pendingByDocumentKey.get(documentKey);
      if (!pending) {
        pending = new Set();
        pendingByDocumentKey.set(documentKey, pending);
      }
      pending.add(finish);
    });
  };

  const refreshSemanticTokens = (_languageId?: string) => {
    activeRefreshListeners.forEach((listener) => listener());
  };

  const primeSemanticTokens = (documentKey: string, semanticTokens: ArrayBuffer) => {
    if (!documentKey) return;
    const tokens = semanticTokens.slice(0);
    semanticTokensByDocumentKey.set(documentKey, tokens);
    settlePending(documentKey, tokens);
  };

  const clearSemanticTokens = (documentKey?: string) => {
    if (documentKey) {
      semanticTokensByDocumentKey.delete(documentKey);
      settlePending(documentKey, null);
      return;
    }
    semanticTokensByDocumentKey.clear();
    settleAllPending();
  };

  const ensureSemanticTokensProvider = function ensureSemanticTokensProvider(languageId: string) {
    if (registeredLanguages.has(languageId)) return;
    ensureLanguageRegistered(monaco, languageId);
    registeredLanguages.add(languageId);
    const provider: Monaco.languages.DocumentSemanticTokensProvider = {
      onDidChange: (listener) => {
        const wrapped = () => listener(undefined);
        activeRefreshListeners.add(wrapped);
        return {
          dispose: () => {
            activeRefreshListeners.delete(wrapped);
          },
        };
      },
      getLegend: () => ({
        tokenTypes: [...tokenTypes],
        tokenModifiers: [],
      }),
      provideDocumentSemanticTokens: async (
        textModel: Monaco.editor.ITextModel,
        _lastResultId: string | null,
        token: Monaco.CancellationToken,
      ) => {
        const requestVersionId = textModel.getVersionId();
        const documentKey =
          (textModel as Monaco.editor.ITextModel & { __treeaseDocumentKey?: string }).__treeaseDocumentKey ??
          `${textModel.uri.toString()}-${textModel.getVersionId()}`;
        if (token?.isCancellationRequested) return { data: new Uint32Array() };
        try {
          const _validate = (data: Uint32Array) =>
            validateSemanticTokensData(data, textModel.getLineCount(), (line) => textModel.getLineMaxColumn(line) - 1);
          const _isCurrentVersion = () => textModel.getVersionId() === requestVersionId;

          const primed = semanticTokensByDocumentKey.get(documentKey);
          if (primed) {
            if (token?.isCancellationRequested) return { data: new Uint32Array() };
            if (!_isCurrentVersion()) return { data: new Uint32Array() };
            const data = new Uint32Array(primed.slice(0));
            if (!_validate(data)) return { data: new Uint32Array() };
            return { data };
          }
          const resolved = await resolveDocumentAnalysis({
            documentKey,
          });
          if (token?.isCancellationRequested) return { data: new Uint32Array() };
          if (!_isCurrentVersion()) return { data: new Uint32Array() };
          if (resolved.status !== 'resolved' || !(resolved.analysis.semanticTokens instanceof ArrayBuffer)) {
            if (!(options.isImportActive?.() ?? false)) {
              const delayed = await waitForPrimedSemanticTokens(documentKey, token);
              if (delayed && !token?.isCancellationRequested) {
                if (!_isCurrentVersion()) return { data: new Uint32Array() };
                const data = new Uint32Array(delayed);
                if (!_validate(data)) return { data: new Uint32Array() };
                return { data };
              }
            }
            return { data: new Uint32Array() };
          }
          primeSemanticTokens(documentKey, resolved.analysis.semanticTokens);
          if (!_isCurrentVersion()) return { data: new Uint32Array() };
          const freshData = new Uint32Array(resolved.analysis.semanticTokens.slice(0));
          if (!_validate(freshData)) return { data: new Uint32Array() };
          return { data: freshData };
        } catch (error) {
          console.error('[semanticTokens] failed', { language: languageId, error });
          return { data: new Uint32Array() };
        }
      },
      releaseDocumentSemanticTokens: () => {},
    };
    monaco.languages.registerDocumentSemanticTokensProvider(languageId, provider);
  };
  return Object.assign(ensureSemanticTokensProvider, {
    refreshSemanticTokens,
    primeSemanticTokens,
    clearSemanticTokens,
  });
}

export function createColorProviderRegistrar(options: DocumentColorRegistrarOptions): DocumentColorRegistrar {
  const { monaco } = options;
  const ensureDocumentColorProvider = createDocumentColorRegistrar(options);
  function ensureColorProvider(languageId: string) {
    ensureLanguageRegistered(monaco, languageId);
    ensureDocumentColorProvider(languageId);
  }
  return Object.assign(ensureColorProvider, {
    updateViewport: ensureDocumentColorProvider.updateViewport,
    refreshVisibleColors: ensureDocumentColorProvider.refreshVisibleColors,
  });
}
