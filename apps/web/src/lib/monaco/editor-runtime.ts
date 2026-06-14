import {
  createColorProviderRegistrar,
  createSemanticTokensRegistrar,
} from './monaco-setup';
import { createYqLanguageSupportRegistrar } from './yq-language-support';
import type * as Monaco from 'monaco-editor';
import { loadMonacoContributions } from './contributions';
import { loadMonacoApi, loadMonacoStandaloneConfiguration, loadMonacoWorkers } from './runtime-adapter';
import type { MonacoApi } from './public-types';

export type MonacoRuntime = {
  monaco: MonacoApi;
  ensureSemanticTokensProvider: (languageId: string) => void;
  refreshSemanticTokens: (languageId?: string) => void;
  primeSemanticTokens: (documentKey: string, semanticTokens: ArrayBuffer) => void;
  clearSemanticTokens: (documentKey?: string) => void;
  ensureDocumentColorProvider: (languageId: string) => void;
  updateDocumentColorViewport: (model: Monaco.editor.ITextModel, visibleRanges: Monaco.Range[]) => void;
  refreshVisibleDocumentColors: (model: Monaco.editor.ITextModel) => void;
  ensureYqLanguageSupport: () => void;
};

export type MonacoShell = {
  monaco: MonacoApi;
};

export type MonacoLanguageServices = {
  ensureSemanticTokensProvider: (languageId: string) => void;
  refreshSemanticTokens: (languageId?: string) => void;
  primeSemanticTokens: (documentKey: string, semanticTokens: ArrayBuffer) => void;
  clearSemanticTokens: (documentKey?: string) => void;
  ensureDocumentColorProvider: (languageId: string) => void;
  updateDocumentColorViewport: (model: Monaco.editor.ITextModel, visibleRanges: Monaco.Range[]) => void;
  refreshVisibleDocumentColors: (model: Monaco.editor.ITextModel) => void;
  ensureYqLanguageSupport: () => void;
};

export type MonacoRuntimeOptions = {
  callWasmWorker: <T>(type: string, payload?: Record<string, any>, transfer?: Transferable[]) => Promise<T>;
  getTokenTypes: () => Promise<readonly string[]>;
  isImportActive?: () => boolean;
};

export async function initMonacoShell(): Promise<MonacoShell> {
  await import('monaco-editor/min/vs/editor/editor.main.css');
  const monaco = await loadMonacoApi();
  const [{ StandaloneServices, IConfigurationService }, { editorWorkerCtor, jsonWorkerCtor }] = await Promise.all([
    loadMonacoStandaloneConfiguration(),
    loadMonacoWorkers(),
  ]);
  await loadMonacoContributions();

  (self as any).MonacoEnvironment = {
    getWorker: function (_moduleId: any, _label: string) {
      if (_label === 'json') return new (jsonWorkerCtor as any)();
      return new (editorWorkerCtor as any)();
    },
  };
  const configurationService = StandaloneServices.get(IConfigurationService);
  configurationService.updateValue('editor.semanticHighlighting.enabled', true);

  return { monaco };
}

export async function initMonacoLanguageServices(
  shell: MonacoShell,
  options: MonacoRuntimeOptions,
): Promise<MonacoLanguageServices> {
  const { monaco } = shell;
  const { callWasmWorker, getTokenTypes, isImportActive } = options;
  const tokenTypes = await getTokenTypes();

  const ensureSemanticTokensProvider = createSemanticTokensRegistrar({
    monaco,
    callWasmWorker,
    tokenTypes,
    includeUri: true,
    logRegister: false,
    isImportActive,
  });
  const ensureDocumentColorProvider = createColorProviderRegistrar({ monaco });
  const ensureYqLanguageSupport = createYqLanguageSupportRegistrar({ monaco });

  return {
    ensureSemanticTokensProvider,
    refreshSemanticTokens: ensureSemanticTokensProvider.refreshSemanticTokens,
    primeSemanticTokens: ensureSemanticTokensProvider.primeSemanticTokens,
    clearSemanticTokens: ensureSemanticTokensProvider.clearSemanticTokens,
    ensureDocumentColorProvider,
    updateDocumentColorViewport: ensureDocumentColorProvider.updateViewport,
    refreshVisibleDocumentColors: ensureDocumentColorProvider.refreshVisibleColors,
    ensureYqLanguageSupport,
  };
}

export async function initMonacoRuntime(options: MonacoRuntimeOptions): Promise<MonacoRuntime> {
  const shell = await initMonacoShell();
  const lang = await initMonacoLanguageServices(shell, options);
  return { ...shell.monaco, monaco: shell.monaco, ...lang };
}

// ── Singletons ───────────────────────────────────────────────────────────

let sharedShellPromise: Promise<MonacoShell> | null = null;
let sharedLangServicesPromise: Promise<MonacoLanguageServices> | null = null;
let sharedRuntimePromise: Promise<MonacoRuntime> | null = null;

export function getSharedMonacoShell(): Promise<MonacoShell> {
  if (sharedShellPromise) return sharedShellPromise;
  const promise = initMonacoShell().catch((error) => {
    if (sharedShellPromise === promise) sharedShellPromise = null;
    throw error;
  });
  sharedShellPromise = promise;
  return promise;
}

export function getSharedMonacoLanguageServices(options: MonacoRuntimeOptions): Promise<MonacoLanguageServices> {
  if (sharedLangServicesPromise) return sharedLangServicesPromise;
  const promise = getSharedMonacoShell()
    .then((shell) => initMonacoLanguageServices(shell, options))
    .catch((error) => {
      if (sharedLangServicesPromise === promise) sharedLangServicesPromise = null;
      throw error;
    });
  sharedLangServicesPromise = promise;
  return promise;
}

export function getSharedMonacoRuntime(options: MonacoRuntimeOptions): Promise<MonacoRuntime> {
  if (sharedRuntimePromise) return sharedRuntimePromise;
  const promise = getSharedMonacoShell()
    .then((shell) =>
      getSharedMonacoLanguageServices(options).then((lang) => ({
        ...shell.monaco,
        monaco: shell.monaco,
        ...lang,
      })),
    )
    .catch((error) => {
      if (sharedRuntimePromise === promise) sharedRuntimePromise = null;
      throw error;
    });
  sharedRuntimePromise = promise;
  return promise;
}
