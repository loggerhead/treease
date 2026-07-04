import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('../services/TreePathService', () => ({
  resolveTreePathResult: vi.fn(),
  resolvePathSpanResult: vi.fn(),
  toByteColumn: vi.fn((text: string, columnIndex: number) => new TextEncoder().encode(text.slice(0, columnIndex)).length),
}));
vi.mock('../store/workspace-snapshot-bindings', () => ({
  getWorkspaceSnapshotId: vi.fn(() => 7),
}));

vi.mock('../components/Editor/editor-position-target', () => ({
  resolveEditorPositionTargetResult: vi.fn(),
}));

vi.mock('../settings/settings-store', () => ({
  settings: {},
}));

vi.mock('svelte/store', async () => {
  const actual = await vi.importActual<typeof import('svelte/store')>('svelte/store');
  return {
    ...actual,
    get: vi.fn(() => ({ parser: { enableNest: true } })),
  };
});

import { resolvePathSpanResult, resolveTreePathResult } from '../services/TreePathService';
import { resolveEditorPositionTargetResult } from '../components/Editor/editor-position-target';
import { createDocumentColorRegistrar } from './editor-color-provider';

function createMonacoStub() {
  const providers = new Map<string, any>();
  class Position {
    constructor(
      public lineNumber: number,
      public column: number,
    ) {}
  }
  class Range {
    constructor(
      public startLineNumber: number,
      public startColumn: number,
      public endLineNumber: number,
      public endColumn: number,
    ) {}
  }
  return {
    providers,
    monaco: {
      Position,
      Range,
      editor: {
        getEditors: vi.fn(() => []),
      },
      languages: {
        registerColorProvider: vi.fn((languageId: string, provider: unknown) => {
          providers.set(languageId, provider);
          return { dispose: vi.fn() };
        }),
      },
    } as any,
  };
}

function createModel(text: string, languageId = 'json') {
  const lines = text.split('\n');
  return {
    uri: { toString: () => 'inmemory://treease/test' },
    getVersionId: () => 1,
    getValue: () => text,
    getValueLength: () => text.length,
    getLanguageId: () => languageId,
    getLineCount: () => lines.length,
    getLineMaxColumn: (lineNumber: number) => (lines[lineNumber - 1]?.length ?? 0) + 1,
    getValueInRange: (range: { startLineNumber: number; startColumn: number; endLineNumber: number; endColumn: number }) => {
      const startLine = range.startLineNumber - 1;
      const endLine = range.endLineNumber - 1;
      if (startLine === endLine) {
        return (lines[startLine] ?? '').slice(range.startColumn - 1, range.endColumn - 1);
      }
      let result = (lines[startLine] ?? '').slice(range.startColumn - 1);
      for (let i = startLine + 1; i < endLine; i++) {
        result += '\n' + (lines[i] ?? '');
      }
      result += '\n' + (lines[endLine] ?? '').slice(0, range.endColumn - 1);
      return result;
    },
    getPositionAt: (offset: number) => {
      const prefix = text.slice(0, offset);
      const lines = prefix.split('\n');
      return { lineNumber: lines.length, column: (lines.at(-1)?.length ?? 0) + 1 };
    },
    getOffsetAt: (position: { lineNumber: number; column: number }) => {
      let offset = 0;
      for (let line = 1; line < position.lineNumber; line += 1) {
        offset += (lines[line - 1]?.length ?? 0) + 1;
      }
      return offset + position.column - 1;
    },
  } as any;
}

function createLargeText(colorLine: number): string {
  const lines = Array.from({ length: 700 }, (_, index) =>
    index + 1 === colorLine ? '{"color":"#4f46e5"}' : `"padding-${index.toString().padStart(4, '0')}":"${'a'.repeat(180)}"`,
  );
  return lines.join('\n');
}

function createSmallViewportText(colorLine: number): string {
  const lines = Array.from({ length: 260 }, (_, index) =>
    index + 1 === colorLine ? '{"color":"#4f46e5"}' : `"padding-${index.toString().padStart(4, '0')}":"short"`,
  );
  return lines.join('\n');
}

describe('createDocumentColorRegistrar', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('registers a provider once per language and detects JSON value colors', async () => {
    const { monaco, providers } = createMonacoStub();
    const ensure = createDocumentColorRegistrar({ monaco });
    ensure('json');
    ensure('json');

    expect(monaco.languages.registerColorProvider).toHaveBeenCalledTimes(1);
    const provider = providers.get('json');
    const text = '{"color":"#4f46e5ff","other":"text"}';
    const colorStart = text.indexOf('#4f46e5ff');
    const colorEnd = colorStart + '#4f46e5ff'.length;

    vi.mocked(resolveTreePathResult).mockResolvedValue({ status: 'ready', data: [{ tag: 0, key: 'color', index: 0 }] } as any);
    vi.mocked(resolveEditorPositionTargetResult).mockResolvedValue({ status: 'ready', data: 'value' });
    vi.mocked(resolvePathSpanResult).mockResolvedValue({
      status: 'ready',
      data: { startByte: colorStart - 1, endByte: colorEnd + 1, row: 0, column: colorStart - 1 },
    } as any);

    const colors = await provider.provideDocumentColors(createModel(text), { isCancellationRequested: false });
    expect(colors).toHaveLength(1);
    expect(colors[0].range).toMatchObject({
      startLineNumber: 1,
      startColumn: colorStart + 1,
      endLineNumber: 1,
      endColumn: colorEnd + 1,
    });
    expect(colors[0].color).toMatchObject({
      red: 79 / 255,
      green: 70 / 255,
      blue: 229 / 255,
      alpha: 1,
    });
  });

  it('ignores key matches when target resolves to key', async () => {
    const { monaco, providers } = createMonacoStub();
    const ensure = createDocumentColorRegistrar({ monaco });
    ensure('json');

    const provider = providers.get('json');
    const text = '{"#4f46e5":"value"}';
    vi.mocked(resolveTreePathResult).mockResolvedValue({ status: 'ready', data: [{ tag: 0, key: '#4f46e5', index: 0 }] } as any);
    vi.mocked(resolveEditorPositionTargetResult).mockResolvedValue({ status: 'ready', data: 'key' });

    const colors = await provider.provideDocumentColors(createModel(text), { isCancellationRequested: false });
    expect(colors).toEqual([]);
  });

  it('stops color resolution when Monaco cancels the request mid-flight', async () => {
    const { monaco, providers } = createMonacoStub();
    const ensure = createDocumentColorRegistrar({ monaco });
    ensure('json');

    const provider = providers.get('json');
    const text = '{"color":"#4f46e5"}';
    const token = { isCancellationRequested: false };

    vi.mocked(resolveTreePathResult).mockImplementationOnce(async () => {
      token.isCancellationRequested = true;
      return { status: 'ready', data: [{ tag: 0, key: 'color', index: 0 }] } as any;
    });

    const colors = await provider.provideDocumentColors(createModel(text), token as any);

    expect(colors).toEqual([]);
    expect(resolveEditorPositionTargetResult).not.toHaveBeenCalled();
    expect(resolvePathSpanResult).not.toHaveBeenCalled();
  });

  it('stops color resolution when the model language changes mid-flight', async () => {
    const { monaco, providers } = createMonacoStub();
    const ensure = createDocumentColorRegistrar({ monaco });
    ensure('json');

    const provider = providers.get('json');
    const text = '{"color":"#4f46e5"}';
    let languageId = 'json';
    const model = {
      ...createModel(text, 'json'),
      getLanguageId: () => languageId,
    } as any;

    vi.mocked(resolveTreePathResult).mockImplementationOnce(async () => {
      languageId = 'yaml';
      return { status: 'ready', data: [{ tag: 0, key: 'color', index: 0 }] } as any;
    });

    const colors = await provider.provideDocumentColors(model, { isCancellationRequested: false });

    expect(colors).toEqual([]);
    expect(resolveEditorPositionTargetResult).not.toHaveBeenCalled();
    expect(resolvePathSpanResult).not.toHaveBeenCalled();
  });

  it('preserves original color family in presentations', () => {
    const { monaco, providers } = createMonacoStub();
    const ensure = createDocumentColorRegistrar({ monaco });
    ensure('json');

    const provider = providers.get('json');
    const presentations = provider.provideColorPresentations(
      createModel('{"color":"rgba(79, 70, 229, 0.5)"}'),
      {
        range: new monaco.Range(1, 11, 1, 31),
        color: {
          red: 79 / 255,
          green: 70 / 255,
          blue: 229 / 255,
          alpha: 0.5,
        },
        __treeaseColorInfo: {
          format: 'rgba',
          originalText: 'rgba(79, 70, 229, 0.5)',
        },
      },
      { isCancellationRequested: false },
    );

    expect(presentations.map((item: { label: string }) => item.label)).toEqual([
      '#4f46e580',
      'rgba(79, 70, 229, 0.5)',
      'hsla(243, 75%, 59%, 0.5)',
    ]);
  });

  it('prefers the model document key when resolving tree path metadata', async () => {
    const { monaco, providers } = createMonacoStub();
    const ensure = createDocumentColorRegistrar({ monaco });
    ensure('json');

    const provider = providers.get('json');
    const model = createModel('{"color":"#4f46e5"}');
    model.__treeaseDocumentKey = 'tab-doc:42';

    vi.mocked(resolveTreePathResult).mockResolvedValue({ status: 'ready', data: [{ tag: 0, key: 'color', index: 0 }] } as any);
    vi.mocked(resolveEditorPositionTargetResult).mockResolvedValue({ status: 'ready', data: 'value' });
    vi.mocked(resolvePathSpanResult).mockResolvedValue({ status: 'ready', data: { startByte: 9, endByte: 18, row: 0, column: 9 } } as any);

    await provider.provideDocumentColors(model, { isCancellationRequested: false });

    expect(resolveTreePathResult).toHaveBeenCalledWith(
      model,
      expect.anything(),
      'tab-doc:42',
      'json',
      true,
      7,
    );
  });

  it('limits document color detection to the registered viewport', async () => {
    const { monaco, providers } = createMonacoStub();
    const ensure = createDocumentColorRegistrar({ monaco });
    ensure('json');

    const provider = providers.get('json');
    const text = createLargeText(220);
    const colorStart = text.indexOf('#4f46e5');
    const colorEnd = colorStart + '#4f46e5'.length;
    const model = createModel(text);
    ensure.updateViewport(model, [new monaco.Range(20, 1, 20, 20)]);

    vi.mocked(resolveTreePathResult).mockResolvedValue({ status: 'ready', data: [{ tag: 0, key: 'color', index: 0 }] } as any);
    vi.mocked(resolveEditorPositionTargetResult).mockResolvedValue({ status: 'ready', data: 'value' });
    vi.mocked(resolvePathSpanResult).mockResolvedValue({
      status: 'ready',
      data: { startByte: colorStart - 1, endByte: colorEnd + 1, row: 0, column: colorStart - 1 },
    } as any);

    const colors = await provider.provideDocumentColors(model, { isCancellationRequested: false });

    expect(colors).toHaveLength(1);
    expect(resolveTreePathResult).toHaveBeenCalledTimes(1);
  });

  it('skips large document color candidates outside the viewport', async () => {
    const { monaco, providers } = createMonacoStub();
    const ensure = createDocumentColorRegistrar({ monaco });
    ensure('json');

    const provider = providers.get('json');
    const model = createModel(createLargeText(650));
    ensure.updateViewport(model, [new monaco.Range(20, 1, 20, 20)]);

    const colors = await provider.provideDocumentColors(model, { isCancellationRequested: false });

    expect(colors).toEqual([]);
    expect(resolveTreePathResult).not.toHaveBeenCalled();
    expect(resolveEditorPositionTargetResult).not.toHaveBeenCalled();
    expect(resolvePathSpanResult).not.toHaveBeenCalled();
  });

  it('also skips small document color candidates outside the viewport', async () => {
    const { monaco, providers } = createMonacoStub();
    const ensure = createDocumentColorRegistrar({ monaco });
    ensure('json');

    const provider = providers.get('json');
    const model = createModel(createSmallViewportText(230));
    ensure.updateViewport(model, [new monaco.Range(1, 1, 1, 20)]);

    const colors = await provider.provideDocumentColors(model, { isCancellationRequested: false });

    expect(colors).toEqual([]);
    expect(resolveTreePathResult).not.toHaveBeenCalled();
    expect(resolveEditorPositionTargetResult).not.toHaveBeenCalled();
    expect(resolvePathSpanResult).not.toHaveBeenCalled();
  });
});
