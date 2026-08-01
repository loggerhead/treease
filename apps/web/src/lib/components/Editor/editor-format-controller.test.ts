import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('svelte-sonner', () => ({
  toast: {
    info: vi.fn(),
    loading: vi.fn(() => 'toast-id'),
    success: vi.fn(),
    error: vi.fn(),
  },
}));

import { toast } from 'svelte-sonner';
import { createEditorFormatController } from './editor-format-controller';

function createOptions(overrides: Record<string, unknown> = {}) {
  const model = {
    getValue: vi.fn(() => '{"b":2,"a":1}'),
  } as any;
  const target = {
    tabId: 'tab-a',
    model,
    documentKey: 'tab-a:0',
    revision: 1,
    languageId: 'json' as const,
  };
  return {
    getActiveTarget: vi.fn(() => target),
    getFormattingOptions: vi.fn(() => ({ indent: 2 })),
    getNestEnabled: vi.fn(() => true),
    isImportActive: vi.fn(() => false),
    isTargetCurrent: vi.fn(() => true),
    isTargetVisible: vi.fn(() => true),
    callWasmWorker: vi.fn(async () => '{\n  "a": 1,\n  "b": 2\n}'),
    replaceWholeDocumentText: vi.fn(() => true),
    resetEditorCursorToStart: vi.fn(),
    ...overrides,
  } as any;
}

describe('createEditorFormatController', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('forwards format options and updates editor when text changes', async () => {
    const options = createOptions();
    const controller = createEditorFormatController(options);

    await controller.formatActive();

    expect(options.callWasmWorker).toHaveBeenCalledWith('format', {
      language: 'json',
      text: '{"b":2,"a":1}',
      options: { indent: 2, nest: true },
    });
    expect(options.replaceWholeDocumentText).toHaveBeenCalledWith(
      expect.objectContaining({ tabId: 'tab-a' }),
      '{\n  "a": 1,\n  "b": 2\n}',
      'format',
    );
    expect(options.resetEditorCursorToStart).toHaveBeenCalledWith(expect.objectContaining({ tabId: 'tab-a' }));
    expect(vi.mocked(toast.loading)).toHaveBeenCalledWith('Format queued...');
    expect(vi.mocked(toast.success)).toHaveBeenCalledWith('Format completed', { id: 'toast-id' });
  });

  it('keeps no-change commands visible without rewriting the editor', async () => {
    const options = createOptions({
      callWasmWorker: vi.fn(async () => '{"b":2,"a":1}'),
    });
    const controller = createEditorFormatController(options);

    await controller.sortActive();

    expect(options.replaceWholeDocumentText).not.toHaveBeenCalled();
    expect(options.resetEditorCursorToStart).not.toHaveBeenCalled();
    expect(vi.mocked(toast.info)).toHaveBeenCalledWith('Sort completed (no changes)', { id: 'toast-id' });
  });

  it('skips queued commands while import is active', async () => {
    const options = createOptions({
      isImportActive: vi.fn(() => true),
    });
    const controller = createEditorFormatController(options);

    await controller.minifyActive();

    expect(options.callWasmWorker).not.toHaveBeenCalled();
    expect(vi.mocked(toast.info)).toHaveBeenCalledWith('Import in progress');
  });

  it('discards a format result when its captured target becomes stale', async () => {
    let current = true;
    const deferred = new Promise<string>((resolve) => queueMicrotask(() => resolve('{"a":1}')));
    const options = createOptions({
      callWasmWorker: vi.fn(() => deferred),
      isTargetCurrent: vi.fn(() => current),
    });
    const controller = createEditorFormatController(options);

    const pending = controller.formatActive();
    current = false;
    await pending;

    expect(options.replaceWholeDocumentText).not.toHaveBeenCalled();
    expect(options.resetEditorCursorToStart).not.toHaveBeenCalled();
  });

  it('does not add application-level queue blocking between tabs', async () => {
    let resolveFirst!: (value: string) => void;
    let resolveSecond!: (value: string) => void;
    const first = new Promise<string>((resolve) => {
      resolveFirst = resolve;
    });
    const second = new Promise<string>((resolve) => {
      resolveSecond = resolve;
    });
    const options = createOptions();
    const targetA = options.getActiveTarget();
    const targetB = {
      ...targetA,
      tabId: 'tab-b',
      documentKey: 'tab-b:0',
      model: { getValue: () => '{"b":2,"a":1}' },
    };
    options.getActiveTarget
      .mockReturnValueOnce(targetA)
      .mockReturnValueOnce(targetB);
    options.callWasmWorker
      .mockReturnValueOnce(first)
      .mockReturnValueOnce(second);
    const controller = createEditorFormatController(options);

    const pendingA = controller.formatActive();
    const pendingB = controller.formatActive();
    await vi.waitFor(() => expect(options.callWasmWorker).toHaveBeenCalledTimes(2));

    resolveSecond('{"b":2,"a":1}');
    resolveFirst('{"b":2,"a":1}');
    await Promise.all([pendingA, pendingB]);
  });
});
