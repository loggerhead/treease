// Responsibility: unit tests for editor-hover.
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { valueToTreeNode } from '../../../shared/tree-node-value';
import { SemType, TreeKind } from '@core-wasm/index';

vi.mock('../../services/TreePathService', () => ({
  resolveTreePathResult: vi.fn(),
  resolvePathSpanResult: vi.fn(),
}));
vi.mock('../../store/workspace-store', () => ({
  getWorkspaceSnapshotId: vi.fn(() => 7),
}));
vi.mock('../../services/SnapshotProjectionService', () => ({
  queryNodePreview: vi.fn(),
  queryPathValue: vi.fn(),
  nodePreviewToTreeNode: vi.fn((preview) => ({
    kind: preview.kind,
    semType: preview.semType,
    tag: preview.tag,
    value: preview.value,
    children: [],
  })),
}));

vi.mock('../../preview', () => ({
  generatePreview: vi.fn(),
}));

vi.mock('./editor-position-target', () => ({
  resolveEditorPositionTargetResult: vi.fn(),
}));

import { generatePreview } from '../../preview';
import { resolvePathSpanResult, resolveTreePathResult } from '../../services/TreePathService';
import { queryNodePreview, queryPathValue } from '../../services/SnapshotProjectionService';
import { resolveEditorPositionTargetResult } from './editor-position-target';
import { registerEditorHoverPreview } from './editor-hover';

function getProvider(monaco: any, language = 'json') {
  const call = vi.mocked(monaco.languages.registerHoverProvider).mock.calls.find(([id]) => id === language);
  if (!call) throw new Error(`provider not found for ${language}`);
  return call[1];
}

describe('registerEditorHoverPreview', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('builds hover content from TreeNode value and raw path span', async () => {
    const text = '{"message":"\\u4f60\\u597d"}';
    const startByte = text.indexOf('"\\u4f60');
    const endByte = startByte + '"\\u4f60\\u597d"'.length;
    const tree = valueToTreeNode({ message: '你好' });
    const model = {
      getValue: vi.fn(() => text),
    } as any;
    const editor = {
      getModel: vi.fn(() => model),
    } as any;
    const providers = new Map<string, any>();
    const monaco = {
      languages: {
        registerHoverProvider: vi.fn((language: string, provider: unknown) => {
          providers.set(language, provider);
          return { dispose: vi.fn() };
        }),
      },
      Range: class Range {
        constructor(
          public startLineNumber: number,
          public startColumn: number,
          public endLineNumber: number,
          public endColumn: number,
        ) {}
      },
    } as any;

    vi.mocked(resolveTreePathResult).mockResolvedValue({ status: 'ready', data: [{ tag: 0, key: 'message', index: 0 }] } as any);
    vi.mocked(resolveEditorPositionTargetResult).mockResolvedValue({ status: 'ready', data: 'value' });
    vi.mocked(resolvePathSpanResult).mockResolvedValue({ status: 'ready', data: { startByte, endByte, row: 0, column: 11 } } as any);
    vi.mocked(queryNodePreview).mockResolvedValue({
      status: 'ready',
      data: {
        kind: TreeKind.SCALAR,
        semType: SemType.STR,
        tag: 'str',
        value: '你好',
        valueType: 'string',
        isScalar: true,
      },
    });
    vi.mocked(queryPathValue).mockResolvedValue({
      status: 'ready',
      data: {
        valueType: 'string',
        value: '你好',
        sourceText: '"\\u4f60\\u597d"',
        displayText: '"\\u4f60\\u597d"',
      },
    });
    vi.mocked(generatePreview).mockResolvedValue('<div>Preview</div>');

    registerEditorHoverPreview({
      monaco,
      editor,
      getTreeState: () => ({ tree, value: { message: '你好' }, revision: 1, source: 'editor' }),
      getRevision: () => 1,
      getDocumentKey: () => 'vitest://hover',
      getLanguageId: () => 'json',
      getNestEnabled: () => true,
    });

    const provider = getProvider(monaco);
    const hover = await provider.provideHover(model, { lineNumber: 1, column: 15 });

    expect(generatePreview).toHaveBeenCalledWith(
      expect.objectContaining({
        value: '你好',
        rawValue: '"\\u4f60\\u597d"',
        language: 'json',
      }),
    );
    expect(hover.contents).toEqual([{ value: '<div>Preview</div>', supportHtml: true, isTrusted: true }]);
  });

  it('returns null for key targets without calling preview generation', async () => {
    const model = {
      getValue: vi.fn(() => '{"message":"hello"}'),
    } as any;
    const editor = {
      getModel: vi.fn(() => model),
    } as any;
    const monaco = {
      languages: {
        registerHoverProvider: vi.fn((_language: string, provider: unknown) => {
          return { dispose: vi.fn(), provider };
        }),
      },
      Range: class Range {},
    } as any;

    vi.mocked(resolveTreePathResult).mockResolvedValue({ status: 'ready', data: [{ tag: 0, key: 'message', index: 0 }] } as any);
    vi.mocked(resolveEditorPositionTargetResult).mockResolvedValue({ status: 'ready', data: 'key' });

    registerEditorHoverPreview({
      monaco,
      editor,
      getTreeState: () => ({ tree: valueToTreeNode({ message: 'hello' }), value: {}, revision: 2, source: 'editor' }),
      getRevision: () => 2,
      getDocumentKey: () => 'vitest://hover/key',
      getLanguageId: () => 'json',
      getNestEnabled: () => true,
    });

    const provider = getProvider(monaco);
    const hover = await provider.provideHover(model, { lineNumber: 1, column: 3 });

    expect(hover).toBeNull();
    expect(generatePreview).not.toHaveBeenCalled();
  });


  it('maps multiple preview blocks into hover contents', async () => {
    const model = {
      getValue: vi.fn(() => '{"message":"hello"}'),
    } as any;
    const editor = {
      getModel: vi.fn(() => model),
    } as any;
    const monaco = {
      languages: {
        registerHoverProvider: vi.fn((_language: string, provider: unknown) => {
          return { dispose: vi.fn(), provider };
        }),
      },
      Range: class Range {},
    } as any;

    vi.mocked(resolveTreePathResult).mockResolvedValue({ status: 'ready', data: [{ tag: 0, key: 'message', index: 0 }] } as any);
    vi.mocked(resolveEditorPositionTargetResult).mockResolvedValue({ status: 'ready', data: 'value' });
    vi.mocked(resolvePathSpanResult).mockResolvedValue({ status: 'ready', data: { startByte: 11, endByte: 18, row: 0, column: 11 } } as any);
    vi.mocked(queryNodePreview).mockResolvedValue({
      status: 'ready',
      data: {
        kind: TreeKind.SCALAR,
        semType: SemType.STR,
        tag: 'str',
        value: 'hello',
        valueType: 'string',
        isScalar: true,
      },
    });
    vi.mocked(queryPathValue).mockResolvedValue({
      status: 'ready',
      data: {
        valueType: 'string',
        value: 'hello',
        sourceText: '"hello"',
        displayText: '"hello"',
      },
    });
    vi.mocked(generatePreview).mockResolvedValue(['<div>A</div>', '<div>B</div>']);

    registerEditorHoverPreview({
      monaco,
      editor,
      getTreeState: () => ({ tree: valueToTreeNode({ message: 'hello' }), value: {}, revision: 4, source: 'editor' }),
      getRevision: () => 4,
      getDocumentKey: () => 'vitest://hover/multi',
      getLanguageId: () => 'json',
      getNestEnabled: () => true,
    });

    const provider = getProvider(monaco);
    const hover = await provider.provideHover(model, { lineNumber: 1, column: 14 });

    expect(hover.contents).toEqual([
      { value: '<div>A</div>', supportHtml: true, isTrusted: true },
      { value: '<div>B</div>', supportHtml: true, isTrusted: true },
    ]);
  });

  it('ignores hover requests outside the current model line range', async () => {
    const model = {
      getValue: vi.fn(() => '{"message":"hello"}'),
      getLineCount: vi.fn(() => 1),
      getLineMaxColumn: vi.fn(() => 20),
      getVersionId: vi.fn(() => 1),
      getLanguageId: vi.fn(() => 'json'),
    } as any;
    const editor = {
      getModel: vi.fn(() => model),
    } as any;
    const monaco = {
      languages: {
        registerHoverProvider: vi.fn((_language: string, provider: unknown) => {
          return { dispose: vi.fn(), provider };
        }),
      },
      Range: class Range {},
    } as any;

    registerEditorHoverPreview({
      monaco,
      editor,
      getTreeState: () => ({ tree: valueToTreeNode({ message: 'hello' }), value: {}, revision: 5, source: 'editor' }),
      getRevision: () => 5,
      getDocumentKey: () => 'vitest://hover/out-of-range',
      getLanguageId: () => 'json',
      getNestEnabled: () => true,
    });

    const provider = getProvider(monaco);
    const hover = await provider.provideHover(model, { lineNumber: 2, column: 1 });

    expect(hover).toBeNull();
    expect(resolveTreePathResult).not.toHaveBeenCalled();
    expect(generatePreview).not.toHaveBeenCalled();
  });
});
