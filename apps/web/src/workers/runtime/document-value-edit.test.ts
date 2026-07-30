import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocked = vi.hoisted(() => ({
  applyValueEditCanonical: vi.fn(),
  parseValueForPath: vi.fn(),
  planGraphValueEdit: vi.fn(),
  treeNodeToValue: vi.fn(),
  valueToTreeNode: vi.fn(),
  parseToTreeNode: vi.fn(),
}));

vi.mock('@core-wasm/index', () => ({
  applyValueEditCanonical: mocked.applyValueEditCanonical,
  parseValueForPath: mocked.parseValueForPath,
  planGraphValueEdit: mocked.planGraphValueEdit,
}));

vi.mock('../../shared/tree-node-value', () => ({
  treeNodeToValue: mocked.treeNodeToValue,
  valueToTreeNode: mocked.valueToTreeNode,
}));


vi.mock('./tree-path', () => ({
  normalizePathSegs: (path: unknown) => path,
}));
vi.mock('./document-parse', () => ({
  parseToTreeNode: mocked.parseToTreeNode,
}));

import { handleApplyValueEditCanonical, handlePlanGraphValueEdit } from './document-value-edit';

describe('document-value-edit', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('normalizes tree-like input before canonical apply', async () => {
    const inputTree = { kind: 1, semType: 2, children: [] };
    const normalizedTree = { kind: 'normalized' };
    const parsedTree = { kind: 'parsed' };
    mocked.treeNodeToValue.mockReturnValueOnce('patched');
    mocked.valueToTreeNode.mockReturnValueOnce(normalizedTree);
    mocked.applyValueEditCanonical.mockResolvedValueOnce({
      text: 'next-text',
      tree: parsedTree,
      value: { ok: true },
    });
    const result = await handleApplyValueEditCanonical({
      id: 1,
      type: 'applyValueEditCanonical',
      language: 'json',
      text: 'source',
      path: [{ key: 'name' }],
      preferKey: false,
      value: inputTree,
      nest: true,
    } as any);
    expect(mocked.parseToTreeNode).not.toHaveBeenCalled();
    expect(mocked.applyValueEditCanonical).toHaveBeenCalledWith('json', 'source', [{ key: 'name' }], false, 'patched');
    expect(result).toEqual({ text: 'next-text', tree: parsedTree, value: { ok: true } });
  });

  it('reuses the normalized plain value for snapshot planner edits', async () => {
    const inputTree = { kind: 1, semType: 2, children: [] };
    const normalizedTree = { kind: 'normalized' };
    const edits = [{ startByte: 8, oldEndByte: 15, newEndByte: 20, text: 'patched' }];
    mocked.treeNodeToValue.mockReturnValueOnce('row-0-updated');
    mocked.valueToTreeNode.mockReturnValueOnce(normalizedTree);
    mocked.planGraphValueEdit.mockResolvedValueOnce({
      status: 'ready',
      data: { mode: 'edits', edits, reason: null },
    });

    const result = await handlePlanGraphValueEdit({
      id: 2,
      type: 'planGraphValueEdit',
      documentKey: 'doc-1',
      snapshotId: 7,
      language: 'json',
      text: 'source',
      path: [{ key: 'name' }],
      preferKey: false,
      value: inputTree,
      nest: true,
      rawReplacement: '{\n  "name": "patched"\n}',
    } as any);

    expect(mocked.planGraphValueEdit).toHaveBeenCalledWith({
      documentKey: 'doc-1',
      snapshotId: 7,
      language: 'json',
      path: [{ key: 'name' }],
      preferKey: false,
      value: 'row-0-updated',
      rawReplacement: '{\n  "name": "patched"\n}',
    });
    expect(result).toEqual({
      mode: 'edits',
      edits,
      tree: normalizedTree,
      value: 'row-0-updated',
      text: 'source',
    });
  });
  it('returns snapshotNotReady when snapshot planning is unavailable', async () => {
    const inputTree = { kind: 2, semType: 3, children: [] };
    const normalizedTree = { kind: 'normalized' };
    mocked.treeNodeToValue.mockReturnValueOnce(43);
    mocked.valueToTreeNode.mockReturnValueOnce(normalizedTree);
    mocked.planGraphValueEdit.mockResolvedValueOnce({ status: 'snapshotNotReady' });
    const result = await handlePlanGraphValueEdit({
      id: 3,
      type: 'planGraphValueEdit',
      documentKey: 'doc-2',
      snapshotId: null,
      language: 'json',
      text: '{"object":{"int":42}}',
      path: [{ key: 'object' }, { key: 'int' }],
      preferKey: false,
      value: inputTree,
      nest: true,
    } as any);
    expect(mocked.planGraphValueEdit).not.toHaveBeenCalled();
    expect(mocked.parseToTreeNode).not.toHaveBeenCalled();
    expect(mocked.applyValueEditCanonical).not.toHaveBeenCalled();
    expect(result).toEqual({ mode: 'snapshotNotReady' });
  });
});
