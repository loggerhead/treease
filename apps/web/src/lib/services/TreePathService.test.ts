import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('../wasm/wasm-worker-singleton', () => ({
  callSharedWasmWorker: vi.fn(),
}));


import { callSharedWasmWorker } from '../wasm/wasm-worker-singleton';
import {
  getTreePathAtPosition,
  resolvePathAnchorSafe,
  resolvePathSelectionRangeSafe,
  resolvePathSpan,
  resolveTreePath,
  resolveTreePathFromText,
  toByteColumn,
} from './TreePathService';

describe('TreePathService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });
  const readyResult = (anchors: Array<Record<string, unknown>>) => ({
    status: 'ready' as const,
    data: { anchors },
  })


  it('toByteColumn counts utf-8 bytes correctly', () => {
    expect(toByteColumn('a你b', 1)).toBe(1);
    expect(toByteColumn('a你b', 2)).toBe(4);
    expect(toByteColumn('a你b', 3)).toBe(5);
  });

  it('uses the full line byte length when the cursor is past line end', async () => {
    const model = {
      getLineContent: vi.fn().mockReturnValue('a你'),
      getValue: vi.fn().mockReturnValue('a你'),
    } as any;
    const callMock = vi.mocked(callSharedWasmWorker as any);
    callMock.mockResolvedValueOnce(readyResult([{ path: '$["你"]', spanStart: 0, spanEnd: 0, target: 'path', snapshotId: 7 }]));

    const path = await resolveTreePath(
      model,
      { lineNumber: 1, column: 99 } as any,
      'vitest://tree-path/clamp',
      'json' as any,
      true,
      7,
    );

    expect(path).toHaveLength(1);
    expect(callMock).toHaveBeenCalledWith('querySnapshot', expect.objectContaining({ snapshotId: 7, spanStart: 4, spanEnd: 4 }));
  });

  it('resolveTreePath sends a single normalized byte column for character positions', async () => {
    const model = {
      getLineContent: vi.fn().mockReturnValue('a你b'),
      getValue: vi.fn().mockReturnValue('a你b\n'),
    } as any;
    const callMock = vi.mocked(callSharedWasmWorker as any);
    callMock.mockResolvedValueOnce(readyResult([{ path: '$.value', spanStart: 4, spanEnd: 4, target: 'path', snapshotId: 7 }]));

    const path = await resolveTreePath(
      model,
      { lineNumber: 1, column: 3 } as any,
      'vitest://tree-path/char-column',
      'json' as any,
      true,
      7,
    );

    expect(path).toHaveLength(1);
    expect(callMock).toHaveBeenCalledTimes(1);
    expect(callMock.mock.calls[0][0]).toBe('querySnapshot');
    expect(callMock.mock.calls[0][1]).toMatchObject({ snapshotId: 7, spanStart: 4, spanEnd: 4 });
  });

  it('resolveTreePath keeps whitespace positions and leaves fallback to core', async () => {
    const model = {
      getLineContent: vi.fn().mockReturnValue('   abc'),
      getValue: vi.fn().mockReturnValue('   abc\n'),
    } as any;
    const callMock = vi.mocked(callSharedWasmWorker as any);
    callMock.mockResolvedValueOnce(readyResult([]));

    const path = await resolveTreePath(
      model,
      { lineNumber: 1, column: 1 } as any,
      'vitest://tree-path/whitespace',
      'json' as any,
      true,
      7,
    );

    expect(path).toEqual([]);
    expect(callMock).toHaveBeenCalledTimes(1);
    expect(callMock.mock.calls[0][1]).toMatchObject({ snapshotId: 7, spanStart: 0, spanEnd: 0 });
  });

  it('getTreePathAtPosition returns empty array on failure', async () => {
    const model = {
      getLineContent: vi.fn().mockReturnValue('{"a":1}'),
      getValue: vi.fn().mockReturnValue('{"a":1}'),
    } as any;

    const callMock = vi.mocked(callSharedWasmWorker as any);
    callMock.mockRejectedValueOnce(new Error('boom'));

    const path = await getTreePathAtPosition(
      model,
      { lineNumber: 1, column: 2 } as any,
      'vitest://tree-path/error',
      'json' as any,
      true,
      7,
    );

    expect(path).toEqual([]);
  });

  it('resolvePathSpan forwards query target to the worker', async () => {
    const model = {
      getValue: vi.fn().mockReturnValue('flags:\n  - true\n'),
    } as any;
    const callMock = vi.mocked(callSharedWasmWorker as any);
    callMock.mockResolvedValueOnce(readyResult([{ path: '$.flags', spanStart: 11, spanEnd: 15, target: 'span', snapshotId: 7 }]));

    await resolvePathSpan(
      model,
      [{ tag: 0, key: 'flags' as any, index: 0 } as any],
      'vitest://path-span/nest',
      'yaml' as any,
      'value',
      true,
      7,
    );

    expect(callMock).toHaveBeenCalledWith(
      'querySnapshot',
      expect.objectContaining({
        snapshotId: 7,
        queryKind: 'findAnchors',
        pathPattern: '$.flags',
        target: 'value',
      }),
    );
  });

  it('resolvePathAnchorSafe retries with the fallback target in order', async () => {
    const model = {
      getValue: vi.fn().mockReturnValue('{"a":1}'),
    } as any;
    const callMock = vi.mocked(callSharedWasmWorker as any);
    callMock
      .mockResolvedValueOnce(readyResult([]))
      .mockResolvedValueOnce(readyResult([{ path: '$.a', spanStart: 5, spanEnd: 6, target: 'span', snapshotId: 7 }]));

    const anchor = await resolvePathAnchorSafe(
      model,
      [{ tag: 0, key: 'a', index: 0 } as any],
      'vitest://path-anchor/target-fallback',
      'json' as any,
      'key',
      true,
      7,
    );

    expect(anchor).toEqual({ row: 0, column: 5 });
    expect(callMock).toHaveBeenNthCalledWith(
      1,
      'querySnapshot',
      expect.objectContaining({ pathPattern: '$.a', target: 'key' }),
    );
    expect(callMock).toHaveBeenNthCalledWith(
      2,
      'querySnapshot',
      expect.objectContaining({ pathPattern: '$.a', target: 'value' }),
    );
  });

  it('resolvePathSpan returns coordinates when valid', async () => {
    const model = {
      getValue: vi.fn().mockReturnValue('{"a":1}'),
    } as any;
    const callMock = vi.mocked(callSharedWasmWorker as any);
    callMock.mockResolvedValueOnce(readyResult([{ path: '$.a', spanStart: 5, spanEnd: 6, target: 'span', snapshotId: 7 }]));

    const span = await resolvePathSpan(
      model,
      [{ tag: 1, key: 'a', index: 0 } as any],
      'vitest://path-span/ok',
      'json' as any,
      'value',
      true,
      7,
    );

    expect(span).toEqual({ startByte: 5, endByte: 6, row: 0, column: 5 });
  });

  it('resolvePathSpan returns null for invalid span', async () => {
    const model = {
      getValue: vi.fn().mockReturnValue('{"a":1}'),
    } as any;
    const callMock = vi.mocked(callSharedWasmWorker as any);
    callMock.mockResolvedValueOnce(readyResult([{ path: '$.a', spanStart: 8, spanEnd: 6, target: 'span', snapshotId: 7 }]));

    const span = await resolvePathSpan(
      model,
      [{ tag: 1, key: 'a', index: 0 } as any],
      'vitest://path-span/invalid',
      'json' as any,
      'value',
      true,
      7,
    );

    expect(span).toBeNull();
  });

  it('resolvePathAnchorSafe uses the first valid span candidate', async () => {
    const model = {
      getValue: vi.fn().mockReturnValue('{"a":1}'),
    } as any;
    const callMock = vi.mocked(callSharedWasmWorker as any);
    callMock.mockResolvedValueOnce(readyResult([{ path: '$.a', spanStart: 5, spanEnd: 6, target: 'span', snapshotId: 7 }]));

    const anchor = await resolvePathAnchorSafe(
      model,
      [{ tag: 1, key: 'a', index: 0 } as any],
      'vitest://path-anchor/span-first',
      'json' as any,
      'value',
      true,
      7,
    );

    expect(anchor).toEqual({ row: 0, column: 5 });
    expect(callMock).toHaveBeenCalledTimes(1);
    expect(callMock).toHaveBeenCalledWith('querySnapshot', expect.objectContaining({ queryKind: 'findAnchors' }));
  });

  it('resolvePathAnchorSafe returns null when no span candidate is valid', async () => {
    const model = {
      getValue: vi.fn().mockReturnValue('{"a":1}'),
    } as any;
    const callMock = vi.mocked(callSharedWasmWorker as any);
    callMock
      .mockResolvedValueOnce(readyResult([{ path: '$.a', spanStart: 8, spanEnd: 6, target: 'span', snapshotId: 7 }]))
      .mockResolvedValueOnce(readyResult([{ path: '$.a', spanStart: 8, spanEnd: 6, target: 'span', snapshotId: 7 }]));

    const anchor = await resolvePathAnchorSafe(
      model,
      [{ tag: 1, key: 'a', index: 0 } as any],
      'vitest://path-anchor/fallback',
      'json' as any,
      'value',
      true,
      7,
    );

    expect(anchor).toBeNull();
    expect(callMock.mock.calls[0][0]).toBe('querySnapshot');
    expect(callMock.mock.calls[1][0]).toBe('querySnapshot');
  });

  it('resolvePathSelectionRangeSafe maps utf-8 byte span to monaco positions', async () => {
    const text = '{"message":"你好"}';
    const startByte = new TextEncoder().encode('{"message":"').length;
    const endByte = new TextEncoder().encode('{"message":"你好').length;
    const model = {
      getValue: vi.fn().mockReturnValue(text),
      getPositionAt: vi.fn((offset: number) => ({ lineNumber: 1, column: offset + 1 })),
    } as any;
    const callMock = vi.mocked(callSharedWasmWorker as any);
    callMock.mockResolvedValueOnce(readyResult([{ path: '$.message', spanStart: startByte, spanEnd: endByte, target: 'span', snapshotId: 7 }]));

    const range = await resolvePathSelectionRangeSafe(
      model,
      [{ tag: 1, key: 'message', index: 0 } as any],
      'vitest://path-selection/utf8',
      'json' as any,
      'value',
      true,
      7,
    );

    expect(range).toEqual({
      start: { lineNumber: 1, column: 13 },
      end: { lineNumber: 1, column: 15 },
    });
    expect(model.getPositionAt).toHaveBeenNthCalledWith(1, 12);
    expect(model.getPositionAt).toHaveBeenNthCalledWith(2, 14);
  });

  it('resolveTreePathFromText sends a single normalized byte column in auto mode', async () => {
    const callMock = vi.mocked(callSharedWasmWorker as any);
    callMock.mockResolvedValueOnce(readyResult([{ path: '$.value', spanStart: 4, spanEnd: 4, target: 'path', snapshotId: 7 }]));

    const path = await resolveTreePathFromText('a你b', 0, 2, 'vitest://tree-path/auto', 'javascript' as any, true, 'auto', 7);

    expect(path).toHaveLength(1);
    expect(callMock).toHaveBeenCalledTimes(1);
    expect(callMock.mock.calls[0][1]).toMatchObject({ snapshotId: 7, spanStart: 4, spanEnd: 4 });
  });

  it('resolveTreePathFromText leaves punctuation fallback to core', async () => {
    const line = 'user = { name: "Ada" }';
    const callMock = vi.mocked(callSharedWasmWorker as any);
    callMock.mockResolvedValueOnce(readyResult([{ path: '$.name', spanStart: 14, spanEnd: 14, target: 'path', snapshotId: 7 }]));

    const path = await resolveTreePathFromText(line, 0, 14, 'vitest://tree-path/auto-punct', 'javascript' as any, true, 'auto', 7);

    expect(path).toHaveLength(1);
    expect(callMock).toHaveBeenCalledTimes(1);
    expect(callMock.mock.calls[0][1]).toMatchObject({ snapshotId: 7, spanStart: 14, spanEnd: 14 });
  });
});
