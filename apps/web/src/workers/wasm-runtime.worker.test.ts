import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SemType, TreeKind } from '@core-wasm/index'

const mocked = vi.hoisted(() => ({
  AUXILIARY_TOKEN_TYPES: ['punctuation', 'comment', 'operator', 'function', 'variable', 'tag', 'attribute'],
  TREE_NODE_TOKEN_TYPES: ['map', 'key', 'seq', 'str', 'int', 'float', 'boolean', 'nil'],
  initWasm: vi.fn(async () => {}),
  isStructurallyEqual: vi.fn(async () => false),
  compareStructured: vi.fn(async () => false),
  diffStructured: vi.fn(async () => ({ pairs: [] })),
  diffText: vi.fn(async () => ({ pairs: [] })),
  parseToTree: vi.fn(async () => ({ kind: 0, semType: 3, tag: 'str', value: 'x', children: [] })),
  formatJson: vi.fn(async () => ({ text: 'formatted' })),
  minifyJson: vi.fn(async () => ({ text: 'minified' })),
  sortText: vi.fn(async () => 'sorted'),
  getSemanticTokens: vi.fn(async () => new Uint8Array()),
  getDiagnostics: vi.fn(async () => []),
  convertJson: vi.fn(async () => ({ text: 'converted' })),
  runYqText: vi.fn(async () => 'yq-output'),
  parseValueToTreeJson: vi.fn(),
  parseValueForPath: vi.fn(),
  planGraphValueEdit: vi.fn(),
  getStoredDocumentAnalysis: vi.fn(async () => null),
  findJsonBlockAtPosition: vi.fn(async () => ({ found: false, startByte: 0, endByte: 0, startRow: 0, startColumn: 0, endRow: 0, endColumn: 0 })),
  getTreePath: vi.fn(),
  getPathSpan: vi.fn(),
  applyValueEditCanonical: vi.fn(),
  getGraphDeltaChunk: vi.fn(),
  freeStreams: vi.fn(),
  decodeGraphDeltaPayload: vi.fn(),
  startDocumentJob: vi.fn(),
  advanceDocumentJob: vi.fn(),
  cancelDocumentJob: vi.fn(),
  querySnapshot: vi.fn(),
  buildHoverSubgraphProjection: vi.fn(),
  TOKEN_TYPES: [
    'map',
    'key',
    'seq',
    'str',
    'int',
    'float',
    'boolean',
    'nil',
    'punctuation',
    'comment',
    'operator',
    'function',
    'variable',
    'tag',
    'attribute',
  ],
}));


vi.mock('@core-wasm/index', () => ({
  initWasm: mocked.initWasm,
  isStructurallyEqual: mocked.isStructurallyEqual,
  compareStructured: mocked.compareStructured,
  diffStructured: mocked.diffStructured,
  diffText: mocked.diffText,
  parseToTree: mocked.parseToTree,
  formatJson: mocked.formatJson,
  minifyJson: mocked.minifyJson,
  sortText: mocked.sortText,
  getSemanticTokens: mocked.getSemanticTokens,
  getDiagnostics: mocked.getDiagnostics,
  convertJson: mocked.convertJson,
  runYqText: mocked.runYqText,
  parseValueToTreeJson: mocked.parseValueToTreeJson,
  parseValueForPath: mocked.parseValueForPath,
  planGraphValueEdit: mocked.planGraphValueEdit,
  getStoredDocumentAnalysis: mocked.getStoredDocumentAnalysis,
  findJsonBlockAtPosition: mocked.findJsonBlockAtPosition,
  getTreePath: mocked.getTreePath,
  getPathSpan: mocked.getPathSpan,
  applyValueEditCanonical: mocked.applyValueEditCanonical,
  getGraphDeltaChunk: mocked.getGraphDeltaChunk,
  freeStreams: mocked.freeStreams,
  decodeGraphDeltaPayload: mocked.decodeGraphDeltaPayload,
  startDocumentJob: mocked.startDocumentJob,
  advanceDocumentJob: mocked.advanceDocumentJob,
  cancelDocumentJob: mocked.cancelDocumentJob,
  querySnapshot: mocked.querySnapshot,
  buildHoverSubgraphProjection: mocked.buildHoverSubgraphProjection,
  AUXILIARY_TOKEN_TYPES: mocked.AUXILIARY_TOKEN_TYPES,
  TREE_NODE_TOKEN_TYPES: mocked.TREE_NODE_TOKEN_TYPES,
  TOKEN_TYPES: mocked.TOKEN_TYPES,
  PathSegTag: { KEY: 0, INDEX: 1 },
  SemType: { MAP: 1, SEQ: 2, STR: 3, INT: 4, FLOAT: 5, BOOLEAN: 6, NIL: 7 },
  TreeKind: { MAPPING: 10, SEQUENCE: 11, SCALAR: 9, ALIAS: 13 },
  GraphKind: { SCALAR: 0, TABLE: 1, OBJECT: 2 },
}));

vi.mock('@core-wasm/pkg', () => ({
  get_chunk_size_config: () => ({}),
}));

type WorkerMessage = { id: number; type: string; [key: string]: any };

function scalarNode(value: string) {
  return { kind: TreeKind.SCALAR, semType: SemType.STR, tag: 'str', value, children: [] };
}




function graphDeltaNode(overrides: Record<string, unknown> = {}) {
  const boxArgs = { x: 0, y: 0, width: 120, height: 32, cornerRadius: 6 };
  return {
    id: 1,
    kind: 0,
    depth: 0,
    boxArgs,
    path: [],
    meta: {
      text: 'root',
      semType: SemType.MAP,
      path: [],
      value: '',
      boxArgs,
      textArgs: { ...boxArgs, text: 'root', textAlign: 0, textVerticalAlign: 1, editable: 0 },
    },
    rows: [],
    ...overrides,
  };
}

function storedGraphDelta() {
  return {
    clear: 1,
    nodesAdded: [graphDeltaNode()],
    nodesUpdated: [],
    nodesRemoved: [],
    edgesAdded: [],
    edgesRemoved: [],
  };
}

function projectionGraphData(overrides: Record<string, unknown> = {}) {
  const graphData = storedGraphDelta();
  return {
    nodesAdded: graphData.nodesAdded,
    nodesUpdated: graphData.nodesUpdated,
    nodesRemoved: graphData.nodesRemoved,
    edgesAdded: graphData.edgesAdded,
    edgesRemoved: graphData.edgesRemoved,
    ...overrides,
  };
}

function projectionDelta(clear = false, graphData = projectionGraphData()) {
  return {
    clear,
    graphData,
  };
}

describe('wasm-runtime worker', () => {
  let posted: any[];
  let postCalls: Array<{ message: any; transfer?: Transferable[] }>;
  let selfMock: { postMessage: ReturnType<typeof vi.fn>; onmessage: ((event: { data: WorkerMessage }) => void) | null };

  async function send(message: WorkerMessage) {
    selfMock.onmessage?.({ data: message });
    await vi.waitFor(() => {
      expect(posted.some((entry: any) => entry?.id === message.id)).toBe(true);
    });
    return posted.findLast((entry: any) => entry?.id === message.id);
  }

  beforeEach(async () => {
    vi.resetModules();
    vi.resetAllMocks();

    // Restore default implementations
    mocked.initWasm.mockImplementation(async () => {});
    mocked.isStructurallyEqual.mockImplementation(async () => false);
    mocked.compareStructured.mockImplementation(async () => false);
    mocked.diffStructured.mockImplementation(async () => ({ pairs: [] }));
    mocked.diffText.mockImplementation(async () => ({ pairs: [] }));
    mocked.parseToTree.mockImplementation(async () => scalarNode('x'));
    mocked.formatJson.mockImplementation(async () => ({ text: 'formatted', cursor: 0 }));
    mocked.minifyJson.mockImplementation(async () => ({ text: 'minified', cursor: 0 }));
    mocked.sortText.mockImplementation(async () => 'sorted');
    mocked.getSemanticTokens.mockImplementation(async () => new Uint8Array());
    mocked.getDiagnostics.mockImplementation(async () => []);
    mocked.getStoredDocumentAnalysis.mockImplementation(async () => null);
    mocked.findJsonBlockAtPosition.mockImplementation(async () => ({ found: false, startByte: 0, endByte: 0, startRow: 0, startColumn: 0, endRow: 0, endColumn: 0 }));
    mocked.getGraphDeltaChunk.mockImplementation((result: any) =>
      result?.outputs?.graph?.chunk ? storedGraphDelta() : null,
    );
    mocked.convertJson.mockImplementation(async () => ({ text: 'converted' }));
    mocked.runYqText.mockImplementation(async () => 'yq-output');
    mocked.startDocumentJob.mockImplementation(async () => ({
      jobHandle: 1,
      batch: {
        requestSeq: 1,
        events: [],
        terminal: null,
      },
    }));
    mocked.advanceDocumentJob.mockImplementation(async ({ kind }: { kind: string }) =>
      kind === 'close'
        ? {
            requestSeq: 1,
            events: [{ type: 'snapshotReady', snapshotId: 1, analysis: null, mainGraph: null }],
            terminal: { type: 'completed' },
          }
        : {
            requestSeq: 1,
            events: [],
            terminal: null,
          },
    );
    mocked.cancelDocumentJob.mockImplementation(async () => ({
      requestSeq: 1,
      events: [],
      terminal: { type: 'cancelled' },
    }));
    mocked.querySnapshot.mockImplementation(async () => ({ status: 'ready', data: { anchors: [] } }));
    mocked.buildHoverSubgraphProjection.mockImplementation(async () => ({ status: 'ready', data: projectionDelta(false, projectionGraphData()) }));

    posted = [];
    postCalls = [];
    selfMock = {
      postMessage: vi.fn((message: any, transfer?: Transferable[]) => {
        posted.push(message);
        postCalls.push({ message, transfer });
      }),
      onmessage: null,
    };
    (globalThis as any).self = selfMock;

    await import('./wasm-runtime.worker');

    await send({ id: 1, type: 'init', wasmURL: 'file://fake.wasm' });
  });

  describe('compare', () => {
    it('returns equal when texts are byte-identical', async () => {
      const res = await send({ id: 2, type: 'compare', language: 'json', left: '{"a":1}', right: '{"a":1}' });

      expect(res.ok).toBe(true);
      expect(res.data).toEqual({ mode: 'tree', equal: true, result: { pairs: [] } });
      expect(mocked.isStructurallyEqual).not.toHaveBeenCalled();
    });

    it('uses same-language fast path when isStructurallyEqual returns true', async () => {
      mocked.isStructurallyEqual.mockImplementation(async () => true);

      const res = await send({ id: 3, type: 'compare', language: 'json', left: '{"a":1}', right: '{"a": 1}' });

      expect(res.ok).toBe(true);
      expect(res.data).toEqual({ mode: 'tree', equal: true, result: { pairs: [] } });
      expect(mocked.isStructurallyEqual).toHaveBeenCalledWith('json', '{"a":1}', '{"a": 1}');
      expect(mocked.parseToTree).not.toHaveBeenCalled();
      expect(mocked.diffStructured).not.toHaveBeenCalled();
      expect(mocked.diffText).not.toHaveBeenCalled();
    });

it('falls back to text diff directly when isStructurallyEqual throws', async () => {
      mocked.isStructurallyEqual.mockRejectedValueOnce(new Error('fast path failed'));
      const diffResult = {
        pairs: [{ hasLeft: true, hasRight: true, left: { byteOffset: 0, byteLength: 1 }, right: { byteOffset: 0, byteLength: 1 } }],
      };
      mocked.diffText.mockImplementation(async () => diffResult);

      const res = await send({ id: 4, type: 'compare', language: 'json', left: '{"a":1}', right: '{"b":2}' });

      expect(res.ok).toBe(true);
      expect(res.data.mode).toBe('text');
      expect(mocked.parseToTree).not.toHaveBeenCalled();
      expect(mocked.diffText).toHaveBeenCalled();
    });

    it('uses text compare directly when languages differ', async () => {
      const diffResult = {
        pairs: [{ hasLeft: true, hasRight: true, left: { byteOffset: 0, byteLength: 1 }, right: { byteOffset: 0, byteLength: 1 } }],
      };
      mocked.diffText.mockImplementation(async () => diffResult);

      const res = await send({
        id: 5,
        type: 'compare',
        language: 'json',
        leftLanguage: 'json',
        rightLanguage: 'yaml',
        left: '{"a":1}',
        right: 'a: 1',
      });

      expect(res.ok).toBe(true);
      expect(res.data).toEqual({ mode: 'text', equal: false, result: diffResult });
      expect(mocked.isStructurallyEqual).not.toHaveBeenCalled();
      expect(mocked.parseToTree).not.toHaveBeenCalled();
    });

it('falls back to text mode with unified whitespace handling when isStructurallyEqual fails', async () => {
      mocked.isStructurallyEqual.mockRejectedValueOnce(new Error('fast path failed'));
      mocked.diffText.mockImplementation(async () => ({
        pairs: [{ hasLeft: true, hasRight: true, left: { byteOffset: 1, byteLength: 1 }, right: { byteOffset: 1, byteLength: 3 } }],
      }));

      const res = await send({
        id: 6,
        type: 'compare',
        language: 'json',
        left: '{ }',
        right: '{   }',
      });

      expect(res.ok).toBe(true);
      expect(res.data).toEqual({ mode: 'text', equal: true, result: { pairs: [] } });
    });

it('returns structured diff when isStructurallyEqual returns false for structurally different texts', async () => {
      const diffResult = {
        pairs: [
          {
            hasLeft: true,
            hasRight: true,
            left: { byteOffset: 1, byteLength: 2 },
            right: { byteOffset: 1, byteLength: 4 },
          },
        ],
      };
      mocked.diffStructured.mockImplementation(async () => diffResult);

      const res = await send({
        id: 7,
        type: 'compare',
        language: 'json',
        left: '{ }',
        right: '{    }',
      });

      expect(res.ok).toBe(true);
      expect(res.data).toEqual({ mode: 'tree', equal: false, result: diffResult });
      expect(mocked.diffStructured).toHaveBeenCalledWith('json', '{ }', '{    }');
      expect(mocked.diffText).not.toHaveBeenCalled();
    });


    it('returns graph search ready empty result without indexing when query is blank', async () => {
      const res = await send({
        id: 7,
        type: 'graphSearch',
        documentKey: 'search-blank',
        language: 'json',
        text: '{"a":1}',
        query: '   ',
        nest: false,
      });

      expect(res.ok).toBe(true);
      expect(res.data).toEqual({ status: 'ready', data: [] });
    });

    it('graphSearch 缺少 current snapshot 时返回 snapshotNotReady', async () => {
      const res = await send({
        id: 71,
        type: 'graphSearch',
        documentKey: 'search-not-ready',
        snapshotId: null,
        language: 'json',
        query: 'a',
        nest: false,
      });

      expect(res.ok).toBe(true);
      expect(res.data).toEqual({ status: 'snapshotNotReady' });
      expect(mocked.querySnapshot).not.toHaveBeenCalled();
    });

    it('graphSearch returns snapshotNotReady when search index snapshot query is not ready', async () => {
      mocked.querySnapshot.mockResolvedValueOnce({ status: 'snapshotNotReady' });

      const res = await send({
        id: 72,
        type: 'graphSearch',
        documentKey: 'search-query-not-ready',
        snapshotId: 72,
        language: 'json',
        query: 'a',
        nest: false,
      });

      expect(res.ok).toBe(true);
      expect(res.data).toEqual({ status: 'snapshotNotReady' });
      expect(mocked.querySnapshot).toHaveBeenCalledWith(
        expect.objectContaining({
          documentKey: 'search-query-not-ready',
          snapshotId: 72,
          queryKind: 'searchIndex',
        }),
      );
    });

    it('treePath 缺少 current snapshot 时返回 snapshotNotReady', async () => {
      const res = await send({
        id: 8,
        type: 'treePath',
        documentKey: 'not-ready',
        language: 'json',
        text: '{"a":1}',
        row: 0,
        column: 1,
        nest: false,
      });

      expect(res.ok).toBe(true);
      expect(res.data).toEqual({ status: 'snapshotNotReady' });
      expect(mocked.startDocumentJob).not.toHaveBeenCalled();
      expect(mocked.querySnapshot).not.toHaveBeenCalled();
    });

    it('pathSpan 缺少 current snapshot 时返回 snapshotNotReady', async () => {
      const res = await send({
        id: 82,
        type: 'pathSpan',
        documentKey: 'not-ready',
        language: 'json',
        text: '{"a":1}',
        path: [],
        target: 'value',
        nest: false,
      });

      expect(res.ok).toBe(true);
      expect(res.data).toEqual({ status: 'snapshotNotReady' });
      expect(mocked.startDocumentJob).not.toHaveBeenCalled();
      expect(mocked.querySnapshot).not.toHaveBeenCalled();
    });

    it('returns JSON block text and editor range for cursor position', async () => {
      mocked.findJsonBlockAtPosition.mockImplementation(async () => ({
        found: true,
        startByte: 7,
        endByte: 14,
        startRow: 1,
        startColumn: 0,
        endRow: 1,
        endColumn: 7,
      }));

      const res = await send({
        id: 83,
        type: 'findJsonBlockAtPosition',
        documentKey: 'doc-jsonl',
        language: 'json',
        text: 'prefix\n{"a":1}\nsuffix',
        row: 1,
        column: 2,
      });

      expect(res.ok).toBe(true);
      expect(res.data).toEqual({
        found: true,
        text: '{"a":1}',
        startByte: 7,
        endByte: 14,
        startLineNumber: 2,
        startColumn: 1,
        endLineNumber: 2,
        endColumn: 8,
      });
      expect(mocked.findJsonBlockAtPosition).toHaveBeenCalledWith('json', 'prefix\n{"a":1}\nsuffix', 1, 2);
    });

    it('returns empty JSON block result when no block is found', async () => {
      mocked.findJsonBlockAtPosition.mockImplementation(async () => ({ found: false, startByte: 0, endByte: 0, startRow: 0, startColumn: 0, endRow: 0, endColumn: 0 }));

      const res = await send({
        id: 84,
        type: 'findJsonBlockAtPosition',
        documentKey: 'doc-jsonl',
        language: 'json',
        text: 'prefix\nnot-json\nsuffix',
        row: 1,
        column: 2,
      });

      expect(res.ok).toBe(true);
      expect(res.data).toEqual({
        found: false,
        text: '',
        startByte: 0,
        endByte: 0,
        startLineNumber: 1,
        startColumn: 1,
        endLineNumber: 1,
        endColumn: 1,
      });
    });

    it('converts JSON block byte offsets to editor ranges for non-ASCII text', async () => {
      mocked.findJsonBlockAtPosition.mockImplementation(async () => ({
        found: true,
        startByte: 7,
        endByte: 18,
        startRow: 1,
        startColumn: 0,
        endRow: 1,
        endColumn: 11,
      }));

      const text = '前缀\n{"a":"值"}\n后缀';
      const res = await send({
        id: 85,
        type: 'findJsonBlockAtPosition',
        documentKey: 'doc-jsonl-unicode',
        language: 'json',
        text,
        row: 1,
        column: 2,
      });

      expect(res.ok).toBe(true);
      expect(res.data).toEqual({
        found: true,
        text: '{"a":"值"}',
        startByte: 7,
        endByte: 18,
        startLineNumber: 2,
        startColumn: 1,
        endLineNumber: 2,
        endColumn: 10,
      });
      expect(mocked.findJsonBlockAtPosition).toHaveBeenCalledWith('json', text, 1, 2);
    });

    it('graphSearch 在 snapshot 已就绪时只依赖 query 结果，不读取旧 authoritative cache', async () => {
      mocked.querySnapshot
        .mockResolvedValueOnce({
          status: 'ready',
          data: {
            anchors: [],
            searchItems: [
              {
                path: '$.a',
                pathText: '$.a',
                label: 'a',
                keyText: 'a',
                valueText: '1',
                target: 'key',
              },
            ],
          },
        })
        .mockResolvedValueOnce({
          status: 'ready',
          data: { anchors: [{ path: '$.a', spanStart: 2, spanEnd: 3 }] },
        });

      const res = await send({
        id: 9,
        type: 'graphSearch',
        documentKey: 'stored-search',
        snapshotId: 9,
        language: 'json',
        query: 'a',
        nest: false,
      });

      expect(res.ok).toBe(true);
      expect(res.data).toMatchObject({
        status: 'ready',
        data: [
        {
          label: 'a',
          target: 'key',
          pathText: '$.a',
        },
      ],
      });
      expect(mocked.getStoredDocumentAnalysis).not.toHaveBeenCalled();
      expect(mocked.parseToTree).not.toHaveBeenCalled();
      expect(mocked.querySnapshot).toHaveBeenNthCalledWith(
        1,
        expect.objectContaining({
          documentKey: 'stored-search',
          snapshotId: 9,
          queryKind: 'searchIndex',
        }),
      );
      expect(mocked.querySnapshot).toHaveBeenNthCalledWith(
        2,
        expect.objectContaining({
          documentKey: 'stored-search',
          snapshotId: 9,
          queryKind: 'findAnchors',
          pathPattern: '$.a',
        }),
      );
    });
  });

  describe('document transforms', () => {
    it('forwards format, minify, sort, convert and runYq requests', async () => {
      mocked.formatJson.mockImplementation(async () => ({ text: '{\n  "a": 1\n}', cursor: 0 }));
      mocked.minifyJson.mockImplementation(async () => ({ text: '{"a":1}', cursor: 0 }));
      mocked.sortText.mockImplementation(async () => '{"a":1,"b":2}');
      mocked.convertJson.mockImplementation(async () => ({ text: 'a: 1' }));
      mocked.runYqText.mockImplementation(async () => '{"name":"Alice"}');

      const format = await send({
        id: 10,
        type: 'format',
        language: 'json',
        text: '{"a":1}',
        options: { nest: true },
      });
      const minify = await send({ id: 11, type: 'minify', language: 'json', text: '{\n  "a": 1\n}' });
      const sort = await send({ id: 12, type: 'sort', language: 'json', text: '{"b":2,"a":1}' });
      const convert = await send({
        id: 13,
        type: 'convert',
        sourceLanguage: 'json',
        targetFormat: 'yaml',
        text: '{"a":1}',
      });
      const runYq = await send({
        id: 14,
        type: 'runYq',
        language: 'json',
        text: '{"items":[{"name":"Alice"}]}',
        expression: '.items[0]',
        nest: true,
      });

      expect(format.data).toBe('{\n  "a": 1\n}');
      expect(minify.data).toBe('{"a":1}');
      expect(sort.data).toBe('{"a":1,"b":2}');
      expect(convert.data).toBe('a: 1');
      expect(runYq.data).toBe('{"name":"Alice"}');
      expect(mocked.formatJson).toHaveBeenCalledWith({
        language: 'json',
        text: '{"a":1}',
        indent: undefined,
        nest: true,
        sortKeys: undefined,
      });
      expect(mocked.minifyJson).toHaveBeenCalledWith({ language: 'json', text: '{\n  "a": 1\n}' });
      expect(mocked.sortText).toHaveBeenCalledWith('json', '{"b":2,"a":1}', expect.objectContaining({}));
      expect(mocked.convertJson).toHaveBeenCalledWith({ sourceLanguage: 'json', targetFormat: 'yaml', text: '{"a":1}', indent: undefined });
      expect(mocked.runYqText).toHaveBeenCalledWith(
        'json',
        '{"items":[{"name":"Alice"}]}',
        '.items[0]',
        expect.objectContaining({ nest: true }),
      );
    });

    it('decodes textBytes payloads before formatting', async () => {
      mocked.formatJson.mockImplementation(async () => ({ text: '{\n  "a": 1\n}', cursor: 0 }));

      const res = await send({
        id: 11,
        type: 'format',
        language: 'json',
        textBytes: new TextEncoder().encode('{"a":1}').buffer,
      } as any);

      expect(res.ok).toBe(true);
      expect(res.data).toBe('{\n  "a": 1\n}');
      expect(mocked.formatJson).toHaveBeenCalledWith({
        language: 'json',
        text: '{"a":1}',
        indent: undefined,
        nest: false,
        sortKeys: undefined,
      });
    });
  });

  describe('parseToTree', () => {
    it('returns parsed tree', async () => {
      mocked.parseToTree.mockImplementation(async () => ({
        kind: TreeKind.MAPPING,
        semType: SemType.MAP,
        tag: '',
        value: '',
        children: [
          { kind: TreeKind.SCALAR, semType: SemType.STR, tag: '', value: 'a', children: [] },
          { kind: TreeKind.SCALAR, semType: SemType.INT, tag: '', value: '1', children: [] },
        ],
      }));

      const res = await send({ id: 20, type: 'parseToTree', language: 'json', text: '{"a":1}', nest: false });

      expect(res.ok).toBe(true);
      expect(res.data).toHaveProperty('kind');
      expect(res.data).toHaveProperty('children');
    });
  });


  describe('treePath', () => {
    it('显式 snapshot 的 treePath 会查询 resolvePath', async () => {
      mocked.querySnapshot.mockResolvedValueOnce({
        status: 'ready',
        data: { anchors: [{ path: '$.a', spanStart: 0, spanEnd: 0 }] },
      });

      const res = await send({
        id: 681,
        type: 'treePath',
        documentKey: 'stale-language-key',
        snapshotId: 681,
        language: 'toml',
        text: '{"a":1}',
        row: 0,
        column: 0,
        nest: true,
      });

      expect(res.ok).toBe(true);
      expect(res.data).toEqual([{ tag: 0, key: 'a', index: 0 }]);
      expect(mocked.querySnapshot).toHaveBeenCalledWith(
        expect.objectContaining({
          documentKey: 'stale-language-key',
          snapshotId: 681,
          queryKind: 'resolvePath',
          spanStart: 0,
          spanEnd: 0,
        }),
      );
    });

    it('TOML treePath 通过 snapshot query 返回路径', async () => {
      mocked.querySnapshot.mockResolvedValueOnce({
        status: 'ready',
        data: { anchors: [{ path: '$.a', spanStart: 0, spanEnd: 0 }] },
      });

      const res = await send({
        id: 686,
        type: 'treePath',
        documentKey: 'toml-tree-path-disabled-key',
        snapshotId: 686,
        language: 'toml',
        text: 'a = 1',
        row: 0,
        column: 0,
        nest: true,
      });

      expect(res.ok).toBe(true);
      expect(res.data).toEqual([{ tag: 0, key: 'a', index: 0 }]);
      expect(mocked.querySnapshot).toHaveBeenCalledWith(
        expect.objectContaining({
          documentKey: 'toml-tree-path-disabled-key',
          snapshotId: 686,
          queryKind: 'resolvePath',
          spanStart: 0,
          spanEnd: 0,
        }),
      );
    });

    it('treePath 不再依赖 worker 侧格式门禁，只消费 query 结果', async () => {
      mocked.querySnapshot.mockResolvedValueOnce({
        status: 'ready',
        data: { anchors: [{ path: '$.a', spanStart: 0, spanEnd: 0 }] },
      });

      const res = await send({
        id: 682,
        type: 'treePath',
        documentKey: 'json-array-as-toml-key',
        snapshotId: 682,
        language: 'toml',
        text: '[{"a":1}]',
        row: 0,
        column: 0,
        nest: true,
      });

      expect(res.ok).toBe(true);
      expect(res.data).toEqual([{ tag: 0, key: 'a', index: 0 }]);
      expect(mocked.querySnapshot).toHaveBeenCalledWith(
        expect.objectContaining({
          documentKey: 'json-array-as-toml-key',
          snapshotId: 682,
          queryKind: 'resolvePath',
          spanStart: 0,
          spanEnd: 0,
        }),
      );
    });

    it('已有 diagnostics 时 treePath 仍可通过 snapshot query 返回结构化路径', async () => {
      mocked.querySnapshot.mockResolvedValueOnce({
        status: 'ready',
        data: { anchors: [{ path: '$.a', spanStart: 0, spanEnd: 0 }] },
      });

      const res = await send({
        id: 684,
        type: 'treePath',
        documentKey: 'diagnostic-language-key',
        snapshotId: 684,
        language: 'toml',
        text: 'a =',
        row: 0,
        column: 0,
        nest: true,
      });

      expect(res.ok).toBe(true);
      expect(res.data).toEqual([{ tag: 0, key: 'a', index: 0 }]);
      expect(mocked.querySnapshot).toHaveBeenCalledWith(
        expect.objectContaining({
          documentKey: 'diagnostic-language-key',
          snapshotId: 684,
          queryKind: 'resolvePath',
          spanStart: 0,
          spanEnd: 0,
        }),
      );
    });

    it('normalizes path segments with string values', async () => {
      mocked.querySnapshot.mockResolvedValueOnce({
        status: 'ready',
        data: { anchors: [{ path: '$.a', spanStart: 0, spanEnd: 0 }] },
      });

      const res = await send({
        id: 70,
        type: 'treePath',
        documentKey: 'path-key',
        snapshotId: 12,
        language: 'json',
        text: '{"a":1}',
        row: 0,
        column: 0,
        nest: true,
      });

      expect(res.ok).toBe(true);
      expect(res.data[0]).toMatchObject({ tag: 0, key: 'a', index: 0 });
    });
  });

  describe('parseValueToTree', () => {
    it('clones wasm tree node values into plain strings', async () => {
      mocked.parseValueToTreeJson.mockImplementation(async () => ({
        kind: TreeKind.SCALAR,
        semType: SemType.STR,
        tag: { toString: () => 'tag' },
        value: { toString: () => 'value' },
        children: [],
      }));

      const res = await send({
        id: 71,
        type: 'parseValueToTree',
        language: 'json',
        text: '"value"',
        nest: true,
      });

      expect(res.ok).toBe(true);
      expect(res.data).toMatchObject({ tag: 'tag', value: 'value' });
    });

    it('converts raw values into plain tree nodes', async () => {
      const res = await send({
        id: 73,
        type: 'valueToTreeNode',
        value: { enabled: true, count: 2 },
      });

      expect(res.ok).toBe(true);
      expect(res.data.kind).toBe(TreeKind.MAPPING);
      expect(Array.isArray(res.data.children)).toBe(true);
    });
  });

  describe('parseValueForPath', () => {
    it('forwards documentKey to core wasm api', async () => {
      mocked.parseValueForPath.mockResolvedValue({
        kind: TreeKind.SCALAR,
        semType: SemType.STR,
        tag: 'str',
        value: 'a',
        children: [],
      });

      const res = await send({
        id: 731,
        type: 'parseValueForPath',
        language: 'json',
        documentKey: 'doc-analysis-key',
        text: '{"enabled":true}',
        path: [{ tag: 0, key: 'enabled', index: 0 }],
        rawEdit: 'a',
        preferKey: false,
        nest: true,
      });

      expect(res.ok).toBe(true);
      expect(mocked.parseValueForPath).toHaveBeenCalledWith(
        'json',
        'doc-analysis-key',
        '{"enabled":true}',
        [{ tag: 0, key: 'enabled', index: 0 }],
        'a',
        false,
        { nest: true },
      );
    });
  });
  describe('applyValueEditCanonical', () => {
    it('returns canonical text, tree, and value', async () => {
      mocked.applyValueEditCanonical.mockResolvedValue({
        text: '{"name":"Bob"}',
        tree: {
          kind: TreeKind.MAPPING,
          semType: SemType.MAP,
          tag: '',
          value: '',
          children: [
            { kind: TreeKind.SCALAR, semType: SemType.STR, tag: '', value: 'name', children: [] },
            { kind: TreeKind.SCALAR, semType: SemType.STR, tag: '', value: 'Bob', children: [] },
          ],
        },
        value: { name: 'Bob' },
      });
      const res = await send({
        id: 74,
        type: 'applyValueEditCanonical',
        language: 'json',
        text: '{"name":"Alice"}',
        path: [{ tag: 0, key: 'name', index: 0 }],
        preferKey: false,
        value: { kind: TreeKind.SCALAR, semType: SemType.STR, tag: '', value: 'Bob', children: [] },
        nest: true,
      });
      expect(res.ok).toBe(true);
      expect(res.data).toEqual({
        text: '{"name":"Bob"}',
        tree: expect.objectContaining({ kind: TreeKind.MAPPING }),
        value: { name: 'Bob' },
      });
      expect(mocked.parseToTree).not.toHaveBeenCalled();
      expect(mocked.applyValueEditCanonical).toHaveBeenCalledWith(
        'json',
        '{"name":"Alice"}',
        [{ tag: 0, key: 'name', index: 0 }],
        false,
        'Bob',
      );
    });
  });


  describe('planGraphValueEdit', () => {

    it('does not call compat replace when snapshot-bound planner returns edits for supported languages', async () => {
      const languages = ['json', 'yaml', 'toml', 'csv', 'python', 'javascript'];

      for (const [index, language] of languages.entries()) {
        mocked.planGraphValueEdit.mockResolvedValueOnce({
          status: 'ready',
          data: {
            mode: 'edits',
            edits: [
              {
                startByte: 1,
                oldEndByte: 4,
                newEndByte: 4,
                text: 'new',
              },
            ],
            reason: null,
          },
        });

        const res = await send({
          id: 800 + index,
          type: 'planGraphValueEdit',
          documentKey: `${language}-doc`,
          snapshotId: 17,
          language,
          text: 'old',
          path: [{ tag: 0, key: 'name', index: 0 }],
          preferKey: false,
          value: scalarNode('new'),
          nest: true,
        });

        expect(res.ok).toBe(true);
        expect(res.data.mode).toBe('edits');
        expect(res.data.edits).toHaveLength(1);
      }

    });

    it('returns snapshotNotReady when snapshot-bound planning is unavailable', async () => {
      mocked.planGraphValueEdit.mockResolvedValue({ status: 'snapshotNotReady' });
      const res = await send({
        id: 741,
        type: 'planGraphValueEdit',
        snapshotId: null,
        language: 'json',
        text: 'abXXcdYYef',
        path: [{ tag: 0, key: 'name', index: 0 }],
        preferKey: false,
        value: scalarNode('patched'),
        nest: true,
      });
      expect(res.ok).toBe(true);
      expect(res.data).toEqual({
        mode: 'snapshotNotReady',
      });
      expect(mocked.parseToTree).not.toHaveBeenCalled();
      expect(mocked.applyValueEditCanonical).not.toHaveBeenCalled();
    });
  });

  describe('semanticTokens', () => {
    it('returns semantic tokens legend', async () => {
      const res = await send({ id: 39, type: 'semanticTokensLegend' });

      expect(res.ok).toBe(true);
      expect(res.data).toEqual(mocked.TOKEN_TYPES);
      expect(res.data).toEqual([...mocked.TREE_NODE_TOKEN_TYPES, ...mocked.AUXILIARY_TOKEN_TYPES]);
    });
  });

  describe('diagnostics', () => {
    it('returns diagnostics array', async () => {
      mocked.getDiagnostics.mockImplementation(async () => [
        { startLineNumber: 1, startColumn: 1, endLineNumber: 1, endColumn: 2, kind: 1 },
      ]);

      const res = await send({
        id: 41,
        type: 'diagnostics',
        documentKey: 'test-key',
        language: 'json',
        text: '{"a":1}',
      });

      expect(res.ok).toBe(true);
      expect(Array.isArray(res.data)).toBe(true);
      expect(mocked.getDiagnostics).toHaveBeenCalledWith('json', '{"a":1}');
    });
  });


  describe('document analysis cache', () => {
    it('does not expose legacy stored document analysis reads', async () => {
      const res = await send({
        id: 52,
        type: 'getStoredDocumentAnalysis',
        documentKey: 'doc-analysis',
        language: 'json',
      } as any);

      expect(res.ok).toBe(false);
      expect(res.error).toContain('Unhandled worker message type: getStoredDocumentAnalysis');
      expect(mocked.getStoredDocumentAnalysis).not.toHaveBeenCalled();
    });
  });



  describe('dispose and queue behavior', () => {
    it('resets state during dispose', async () => {
      await send({ id: 79, type: 'init' } as any);

      const disposed = await send({ id: 80, type: 'dispose' } as any);

      expect(disposed).toEqual({ id: 80, ok: true, data: true });
    });

    it('warns for ready-state failures and still posts ok:false', async () => {
      const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});
      const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

      const disposed = await send({ id: 81, type: 'dispose' } as any);
      expect(disposed).toEqual({ id: 81, ok: true, data: true });

      const afterDispose = await send({
        id: 82,
        type: 'diagnostics',
        language: 'json',
        text: '{"a":1}',
      } as any);
      expect(afterDispose.ok).toBe(false);
      expect(afterDispose.error).toContain('WASM not initialized');
      expect(warnSpy).toHaveBeenCalledWith('[worker] response', expect.objectContaining({ id: 82, ok: false }));
      expect(errorSpy).not.toHaveBeenCalled();
      warnSpy.mockRestore();
      errorSpy.mockRestore();
    });

    it('logs uncaught handler failures as errors', async () => {
      const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
      mocked.getDiagnostics.mockRejectedValueOnce(new Error('diagnostics boom'));

      const res = await send({
        id: 83,
        type: 'diagnostics',
        documentKey: 'test-key',
        language: 'json',
        text: '{"a":1}',
      } as any);
      expect(res.ok).toBe(false);
      expect(res.error).toContain('diagnostics boom');
      expect(errorSpy).toHaveBeenCalledWith(
        '[worker] message failed',
        expect.objectContaining({ id: 83, type: 'diagnostics' }),
      );
      errorSpy.mockRestore();
    });

    it('returns an error for unknown message types without breaking later requests', async () => {
      await send({ id: 84, type: 'init' } as any);
      const unknown = await send({ id: 85, type: 'unknown-message' } as any);
      expect(unknown).toMatchObject({ id: 85, ok: false });

      const res = await send({
        id: 86,
        type: 'diagnostics',
        documentKey: 'test-key',
        language: 'json',
        text: '{"a":1}',
      } as any);
      expect(res.ok).toBe(true);
    });
  });

  describe('document job transport', () => {

    it('forwards startDocumentJob without synthesizing analysis payloads', async () => {
      mocked.startDocumentJob.mockResolvedValueOnce({
        jobHandle: 1,
        batch: {
          requestSeq: 1,
          events: [],
          terminal: null,
        },
      });

      const res = await send({
        id: 801,
        type: 'startDocumentJob',
        documentKey: 'doc-job',
        language: 'json',
        nest: true,
        outputGraph: true,
        outputAnalysis: true,
      });

      expect(res).toMatchObject({
        id: 801,
        ok: true,
        data: {
          jobHandle: 1,
          batch: {
            requestSeq: 1,
            events: [],
            terminal: null,
          },
        },
      });
      expect(mocked.startDocumentJob).toHaveBeenCalledWith({
        documentKey: 'doc-job',
        language: 'json',
        nest: true,
        settings: {
          parser: { enableNest: true, nestMaxDepth: 8 },
          formatting: {
            indent: 2,
            smart: true,
            formatSourceOnClose: true,
            maxLineLength: 100,
            maxInlineComplexity: 1,
            maxArrayInlineItems: 6,
            alignObjectArrays: true,
          },
        },
        outputGraph: true,
        outputAnalysis: true,
        builderConfig: undefined,
        baseSnapshotId: undefined,
        edits: undefined,
      });
      expect(mocked.getStoredDocumentAnalysis).not.toHaveBeenCalled();
    });

    it('forwards cancelDocumentJob as its facade input object', async () => {
      const batch = {
        requestSeq: 7,
        events: [],
        terminal: { type: 'cancelled' },
      };
      mocked.cancelDocumentJob.mockResolvedValueOnce(batch);

      const res = await send({
        id: 804,
        type: 'cancelDocumentJob',
        jobHandle: 7,
      });

      expect(res).toEqual({
        id: 804,
        ok: true,
        data: true,
      });
      expect(mocked.cancelDocumentJob).toHaveBeenCalledWith({
        jobHandle: 7,
      });
    });

    it('forwards querySnapshot and buildHoverSubgraphProjection to the core wasm facade', async () => {
      mocked.querySnapshot.mockResolvedValueOnce({
        status: 'ready',
        data: {
          anchors: [
            {
              snapshotId: 9,
              path: '[{"tag":0,"key":"items","index":0}]',
              spanStart: 12,
              spanEnd: 18,
              target: 'path',
            },
          ],
        },
      });
      const projectionData = { status: 'ready', data: projectionDelta(false, projectionGraphData()) };
      mocked.buildHoverSubgraphProjection.mockResolvedValueOnce(projectionData);

      const query = await send({
        id: 802,
        type: 'querySnapshot',
        documentKey: 'doc-job',
        snapshotId: 9,
        queryKind: 1,
        pathPattern: '[{"tag":0,"key":"items","index":0}]',
      });
      const projection = await send({
        id: 803,
        type: 'buildHoverSubgraphProjection',
        documentKey: 'doc-job',
        snapshotId: 9,
        path: '[{"tag":0,"key":"items","index":0}]',
      });

      expect(query).toEqual({
        id: 802,
        ok: true,
        data: {
          status: 'ready',
          data: {
            anchors: [
              {
                snapshotId: 9,
                path: '[{"tag":0,"key":"items","index":0}]',
                spanStart: 12,
                spanEnd: 18,
                target: 'path',
              },
            ],
          },
        },
      });
      expect(projection).toEqual({
        id: 803,
        ok: true,
        data: projectionData,
      });
      expect(mocked.querySnapshot).toHaveBeenCalledWith({
        documentKey: 'doc-job',
        snapshotId: 9,
        queryKind: 1,
        pathPattern: '[{"tag":0,"key":"items","index":0}]',
        spanStart: undefined,
        spanEnd: undefined,
        target: undefined,
      });
      expect(mocked.buildHoverSubgraphProjection).toHaveBeenCalledWith({
        documentKey: 'doc-job',
        snapshotId: 9,
        path: '[{"tag":0,"key":"items","index":0}]',
      });
    });
  });

});
