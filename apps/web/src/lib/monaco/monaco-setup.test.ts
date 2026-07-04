import { describe, expect, it, vi } from 'vitest';

const languageRegistry: Array<{ id: string }> = [];

import { createSemanticTokensRegistrar, ensureLanguageRegistered } from './monaco-setup';
import { createYqLanguageSupportRegistrar } from './yq-language-support';
import { hasYqCompletionMatches } from './yq-language-support';

type CallWasmWorker = <T>(type: string, payload?: Record<string, any>, transfer?: Transferable[]) => Promise<T>;

const callWasmWorker: CallWasmWorker = (async <T,>() => ({ semanticTokens: new Uint32Array([1, 2, 3]).buffer } as T));

const makeModel = (text: string, options: { getVersionId?: () => number } = {}) => {
  const lines = text.split('\n');
  return {
    getValue: () => text,
    uri: { toString: () => 'file://test' },
    getVersionId: options.getVersionId ?? (() => 1),
    getLineCount: () => lines.length,
    getLineMaxColumn: (lineNumber: number) => (lines[lineNumber - 1]?.length ?? 0) + 1,
  };
};

const monaco = {
  languages: {
    getLanguages: () => languageRegistry,
    register: (lang: { id: string }) => languageRegistry.push(lang),
    registerDocumentSemanticTokensProvider: vi.fn(),
    setLanguageConfiguration: vi.fn(),
    setMonarchTokensProvider: vi.fn(),
    registerCompletionItemProvider: vi.fn(),
    CompletionItemKind: { Function: 1, Keyword: 2 },
  },
};

vi.mock('monaco-editor/esm/vs/editor/editor.api', () => monaco);

describe('monaco-setup', () => {
  it('registers language when missing', () => {
    languageRegistry.length = 0;
    ensureLanguageRegistered(monaco as any, 'json');
    expect(languageRegistry).toEqual([{ id: 'json' }]);
  });

  it('skips language registration when already present', () => {
    languageRegistry.length = 0;
    languageRegistry.push({ id: 'json' });
    ensureLanguageRegistered(monaco as any, 'json');
    expect(languageRegistry).toEqual([{ id: 'json' }]);
  });

  it('registers semantic tokens provider and returns empty tokens before analysis is primed', async () => {
    const ensure = createSemanticTokensRegistrar({
      monaco: monaco as any,
      callWasmWorker,
      tokenTypes: ['type'],
    });

    ensure('json');
    expect(monaco.languages.registerDocumentSemanticTokensProvider).toHaveBeenCalledTimes(1);

    const provider = monaco.languages.registerDocumentSemanticTokensProvider.mock.calls[0][1];
    expect(provider.getLegend()).toEqual({ tokenTypes: ['type'], tokenModifiers: [] });
    const res = await provider.provideDocumentSemanticTokens(makeModel('{"a":1}'), null, { isCancellationRequested: false });
    expect(res.data).toBeInstanceOf(Uint32Array);
    expect(res.data).toEqual(new Uint32Array());
  });

  it('returns empty tokens when cancelled', async () => {
    const ensure = createSemanticTokensRegistrar({
      monaco: monaco as any,
      callWasmWorker,
      tokenTypes: ['type'],
    });

    ensure('json');
    const provider = monaco.languages.registerDocumentSemanticTokensProvider.mock.calls[0][1];
    const res = await provider.provideDocumentSemanticTokens(makeModel('{"a":1}'), null, { isCancellationRequested: true });
    expect(res.data).toBeInstanceOf(Uint32Array);
    expect(res.data.length).toBe(0);
  });

  it('returns empty semantic tokens when tokens are not primed', async () => {
    const callWasmWorker = vi.fn(async <T,>(_type: string) => {
      return { semanticTokens: new Uint32Array([0, 0, 4, 0, 0]).buffer } as T;
    }) as unknown as CallWasmWorker;
    const ensure = createSemanticTokensRegistrar({
      monaco: monaco as any,
      callWasmWorker,
      tokenTypes: ['type'],
    });

    ensure('toml');
    const provider = monaco.languages.registerDocumentSemanticTokensProvider.mock.calls.at(-1)?.[1];
    const res = await provider.provideDocumentSemanticTokens(makeModel('enabled = true'), null, { isCancellationRequested: false });

    expect(res.data).toBeInstanceOf(Uint32Array);
    expect(res.data).toEqual(new Uint32Array());
    expect(callWasmWorker).not.toHaveBeenCalled();
  });

  it('returns empty semantic tokens while import is active when tokens are not primed', async () => {
    const callWasmWorker = vi.fn(async <T,>(type: string) => {
      if (type === 'getStoredDocumentAnalysis') {
        return { semanticTokens: new Uint32Array([0, 1, 5, 0, 0]).buffer } as T;
      }
      throw new Error(`unexpected ${type}`);
    }) as unknown as CallWasmWorker;
    const ensure = createSemanticTokensRegistrar({
      monaco: monaco as any,
      callWasmWorker,
      tokenTypes: ['type'],
      isImportActive: () => true,
    });

    ensure('json');
    const provider = monaco.languages.registerDocumentSemanticTokensProvider.mock.calls.at(-1)?.[1];
    const res = await provider.provideDocumentSemanticTokens(makeModel('{"a":1}'), null, { isCancellationRequested: false });

    expect(res.data).toBeInstanceOf(Uint32Array);
    expect(res.data.length).toBe(0);
    expect(callWasmWorker).not.toHaveBeenCalled();
  });

  it('returns primed semantic tokens while import is active', async () => {
    const callWasmWorker = vi.fn() as unknown as CallWasmWorker;
    const ensure = createSemanticTokensRegistrar({
      monaco: monaco as any,
      callWasmWorker,
      tokenTypes: ['type'],
      isImportActive: () => true,
    });

    ensure('json');
    ensure.primeSemanticTokens('file://test-1', new Uint32Array([0, 0, 4, 0, 0]).buffer);
    const model = makeModel('true') as ReturnType<typeof makeModel> & { __treeaseDocumentKey?: string };
    model.__treeaseDocumentKey = 'file://test-1';
    const provider = monaco.languages.registerDocumentSemanticTokensProvider.mock.calls.at(-1)?.[1];
    const res = await provider.provideDocumentSemanticTokens(model, null, { isCancellationRequested: false });

    expect(res.data).toEqual(new Uint32Array([0, 0, 4, 0, 0]));
    expect(callWasmWorker).not.toHaveBeenCalled();
  });

  it('waits for semantic tokens primed after an early cache miss', async () => {
    const callWasmWorker = vi.fn(async <T,>() => null as T) as unknown as CallWasmWorker;
    const ensure = createSemanticTokensRegistrar({
      monaco: monaco as any,
      callWasmWorker,
      tokenTypes: ['type'],
    });
    const model = makeModel('true') as ReturnType<typeof makeModel> & { __treeaseDocumentKey?: string };
    model.__treeaseDocumentKey = 'file://pending-json';

    ensure('json');
    const provider = monaco.languages.registerDocumentSemanticTokensProvider.mock.calls.at(-1)?.[1];
    const pending = provider.provideDocumentSemanticTokens(model, null, { isCancellationRequested: false });
    await Promise.resolve();
    ensure.primeSemanticTokens('file://pending-json', new Uint32Array([0, 0, 4, 0, 0]).buffer);
    const res = await pending;

    expect(res.data).toEqual(new Uint32Array([0, 0, 4, 0, 0]));
    expect(callWasmWorker).not.toHaveBeenCalled();
  });

  it('drops delayed semantic tokens when the model version changes mid-request', async () => {
    const callWasmWorker = vi.fn(async <T,>() => null as T) as unknown as CallWasmWorker;
    const ensure = createSemanticTokensRegistrar({
      monaco: monaco as any,
      callWasmWorker,
      tokenTypes: ['type'],
    });
    let versionId = 1;
    const model = makeModel('true', {
      getVersionId: () => versionId,
    }) as ReturnType<typeof makeModel> & { __treeaseDocumentKey?: string };
    model.__treeaseDocumentKey = 'file://pending-versioned-json';

    ensure('json');
    const provider = monaco.languages.registerDocumentSemanticTokensProvider.mock.calls.at(-1)?.[1];
    const pending = provider.provideDocumentSemanticTokens(model, null, { isCancellationRequested: false });
    await Promise.resolve();
    versionId = 2;
    ensure.primeSemanticTokens('file://pending-versioned-json', new Uint32Array([0, 0, 4, 0, 0]).buffer);
    const res = await pending;

    expect(res.data).toEqual(new Uint32Array());
    expect(callWasmWorker).not.toHaveBeenCalled();
  });

  it('exposes semantic token refresh hook through provider onDidChange', () => {
    const ensure = createSemanticTokensRegistrar({
      monaco: monaco as any,
      callWasmWorker,
      tokenTypes: ['type'],
    });

    ensure('json');
    const provider = monaco.languages.registerDocumentSemanticTokensProvider.mock.calls.at(-1)?.[1];
    const listener = vi.fn();
    const disposable = provider.onDidChange(listener);

    ensure.refreshSemanticTokens('json');
    expect(listener).toHaveBeenCalledTimes(1);

    disposable.dispose();
    ensure.refreshSemanticTokens('json');
    expect(listener).toHaveBeenCalledTimes(1);
  });

  it('refreshes active semantic token listeners across language switches', () => {
    const ensure = createSemanticTokensRegistrar({
      monaco: monaco as any,
      callWasmWorker,
      tokenTypes: ['type'],
    });

    ensure('json');
    const jsonProvider = monaco.languages.registerDocumentSemanticTokensProvider.mock.calls.at(-1)?.[1];
    const listener = vi.fn();
    jsonProvider.onDidChange(listener);

    ensure('toml');
    ensure.refreshSemanticTokens('toml');

    expect(listener).toHaveBeenCalledTimes(1);
  });

  it('registers yq language support once', () => {
    const ensure = createYqLanguageSupportRegistrar({ monaco: monaco as any });

    ensure();
    ensure();

    expect(languageRegistry).toContainEqual({ id: 'treease-yq' });
    expect(monaco.languages.setLanguageConfiguration).toHaveBeenCalledTimes(1);
    expect(monaco.languages.setMonarchTokensProvider).toHaveBeenCalledTimes(1);
    expect(monaco.languages.registerCompletionItemProvider).toHaveBeenCalledTimes(1);
  });

  it('does not re-register yq support across registrar instances', () => {
    const ensureA = createYqLanguageSupportRegistrar({ monaco: monaco as any });
    const ensureB = createYqLanguageSupportRegistrar({ monaco: monaco as any });

    ensureA();
    ensureB();

    expect(monaco.languages.setLanguageConfiguration).toHaveBeenCalledTimes(1);
    expect(monaco.languages.setMonarchTokensProvider).toHaveBeenCalledTimes(1);
    expect(monaco.languages.registerCompletionItemProvider).toHaveBeenCalledTimes(1);
  });

  it('matches yq completion prefixes only when suggestions exist', () => {
    expect(hasYqCompletionMatches('to')).toBe(true);
    expect(hasYqCompletionMatches('@to')).toBe(true);
    expect(hasYqCompletionMatches('.object')).toBe(false);
    expect(hasYqCompletionMatches('@object')).toBe(false);
  });
});
