import { beforeEach, describe, expect, it, vi } from 'vitest';
import { InnerEditorEvent } from '@leafer-in/editor';
import { SemType, TreeKind, type TreeNode } from '@core-wasm/index'
import { toWasmPathSeg, toWasmTreeNode } from '../../../shared/brand-bridge';
import { createGraphValueEditController } from './graph-value-edit';

const mocked = vi.hoisted(() => ({
  callSharedWasmWorker: vi.fn(),
  commitTextEdit: vi.fn(),
  resolveCellPath: vi.fn(async (cell: { path?: unknown[] }, _resolve: unknown, fallback: unknown[] = []) => cell.path ?? fallback),
  clearGraphSelectionAfterEdit: vi.fn((current: unknown) => current),
  treeNodeToValue: vi.fn((tree: TreeNode) => tree.value),
}));

vi.mock('../../wasm/wasm-worker-singleton', () => ({
  callSharedWasmWorker: mocked.callSharedWasmWorker,
}));

vi.mock('../../graph/GraphEditHandler', () => ({
  commitTextEdit: mocked.commitTextEdit,
}));

vi.mock('./graph-anchor-index', () => ({
  resolveCellPath: mocked.resolveCellPath,
}));

vi.mock('../GraphViewer.graph-highlight', () => ({
  clearGraphSelectionAfterEdit: mocked.clearGraphSelectionAfterEdit,
}));

vi.mock('../../../shared/tree-node-value', () => ({
  treeNodeToValue: mocked.treeNodeToValue,
}));

function scalarNode(value: string): TreeNode {
  return toWasmTreeNode({
    kind: TreeKind.SCALAR,
    semType: SemType.STR,
    tag: 'str',
    value,
    children: [],
  });
}

describe('graph-value-edit', () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it('uses replaceSourceText when worker returns a graph edit replace plan', async () => {
    const canonicalNode = scalarNode('next-value');
    mocked.commitTextEdit.mockResolvedValue({
      nextValue: 'next-value',
      nextValueNode: canonicalNode,
      preferKey: false,
    });
    mocked.callSharedWasmWorker.mockImplementation(async (type: string) => {
      if (type === 'planGraphValueEdit') {
        return {
          mode: 'replace',
          reason: 'graph-edit-not-single-range',
          text: '{"name":"next-value"}',
          tree: canonicalNode,
          value: 'next-value',
        };
      }
      throw new Error(`unexpected worker call: ${type}`);
    });

    const applyTextEdits = vi.fn(() => true);
    const emitEditorMutation = vi.fn();
    const publishTreeState = vi.fn(() => true);
    const model = { getVersionId: vi.fn(() => 1) };
    const controller = createGraphValueEditController({
      getCurrentData: () => ({ name: 'current' }),
      getSourceText: () => '{"name":"current"}',
      getDocumentKey: () => 'doc-key',
      getLanguageId: () => 'json',
      getEnableNest: () => true,
      getEditorIO: () => ({ context: 'editor', getModel: () => model as any, applyTextEdits } as any),
      getEditorRevision: () => 7,
      getActiveSnapshotId: () => 42,
      resolveTreePathByPosition: vi.fn(async () => []),
      nextTreeStateToken: () => 11,
      publishTreeState,
      emitEditorMutation,
      updateActiveTempModel: vi.fn(),
      dispatchGraphEditEvent: vi.fn(),
      handleError: vi.fn(),
    });

    const applied = await controller.applyGraphEdit({ path: [{ key: 'name' }], valueType: 'string' } as any, 'value', 'next-value');

    expect(applied).toBe(true);
    expect(publishTreeState).toHaveBeenCalledWith(11, canonicalNode, 'next-value', 'graph', 8);
    expect(applyTextEdits).not.toHaveBeenCalled();
    expect(emitEditorMutation).toHaveBeenCalledWith({
      type: 'replaceSourceText',
      payload: { text: '{"name":"next-value"}' },
    });
  });

  it('treats string value edits as semantic strings without calling parseValueForPath', async () => {
    const canonicalNode = scalarNode('next-value');
    mocked.commitTextEdit.mockImplementation(async (_context, _cell, _target, _kind, valueParser) => {
      const parsed = await valueParser({
        language: 'json',
        text: 'next-value',
        rawEdit: 'next-value',
        nest: true,
        path: [{ key: 'name' }],
        preferKey: false,
      } as any);
      return {
        nextValue: parsed.value,
        nextValueNode: parsed.tree,
        preferKey: false,
      };
    });
    mocked.callSharedWasmWorker.mockImplementation(async (type: string) => {
      if (type === 'valueToTreeNode') {
        return canonicalNode;
      }
      if (type === 'planGraphValueEdit') {
        return {
          mode: 'edits',
          edits: [
            {
              startByte: 8,
              oldEndByte: 15,
              newEndByte: 20,
              startRow: 0,
              startColumn: 8,
              oldEndRow: 0,
              oldEndColumn: 15,
              newEndRow: 0,
              newEndColumn: 20,
              text: '"next-value"',
            },
          ],
          text: '{"name":"next-value"}',
          tree: canonicalNode,
          value: 'next-value',
        };
      }
      throw new Error(`unexpected worker call: ${type}`);
    });

    const applyTextEdits = vi.fn(() => true);
    const model = { getVersionId: vi.fn(() => 1) };
    const controller = createGraphValueEditController({
      getCurrentData: () => ({ name: 'current' }),
      getSourceText: () => '{"name":"current"}',
      getDocumentKey: () => 'doc-key',
      getLanguageId: () => 'json',
      getEnableNest: () => true,
      getEditorIO: () => ({ context: 'editor', getModel: () => model as any, applyTextEdits } as any),
      getEditorRevision: () => 3,
      getActiveSnapshotId: () => 42,
      resolveTreePathByPosition: vi.fn(async () => []),
      nextTreeStateToken: () => 5,
      publishTreeState: vi.fn(() => true),
      emitEditorMutation: vi.fn(),
      updateActiveTempModel: vi.fn(),
      dispatchGraphEditEvent: vi.fn(),
      handleError: vi.fn(),
    });

    const applied = await controller.applyGraphEdit({ path: [{ key: 'name' }], valueType: 'string' } as any, 'value', 'next-value');

    expect(applied).toBe(true);
    expect(applyTextEdits).toHaveBeenCalledWith([
      expect.objectContaining({ text: '"next-value"' }),
    ]);
    expect(mocked.callSharedWasmWorker).toHaveBeenCalledWith(
      'planGraphValueEdit',
      expect.objectContaining({
        documentKey: 'doc-key',
        snapshotId: 42,
        language: 'json',
        path: [{ key: 'name' }],
        preferKey: false,
      }),
    );
    expect(mocked.callSharedWasmWorker).not.toHaveBeenCalledWith(
      'parseValueForPath',
      expect.anything(),
    );
  });

  it('keeps empty-string edits semantic when the rendered placeholder is double quotes', async () => {
    const canonicalNode = scalarNode('');
    mocked.commitTextEdit.mockImplementation(async (_context, _cell, _target, _kind, valueParser) => {
      const parsed = await valueParser({
        language: 'json',
        text: '""',
        rawEdit: '""',
        nest: true,
        path: [{ key: 'empty string' }],
        preferKey: false,
      } as any);
      return {
        nextValue: parsed.value,
        nextValueNode: parsed.tree,
        preferKey: false,
      };
    });
    mocked.callSharedWasmWorker.mockImplementation(async (type: string, payload: any) => {
      if (type === 'valueToTreeNode') {
        expect(payload).toEqual({ value: '' });
        return canonicalNode;
      }
      if (type === 'planGraphValueEdit') {
        return {
          mode: 'edits',
          edits: [
            {
              startByte: 17,
              oldEndByte: 19,
              newEndByte: 19,
              startRow: 0,
              startColumn: 17,
              oldEndRow: 0,
              oldEndColumn: 19,
              newEndRow: 0,
              newEndColumn: 19,
              text: '""',
            },
          ],
          text: '{"empty string":""}',
          tree: canonicalNode,
          value: '',
        };
      }
      throw new Error(`unexpected worker call: ${type}`);
    });

    const applyTextEdits = vi.fn(() => true);
    const model = { getVersionId: vi.fn(() => 1) };
    const controller = createGraphValueEditController({
      getCurrentData: () => ({ 'empty string': '' }),
      getSourceText: () => '{"empty string":""}',
      getDocumentKey: () => 'doc-key',
      getLanguageId: () => 'json',
      getEnableNest: () => true,
      getEditorIO: () => ({ context: 'editor', getModel: () => model as any, applyTextEdits } as any),
      getEditorRevision: () => 3,
      getActiveSnapshotId: () => 42,
      resolveTreePathByPosition: vi.fn(async () => []),
      nextTreeStateToken: () => 5,
      publishTreeState: vi.fn(() => true),
      emitEditorMutation: vi.fn(),
      updateActiveTempModel: vi.fn(),
      dispatchGraphEditEvent: vi.fn(),
      handleError: vi.fn(),
    });

    const applied = await controller.applyGraphEdit({ path: [{ key: 'empty string' }], valueType: 'string', text: '' } as any, 'value', '""');

    expect(applied).toBe(true);
    expect(applyTextEdits).toHaveBeenCalledWith([
      expect.objectContaining({ text: '""' }),
    ]);
  });

  it('does not replace the whole document when edit-mode application fails in the editor', async () => {
    const canonicalNode = scalarNode('next-value');
    mocked.commitTextEdit.mockResolvedValue({
      nextValue: 'next-value',
      nextValueNode: canonicalNode,
      preferKey: false,
    });
    mocked.callSharedWasmWorker.mockImplementation(async (type: string) => {
      if (type === 'planGraphValueEdit') {
        return {
          mode: 'edits',
          edits: [
            {
              startByte: 8,
              oldEndByte: 15,
              newEndByte: 20,
              startRow: 0,
              startColumn: 8,
              oldEndRow: 0,
              oldEndColumn: 15,
              newEndRow: 0,
              newEndColumn: 20,
              text: '"next-value"',
            },
          ],
          text: '{"name":"next-value"}',
          tree: canonicalNode,
          value: 'next-value',
        };
      }
      throw new Error(`unexpected worker call: ${type}`);
    });

    const applyTextEdits = vi.fn(() => false);
    const emitEditorMutation = vi.fn();
    const model = { getVersionId: vi.fn(() => 1) };
    const controller = createGraphValueEditController({
      getCurrentData: () => ({ name: 'current' }),
      getSourceText: () => '{"name":"current"}',
      getDocumentKey: () => 'doc-key',
      getLanguageId: () => 'json',
      getEnableNest: () => true,
      getEditorIO: () => ({ context: 'editor', getModel: () => model as any, applyTextEdits } as any),
      getEditorRevision: () => 3,
      getActiveSnapshotId: () => 42,
      resolveTreePathByPosition: vi.fn(async () => []),
      nextTreeStateToken: () => 5,
      publishTreeState: vi.fn(() => true),
      emitEditorMutation,
      updateActiveTempModel: vi.fn(),
      dispatchGraphEditEvent: vi.fn(),
      handleError: vi.fn(),
    });

    const applied = await controller.applyGraphEdit({ path: [{ key: 'name' }], valueType: 'string' } as any, 'value', 'next-value');

    expect(applied).toBe(false);
    expect(applyTextEdits).toHaveBeenCalledTimes(1);
    expect(emitEditorMutation).not.toHaveBeenCalled();
  });

  it('passes documentKey when delegating non-string edits to parseValueForPath', async () => {
    const parsedNode = scalarNode('a');
    mocked.commitTextEdit.mockImplementation(async (_context, _cell, _target, _kind, valueParser) => {
      const parsed = await valueParser({
        language: 'json',
        rawEdit: 'a',
        nest: true,
        path: [{ key: 'enabled' }],
        preferKey: false,
      } as any);
      return {
        nextValue: parsed.value,
        nextValueNode: parsed.tree,
        preferKey: false,
      };
    });
    mocked.callSharedWasmWorker.mockImplementation(async (type: string, payload: any) => {
      if (type === 'parseValueForPath') {
        expect(payload).toMatchObject({
          language: 'json',
          documentKey: 'doc-key',
          text: '{"enabled":true}',
          rawEdit: 'a',
          preferKey: false,
          nest: true,
        });
        return parsedNode;
      }
      if (type === 'planGraphValueEdit') {
        return {
          mode: 'replace',
          reason: 'graph-edit-not-single-range',
          text: '{"enabled":"a"}',
          tree: parsedNode,
          value: 'a',
        };
      }
      throw new Error(`unexpected worker call: ${type}`);
    });

    const model = { getVersionId: () => 1 };
    const controller = createGraphValueEditController({
      getCurrentData: () => ({ enabled: true }),
      getSourceText: () => '{"enabled":true}',
      getDocumentKey: () => 'doc-key',
      getLanguageId: () => 'json',
      getEnableNest: () => true,
      getEditorIO: () => ({
        context: 'editor',
        getModel: () => model as any,
        applyTextEdits: vi.fn(() => true),
      } as any),
      getEditorRevision: () => 2,
      getActiveSnapshotId: () => 42,
      resolveTreePathByPosition: vi.fn(async () => []),
      nextTreeStateToken: () => 3,
      publishTreeState: vi.fn(() => true),
      emitEditorMutation: vi.fn(),
      updateActiveTempModel: vi.fn(),
      dispatchGraphEditEvent: vi.fn(),
      handleError: vi.fn(),
    });

    const applied = await controller.applyGraphEdit({ path: [{ key: 'enabled' }], valueType: 'boolean' } as any, 'value', 'a');
    expect(applied).toBe(true);
  });

  it('drops stale graph edit plans when the Monaco model version changes before apply', async () => {
    const canonicalNode = scalarNode('next-value');
    mocked.commitTextEdit.mockResolvedValue({
      nextValue: 'next-value',
      nextValueNode: canonicalNode,
      preferKey: false,
    });
    let modelVersion = 1;
    mocked.callSharedWasmWorker.mockImplementation(async (type: string) => {
      if (type === 'planGraphValueEdit') {
        modelVersion = 2;
        return {
          mode: 'edits',
          edits: [
            {
              startByte: 8,
              oldEndByte: 15,
              newEndByte: 20,
              startRow: 0,
              startColumn: 8,
              oldEndRow: 0,
              oldEndColumn: 15,
              newEndRow: 0,
              newEndColumn: 20,
              text: '"next-value"',
            },
          ],
          text: '{"name":"next-value"}',
          tree: canonicalNode,
          value: 'next-value',
        };
      }
      throw new Error(`unexpected worker call: ${type}`);
    });

    const applyTextEdits = vi.fn(() => true);
    const emitEditorMutation = vi.fn();
    const publishTreeState = vi.fn(() => true);
    const model = { getVersionId: () => modelVersion };
    const controller = createGraphValueEditController({
      getCurrentData: () => ({ name: 'current' }),
      getSourceText: () => '{"name":"current"}',
      getDocumentKey: () => 'doc-key',
      getLanguageId: () => 'json',
      getEnableNest: () => true,
      getEditorIO: () => ({ context: 'editor', getModel: () => model as any, applyTextEdits } as any),
      getEditorRevision: () => 3,
      getActiveSnapshotId: () => 42,
      resolveTreePathByPosition: vi.fn(async () => []),
      nextTreeStateToken: () => 5,
      publishTreeState,
      emitEditorMutation,
      updateActiveTempModel: vi.fn(),
      dispatchGraphEditEvent: vi.fn(),
      handleError: vi.fn(),
    });

    const applied = await controller.applyGraphEdit({ path: [{ key: 'name' }], valueType: 'string' } as any, 'value', 'next-value');

    expect(applied).toBe(false);
    expect(publishTreeState).not.toHaveBeenCalled();
    expect(applyTextEdits).not.toHaveBeenCalled();
    expect(emitEditorMutation).not.toHaveBeenCalled();
  });

  it('drops stale graph edit plans when the document revision changes before apply', async () => {
    const canonicalNode = scalarNode('next-value');
    mocked.commitTextEdit.mockResolvedValue({
      nextValue: 'next-value',
      nextValueNode: canonicalNode,
      preferKey: false,
    });
    let revision = 3;
    mocked.callSharedWasmWorker.mockImplementation(async (type: string) => {
      if (type === 'planGraphValueEdit') {
        revision = 4;
        return {
          mode: 'edits',
          edits: [
            {
              startByte: 8,
              oldEndByte: 15,
              newEndByte: 20,
              startRow: 0,
              startColumn: 8,
              oldEndRow: 0,
              oldEndColumn: 15,
              newEndRow: 0,
              newEndColumn: 20,
              text: '"next-value"',
            },
          ],
          text: '{"name":"next-value"}',
          tree: canonicalNode,
          value: 'next-value',
        };
      }
      throw new Error(`unexpected worker call: ${type}`);
    });

    const applyTextEdits = vi.fn(() => true);
    const emitEditorMutation = vi.fn();
    const publishTreeState = vi.fn(() => true);
    const model = { getVersionId: () => 1 };
    const controller = createGraphValueEditController({
      getCurrentData: () => ({ name: 'current' }),
      getSourceText: () => '{"name":"current"}',
      getDocumentKey: () => 'doc-key',
      getLanguageId: () => 'json',
      getEnableNest: () => true,
      getEditorIO: () => ({ context: 'editor', getModel: () => model as any, applyTextEdits } as any),
      getEditorRevision: () => revision,
      getActiveSnapshotId: () => 42,
      resolveTreePathByPosition: vi.fn(async () => []),
      nextTreeStateToken: () => 5,
      publishTreeState,
      emitEditorMutation,
      updateActiveTempModel: vi.fn(),
      dispatchGraphEditEvent: vi.fn(),
      handleError: vi.fn(),
    });

    const applied = await controller.applyGraphEdit({ path: [{ key: 'name' }], valueType: 'string' } as any, 'value', 'next-value');

    expect(applied).toBe(false);
    expect(publishTreeState).not.toHaveBeenCalled();
    expect(applyTextEdits).not.toHaveBeenCalled();
    expect(emitEditorMutation).not.toHaveBeenCalled();
  });

  it('suppresses graph value edit opens and commits when readonly', async () => {
    let readonly = true;
    const applyTextEdits = vi.fn(() => true);
    const emitEditorMutation = vi.fn();
    const publishTreeState = vi.fn(() => true);
    const dispatchGraphEditEvent = vi.fn();
    const model = { getVersionId: () => 1 };
    const controller = createGraphValueEditController({
      getCurrentData: () => ({ name: 'current' }),
      getSourceText: () => '{"name":"current"}',
      getDocumentKey: () => 'doc-key',
      getLanguageId: () => 'json',
      getEnableNest: () => true,
      isReadonly: () => readonly,
      getEditorIO: () => ({ context: 'editor', getModel: () => model as any, applyTextEdits } as any),
      getEditorRevision: () => 3,
      getActiveSnapshotId: () => 42,
      resolveTreePathByPosition: vi.fn(async () => []),
      nextTreeStateToken: () => 5,
      publishTreeState,
      emitEditorMutation,
      updateActiveTempModel: vi.fn(),
      dispatchGraphEditEvent,
      handleError: vi.fn(),
    });
    const listeners = new Map<unknown, (event: unknown) => void>();
    const editor = {
      innerEditor: { config: {} },
      getInnerEditor: vi.fn(() => ({ config: {} })),
      on: vi.fn((type: unknown, handler: (event: unknown) => void) => {
        listeners.set(type, handler);
      }),
    };
    const cell = { path: [{ key: 'name' }], valueType: 'string', text: 'current' };
    const target = {
      __graphCell: cell,
      __graphCellKind: 'value',
      text: 'current',
    };

    controller.bindGraphEditorLifecycle(editor as any);
    listeners.get(InnerEditorEvent.BEFORE_OPEN)?.({ editTarget: target });

    expect(controller.hasActiveEdit()).toBe(false);
    expect(dispatchGraphEditEvent).not.toHaveBeenCalled();

    readonly = false;
    listeners.get(InnerEditorEvent.BEFORE_OPEN)?.({ editTarget: target });
    expect(controller.hasActiveEdit()).toBe(true);
    expect(dispatchGraphEditEvent).toHaveBeenCalledWith(
      'graph-edit-open',
      expect.objectContaining({ path: [{ key: 'name' }], kind: 'value', valueType: 'string' }),
    );
    dispatchGraphEditEvent.mockClear();

    readonly = true;
    listeners.get(InnerEditorEvent.CLOSE)?.({});
    await Promise.resolve();

    expect(controller.hasActiveEdit()).toBe(false);
    expect(dispatchGraphEditEvent).not.toHaveBeenCalled();
    expect(mocked.commitTextEdit).not.toHaveBeenCalled();
    expect(mocked.callSharedWasmWorker).not.toHaveBeenCalled();
    expect(applyTextEdits).not.toHaveBeenCalled();
    expect(emitEditorMutation).not.toHaveBeenCalled();
    expect(publishTreeState).not.toHaveBeenCalled();

    await expect(controller.applyGraphEdit(cell as any, 'value', 'next')).resolves.toBe(false);

    expect(dispatchGraphEditEvent).not.toHaveBeenCalled();
    expect(mocked.commitTextEdit).not.toHaveBeenCalled();
    expect(mocked.callSharedWasmWorker).not.toHaveBeenCalled();
    expect(applyTextEdits).not.toHaveBeenCalled();
    expect(emitEditorMutation).not.toHaveBeenCalled();
    expect(publishTreeState).not.toHaveBeenCalled();
  });

  it('rejects edits for missing placeholder cells', async () => {
    const applyTextEdits = vi.fn(() => true);
    const emitEditorMutation = vi.fn();
    const publishTreeState = vi.fn(() => true);
    const dispatchGraphEditEvent = vi.fn();
    const controller = createGraphValueEditController({
      getCurrentData: () => ({ user: {} }),
      getSourceText: () => '{"user":{}}',
      getDocumentKey: () => 'doc-key',
      getLanguageId: () => 'json',
      getEnableNest: () => true,
      isReadonly: () => false,
      getEditorIO: () => ({
        context: 'editor',
        getModel: () => ({ getVersionId: () => 1 }) as any,
        applyTextEdits,
      }) as any,
      getEditorRevision: () => 3,
      getActiveSnapshotId: () => 42,
      resolveTreePathByPosition: vi.fn(async () => [
        toWasmPathSeg({ tag: 0, key: 'user', index: 0 }),
        toWasmPathSeg({ tag: 0, key: 'missing', index: 0 }),
      ]),
      nextTreeStateToken: () => 5,
      publishTreeState,
      emitEditorMutation,
      updateActiveTempModel: vi.fn(),
      dispatchGraphEditEvent,
      handleError: vi.fn(),
    });

    await expect(
      controller.applyGraphEdit(
        { path: [], valueType: 'object', text: 'miss', value: 'miss', isMissing: true, editable: false } as any,
        'value',
        '{"name":"Ada"}',
      ),
    ).resolves.toBe(false);

    expect(dispatchGraphEditEvent).not.toHaveBeenCalled();
    expect(mocked.callSharedWasmWorker).not.toHaveBeenCalled();
    expect(applyTextEdits).not.toHaveBeenCalled();
    expect(emitEditorMutation).not.toHaveBeenCalled();
    expect(publishTreeState).not.toHaveBeenCalled();
  });
});
