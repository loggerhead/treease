import { beforeEach, describe, expect, it, vi } from 'vitest';

const mockedPkg = vi.hoisted(() => ({
  default: vi.fn(async () => undefined),
  initSync: vi.fn(),
  init_wasm: vi.fn(),
  start_document_job: vi.fn(),
  build_hover_subgraph_projection: vi.fn(),
  format_text: vi.fn(),
  compare_structured_wasm: vi.fn(),
}));

vi.mock('@core-wasm/pkg', () => mockedPkg);

describe('core wasm facade split', () => {
  beforeEach(() => {
    vi.resetModules();
    mockedPkg.default.mockClear();
    mockedPkg.initSync.mockClear();
    mockedPkg.init_wasm.mockClear();
    mockedPkg.start_document_job.mockResolvedValue({
      jobHandle: 7,
      batch: { requestSeq: 1, events: [], terminal: null },
    });
    mockedPkg.build_hover_subgraph_projection.mockResolvedValue({
      status: 'ready',
      data: { clear: true, graphData: null },
    });
    mockedPkg.format_text.mockResolvedValue('{"a":1}\n');
    mockedPkg.compare_structured_wasm.mockResolvedValue(true);
  });

  it('keeps the barrel entrypoint while exposing document and compat modules', async () => {
    const documentApi = await import('../../../../../packages/core/wasm/monaco/document-api');
    const compatApi = await import('../../../../../packages/core/wasm/monaco/compat-api');
    const entrypoint = await import('@core-wasm/index');

    expect(entrypoint.startDocumentJob).toBe(documentApi.startDocumentJob);
    expect(entrypoint.buildHoverSubgraphProjection).toBe(documentApi.buildHoverSubgraphProjection);
    expect(entrypoint.formatText).toBe(compatApi.formatText);
    expect(entrypoint.compareStructured).toBe(compatApi.compareStructured);

    const settings = {
      parser: { enableNest: false, nestMaxDepth: 8 },
      formatting: {
        indent: 2,
        smart: true,
        formatSourceOnClose: true,
        maxLineLength: 100,
        maxInlineComplexity: 1,
        maxArrayInlineItems: 6,
        alignObjectArrays: true,
      },
    };

    await documentApi.startDocumentJob({
      documentKey: 'doc-1',
      language: 'json',
      nest: false,
      settings,
      outputGraph: true,
      outputAnalysis: true,
    });
    expect(mockedPkg.default).toHaveBeenCalledTimes(1);
    expect(mockedPkg.init_wasm).toHaveBeenCalledTimes(1);
    expect(mockedPkg.start_document_job).toHaveBeenCalledWith({
      documentKey: 'doc-1',
      language: 'json',
      text: '',
      nest: false,
      settings,
      outputGraph: true,
      outputAnalysis: true,
      builderConfig: null,
      baseSnapshotId: null,
      edits: [],
    });

    const hover = await documentApi.buildHoverSubgraphProjection({
      documentKey: 'doc-1',
      snapshotId: 9 as any,
      path: '$',
    });
    expect(mockedPkg.build_hover_subgraph_projection).toHaveBeenCalledWith({
      documentKey: 'doc-1',
      snapshotId: 9,
      path: '$',
    });
    expect(hover).toEqual({
      status: 'ready',
      data: { clear: true, graphData: null },
    });

    await compatApi.formatText('json', '{"a":1}', { indent: 2, sortKeys: true });
    expect(mockedPkg.format_text).toHaveBeenCalledWith({
      language: 'json',
      text: '{"a":1}',
      indent: 2,
      sortKeys: true,
    });

    await compatApi.compareStructured('json', '{"a":1}', '{"a":1}');
    expect(mockedPkg.compare_structured_wasm).toHaveBeenCalledWith({
      language: 'json',
      left: '{"a":1}',
      right: '{"a":1}',
    });
  });
});
