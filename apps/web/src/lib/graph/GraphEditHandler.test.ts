import { describe, it, expect, vi, beforeEach } from 'vitest';

/* Mock WASM worker */
vi.mock('../wasm/wasm-worker-singleton', () => ({
  callSharedWasmWorker: vi.fn(),
}));

import { commitTextEdit, type EditContext } from './GraphEditHandler';
import { callSharedWasmWorker } from '../wasm/wasm-worker-singleton';

const mockedCallWasm = vi.mocked(callSharedWasmWorker);

function keySeg(key: string) {
  return { tag: 0, key, index: 0 };
}
function indexSeg(index: number) {
  return { tag: 1, key: '', index };
}

describe('GraphEditHandler', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('returns null when currentData is null', async () => {
    const ctx: EditContext = { currentData: null, languageId: 'json', nest: false };
    const result = await commitTextEdit(ctx, {}, {}, null);
    expect(result).toBeNull();
  });

  it('returns null when activeEditCell is null', async () => {
    const ctx: EditContext = { currentData: { a: 1 }, languageId: 'json', nest: false };
    const result = await commitTextEdit(ctx, null, {}, null);
    expect(result).toBeNull();
  });

  it('returns null when activeEditTarget is null', async () => {
    const ctx: EditContext = { currentData: { a: 1 }, languageId: 'json', nest: false };
    const result = await commitTextEdit(ctx, { path: [] }, null, null);
    expect(result).toBeNull();
  });

  /* ─── key edit (rename) ─── */
  it('renames a key when editKind is key', async () => {
    const ctx: EditContext = { currentData: { oldKey: 'hello' }, languageId: 'json', nest: false };
    const cell = { path: [keySeg('oldKey')] };
    const target = { text: 'newKey' };
    const result = await commitTextEdit(ctx, cell, target, 'key');
    expect(result).not.toBeNull();
    expect(result!.preferKey).toBe(true);
    expect(result!.updated).toEqual({ newKey: 'hello' });
  });

  it('returns null when key rename does not change anything', async () => {
    const ctx: EditContext = { currentData: { same: 'hello' }, languageId: 'json', nest: false };
    const cell = { path: [keySeg('same')] };
    const target = { text: 'same' };
    const result = await commitTextEdit(ctx, cell, target, 'key');
    expect(result).toBeNull();
    expect(mockedCallWasm).not.toHaveBeenCalled();
  });

  it('rename handles nested object path', async () => {
    const ctx: EditContext = {
      currentData: { parent: { child: 42 } },
      languageId: 'json',
      nest: false,
    };
    const cell = { path: [keySeg('parent'), keySeg('child')] };
    const target = { text: 'renamed' };
    const result = await commitTextEdit(ctx, cell, target, 'key');
    expect(result!.updated).toEqual({ parent: { renamed: 42 } });
  });

  it('rename on empty path returns data as-is', async () => {
    const ctx: EditContext = { currentData: { a: 1 }, languageId: 'json', nest: false };
    const cell = { path: [] };
    const target = { text: 'x' };
    const result = await commitTextEdit(ctx, cell, target, 'key');
    expect(result!.updated).toEqual({ a: 1 });
  });

  /* ─── value edit ─── */
  it('sets value via WASM parse and updates the data structure', async () => {
    mockedCallWasm.mockResolvedValueOnce({ tree: { kind: 2, value: '42', children: [] }, value: 42 } as any);
    const ctx: EditContext = { currentData: { count: 0 }, languageId: 'json', nest: false };
    const cell = { path: [keySeg('count')] };
    const target = { text: '42' };
    const result = await commitTextEdit(ctx, cell, target, 'value');
    expect(result!.preferKey).toBe(false);
    expect(mockedCallWasm).toHaveBeenCalledWith('parseValueToData', expect.objectContaining({ text: '42' }));
    expect((result!.updated as any).count).toBe(42);
    expect(result!.nextValueNode).toEqual({ kind: 2, value: '42', children: [] });
  });

  it('returns null when value edit does not change anything', async () => {
    mockedCallWasm.mockResolvedValueOnce({ tree: { kind: 2, value: '1', children: [] }, value: 1 } as any);
    const ctx: EditContext = { currentData: { count: 1 }, languageId: 'json', nest: false };
    const cell = { path: [keySeg('count')] };
    const target = { text: '1' };
    const result = await commitTextEdit(ctx, cell, target, 'value');
    expect(result).toBeNull();
  });

  it('returns null when value parse throws', async () => {
    mockedCallWasm.mockRejectedValueOnce(new Error('parse error'));
    const ctx: EditContext = { currentData: { name: 'old' }, languageId: 'json', nest: false };
    const cell = { path: [keySeg('name')] };
    const target = { text: 'newRawText' };
    const result = await commitTextEdit(ctx, cell, target, 'value');
    expect(result).toBeNull();
  });

  it('sets value in array by index and verifies updated array', async () => {
    mockedCallWasm.mockResolvedValueOnce({ tree: { kind: 2, value: 'replaced', children: [] }, value: 'replaced' } as any);
    const ctx: EditContext = { currentData: ['a', 'b', 'c'], languageId: 'json', nest: false };
    const cell = { path: [indexSeg(1)] };
    const target = { text: 'replaced' };
    const result = await commitTextEdit(ctx, cell, target, null);
    expect(result).not.toBeNull();
    // Verify the array was updated at index 1
    expect(result!.updated).toEqual(['a', 'replaced', 'c']);
  });

  it('uses __graphCellKind fallback when activeEditKind is null', async () => {
    // NOTE: No mockResolvedValueOnce here — key rename doesn't call WASM
    const ctx: EditContext = { currentData: { x: 1 }, languageId: 'json', nest: false };
    const cell = { path: [keySeg('x')] };
    const target = { text: 'newKey', __graphCellKind: 'key' };
    const result = await commitTextEdit(ctx, cell, target, null);
    expect(result!.preferKey).toBe(true);
    expect(result!.updated).toEqual({ newKey: 1 });
    // WASM should NOT have been called for key rename
    expect(mockedCallWasm).not.toHaveBeenCalled();
  });

  it('value edit on nested path updates only the target leaf', async () => {
    mockedCallWasm.mockResolvedValueOnce({ tree: { kind: 2, value: '99', children: [] }, value: 99 } as any);
    const ctx: EditContext = {
      currentData: { root: { items: [10, 20, 30] } },
      languageId: 'json',
      nest: false,
    };
    const cell = { path: [keySeg('root'), keySeg('items'), indexSeg(2)] };
    const target = { text: '99' };
    const result = await commitTextEdit(ctx, cell, target, 'value');
    expect(result!.updated).toEqual({ root: { items: [10, 20, 99] } });
  });
  it('value edit accepts string key path segments', async () => {
    mockedCallWasm.mockResolvedValueOnce({
      tree: { kind: 2, value: 'row-0-updated', children: [] },
      value: 'row-0-updated',
    } as any);
    const ctx: EditContext = {
      currentData: { table_with_header: [{ id: 0, name: 'row-0', status: 'ready' }] },
      languageId: 'json',
      nest: false,
    };
    const cell = {
      path: [
        { tag: 0, key: 'table_with_header', index: 0 },
        indexSeg(0),
        { tag: 0, key: 'name', index: 0 },
      ],
    };
    const target = { text: 'row-0-updated' };

    const result = await commitTextEdit(ctx, cell, target, 'value');

    expect(result).not.toBeNull();
    expect(result!.updated).toEqual({
      table_with_header: [{ id: 0, name: 'row-0-updated', status: 'ready' }],
    });
  });
});
