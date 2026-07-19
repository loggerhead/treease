import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocked = vi.hoisted(() => ({
  initWasm: vi.fn(),
  getChunkSizeConfig: vi.fn(),
  guessLanguage: vi.fn(),
  handleDiagnostics: vi.fn(),
  clearWorkerRuntimeState: vi.fn(),
  createWorkerRuntimeState: vi.fn(),
}));

vi.mock('@core-wasm/index', () => ({
  initWasm: mocked.initWasm,
  TOKEN_TYPES: ['map', 'key'],
}));

vi.mock('@core-wasm/pkg', () => ({
  get_chunk_size_config: mocked.getChunkSizeConfig,
}));

vi.mock('@core-wasm/guess-language', () => ({
  guessLanguage: mocked.guessLanguage,
}));

vi.mock('./document-parse', () => ({
  handleDiagnostics: mocked.handleDiagnostics,
  handleFindJsonBlockAtPosition: vi.fn(),
  handleParseAndStore: vi.fn(),
  handleParseToTree: vi.fn(),
  handleParseValueToTree: vi.fn(),
}));

vi.mock('./document-value-edit', () => ({
  handleApplyValueEditCanonical: vi.fn(),
  handleParseValueForPath: vi.fn(),
  handlePlanGraphValueEdit: vi.fn(),
  handleValueToTreeNode: vi.fn(),
}));

vi.mock('./document-compare', () => ({ handleCompare: vi.fn() }));
vi.mock('./document-transform', () => ({
  handleConvert: vi.fn(),
  handleFormat: vi.fn(),
  handleMinify: vi.fn(),
  handleCompact: vi.fn(),
  handleRunYq: vi.fn(),
  handleSort: vi.fn(),
}));
vi.mock('./graph-search', () => ({ handleGraphSearch: vi.fn() }));
vi.mock('./document-job', () => ({
  handleAdvanceDocumentJob: vi.fn(),
  handleBuildHoverSubgraphProjection: vi.fn(),
  handleCancelDocumentJob: vi.fn(),
  handleQuerySnapshot: vi.fn(),
  handleStartDocumentJob: vi.fn(),
}));
vi.mock('./tree-path', () => ({ handlePathSpan: vi.fn(), handleTreePath: vi.fn() }));
vi.mock('./worker-runtime-state', () => ({
  createWorkerRuntimeState: mocked.createWorkerRuntimeState,
  clearWorkerRuntimeState: mocked.clearWorkerRuntimeState,
}));

describe('Worker transport', () => {
  let posted: unknown[];
  let transport: { enqueue: (message: any) => void };

  async function waitForResponse(id: number): Promise<any> {
    await vi.waitFor(() => expect(posted.some((message: any) => message.id === id)).toBe(true));
    return posted.findLast((message: any) => message.id === id);
  }

  beforeEach(async () => {
    vi.resetModules();
    vi.clearAllMocks();
    posted = [];
    mocked.initWasm.mockResolvedValue(undefined);
    mocked.getChunkSizeConfig.mockReturnValue({ json: 1024 });
    mocked.createWorkerRuntimeState.mockReturnValue({
      searchIndexByDocumentKey: new Map(),
      graphStateService: { graphStateByDocumentKey: new Map() },
    });
    mocked.handleDiagnostics.mockResolvedValue([]);

    const { createWorkerTransport } = await import('./worker-transport');
    transport = createWorkerTransport({ postMessage: (message: unknown) => posted.push(message), onmessage: null });
  });

  it('uses one error path for the ready gate without invoking an operation', async () => {
    transport.enqueue({ id: 1, type: 'diagnostics', language: 'json', text: '{}' });

    await expect(waitForResponse(1)).resolves.toEqual({ id: 1, ok: false, error: 'WASM not initialized' });
    expect(mocked.handleDiagnostics).not.toHaveBeenCalled();
  });

  it('keeps request correlation at the transport Interface', async () => {
    transport.enqueue({ id: 2, type: 'init', wasmURL: 'memory://core.wasm' });
    await expect(waitForResponse(2)).resolves.toEqual({ id: 2, ok: true, data: { chunkSizeConfig: { json: 1024 } } });

    mocked.handleDiagnostics.mockResolvedValueOnce([{ message: 'ok' }]);
    transport.enqueue({ id: 3, type: 'diagnostics', language: 'json', text: '{}' });

    await expect(waitForResponse(3)).resolves.toEqual({ id: 3, ok: true, data: [{ message: 'ok' }] });
    expect(mocked.handleDiagnostics).toHaveBeenCalledWith({ id: 3, type: 'diagnostics', language: 'json', text: '{}' });
  });

  it('orders a normal request behind an in-flight init request', async () => {
    let releaseInit: (() => void) | null = null;
    mocked.initWasm.mockImplementationOnce(() => new Promise<void>((resolve) => { releaseInit = resolve; }));

    transport.enqueue({ id: 12, type: 'init', wasmURL: 'memory://core.wasm' });
    transport.enqueue({ id: 13, type: 'diagnostics', language: 'json', text: '{}' });
    await vi.waitFor(() => expect(mocked.initWasm).toHaveBeenCalledTimes(1));
    expect(mocked.handleDiagnostics).not.toHaveBeenCalled();

    releaseInit?.();
    await waitForResponse(13);

    expect(posted.filter((message: any) => message.id === 12 || message.id === 13).map((message: any) => message.id)).toEqual([12, 13]);
    expect(mocked.handleDiagnostics).toHaveBeenCalledTimes(1);
  });

  it('turns an operation exception into the matching error response', async () => {
    transport.enqueue({ id: 4, type: 'init', wasmURL: 'memory://core.wasm' });
    await waitForResponse(4);
    mocked.handleDiagnostics.mockRejectedValueOnce(new Error('diagnostics exploded'));

    transport.enqueue({ id: 5, type: 'diagnostics', language: 'json', text: '{}' });

    await expect(waitForResponse(5)).resolves.toEqual({ id: 5, ok: false, error: 'diagnostics exploded' });
  });

  it('serializes operations and preserves response ordering', async () => {
    transport.enqueue({ id: 6, type: 'init', wasmURL: 'memory://core.wasm' });
    await waitForResponse(6);

    let releaseFirst: (() => void) | null = null;
    mocked.handleDiagnostics
      .mockImplementationOnce(() => new Promise<void>((resolve) => { releaseFirst = resolve; }))
      .mockResolvedValueOnce([{ message: 'second' }]);

    transport.enqueue({ id: 7, type: 'diagnostics', language: 'json', text: '{"first":true}' });
    transport.enqueue({ id: 8, type: 'diagnostics', language: 'json', text: '{"second":true}' });
    await vi.waitFor(() => expect(mocked.handleDiagnostics).toHaveBeenCalledTimes(1));
    expect(posted.some((message: any) => message.id === 8)).toBe(false);

    releaseFirst?.();
    await waitForResponse(8);

    expect(posted.filter((message: any) => message.id === 7 || message.id === 8).map((message: any) => message.id)).toEqual([7, 8]);
  });

  it('clears runtime state on dispose and re-enables the ready gate', async () => {
    transport.enqueue({ id: 9, type: 'init', wasmURL: 'memory://core.wasm' });
    await waitForResponse(9);
    transport.enqueue({ id: 10, type: 'dispose' });

    await expect(waitForResponse(10)).resolves.toEqual({ id: 10, ok: true, data: true });
    expect(mocked.clearWorkerRuntimeState).toHaveBeenCalledTimes(1);

    transport.enqueue({ id: 11, type: 'diagnostics', language: 'json', text: '{}' });
    await expect(waitForResponse(11)).resolves.toEqual({ id: 11, ok: false, error: 'WASM not initialized' });
  });
});
