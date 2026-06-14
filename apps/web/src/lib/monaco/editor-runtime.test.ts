import { describe, expect, it, vi } from 'vitest';
import { TOKEN_TYPES } from '@core-wasm/index';

vi.mock('./monaco-setup', () => ({
  createSemanticTokensRegistrar: vi.fn(() => Object.assign(vi.fn(), { refreshSemanticTokens: vi.fn() })),
  createColorProviderRegistrar: vi.fn(() =>
    Object.assign(vi.fn(), {
      updateViewport: vi.fn(),
      refreshVisibleColors: vi.fn(),
    }),
  ),

}));

vi.mock('./contributions', () => ({
  loadMonacoContributions: vi.fn(async () => {}),
}));
vi.mock('./yq-language-support', () => ({
  createYqLanguageSupportRegistrar: vi.fn(() => vi.fn()),
}));


vi.mock('monaco-editor/min/vs/editor/editor.main.css', () => ({}));

const configurationService = { updateValue: vi.fn() };
const StandaloneServices = { get: vi.fn(() => configurationService) };

const editorWorker = vi.fn();
const jsonWorker = vi.fn();

vi.mock('./runtime-adapter', () => ({
  loadMonacoApi: vi.fn(async () => ({
    editor: {},
    languages: {},
  })),
  loadMonacoStandaloneConfiguration: vi.fn(async () => ({
    StandaloneServices,
    IConfigurationService: 'config',
  })),
  loadMonacoWorkers: vi.fn(async () => ({
    editorWorkerCtor: editorWorker,
    jsonWorkerCtor: jsonWorker,
  })),
}));

(globalThis as any).self = {};

import { getSharedMonacoRuntime, initMonacoRuntime } from './editor-runtime';
import { createColorProviderRegistrar, createSemanticTokensRegistrar } from './monaco-setup';
import { createYqLanguageSupportRegistrar } from './yq-language-support';
import { loadMonacoContributions } from './contributions';

describe('editor-runtime', () => {
  it('initializes Monaco environment and semantic highlighting', async () => {
    const getTokenTypes = vi.fn(async () => TOKEN_TYPES);
    const callWasmWorker = async <T,>() => new ArrayBuffer(0) as T;
    const runtime = await initMonacoRuntime({
      callWasmWorker,
      getTokenTypes,
    });

    expect(loadMonacoContributions).toHaveBeenCalledTimes(1);
    expect(getTokenTypes).toHaveBeenCalledTimes(1);
    expect(createSemanticTokensRegistrar).toHaveBeenCalledTimes(1);
    expect(createSemanticTokensRegistrar).toHaveBeenCalledWith(expect.objectContaining({ tokenTypes: TOKEN_TYPES }));
    expect(createColorProviderRegistrar).toHaveBeenCalledTimes(1);
    expect(StandaloneServices.get).toHaveBeenCalledWith('config');
    expect(configurationService.updateValue).toHaveBeenCalledWith('editor.semanticHighlighting.enabled', true);

    const env = (self as any).MonacoEnvironment;
    expect(env).toBeTruthy();
    expect(env.getWorker(undefined, 'json')).toBeInstanceOf(jsonWorker as any);
    expect(env.getWorker(undefined, 'other')).toBeInstanceOf(editorWorker as any);

    expect(runtime.monaco).toBeTruthy();
    expect(typeof runtime.ensureSemanticTokensProvider).toBe('function');
    expect(typeof runtime.refreshSemanticTokens).toBe('function');
    expect(typeof runtime.ensureDocumentColorProvider).toBe('function');
    expect(typeof runtime.updateDocumentColorViewport).toBe('function');
    expect(typeof runtime.refreshVisibleDocumentColors).toBe('function');
    expect(createYqLanguageSupportRegistrar).toHaveBeenCalledTimes(1);
    expect(typeof runtime.ensureYqLanguageSupport).toBe('function');
  });

  it('surfaces contribution loading failures', async () => {
    vi.mocked(loadMonacoContributions).mockRejectedValueOnce(new Error('load failed'));
    const callWasmWorker = async <T,>() => new ArrayBuffer(0) as T;

    await expect(
      initMonacoRuntime({
        callWasmWorker,
        getTokenTypes: async () => ['type'],
      }),
    ).rejects.toThrow('load failed');
  });

  it('shares Monaco runtime initialization and retries after a failure', async () => {
    const loadCallCount = vi.mocked(loadMonacoContributions).mock.calls.length;
    vi.mocked(loadMonacoContributions).mockRejectedValueOnce(new Error('load failed'));
    const getTokenTypes = vi.fn(async () => TOKEN_TYPES);
    const callWasmWorker = async <T,>() => new ArrayBuffer(0) as T;

    await expect(
      getSharedMonacoRuntime({
        callWasmWorker,
        getTokenTypes,
      }),
    ).rejects.toThrow('load failed');

    const [firstRuntime, secondRuntime] = await Promise.all([
      getSharedMonacoRuntime({
        callWasmWorker,
        getTokenTypes,
      }),
      getSharedMonacoRuntime({
        callWasmWorker,
        getTokenTypes,
      }),
    ]);
    const cachedRuntime = await getSharedMonacoRuntime({
      callWasmWorker,
      getTokenTypes,
    });

    expect(firstRuntime).toBe(secondRuntime);
    expect(cachedRuntime).toBe(firstRuntime);
    expect(loadMonacoContributions).toHaveBeenCalledTimes(loadCallCount + 2);
    expect(getTokenTypes).toHaveBeenCalledTimes(1);
  });
});
