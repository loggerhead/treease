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
  return {
    getModel: vi.fn(() => model),
    getLanguageId: vi.fn(() => 'json' as const),
    getFormattingOptions: vi.fn(() => ({ indent: 2 })),
    getNestEnabled: vi.fn(() => true),
    isImportActive: vi.fn(() => false),
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
    expect(options.replaceWholeDocumentText).toHaveBeenCalledWith('{\n  "a": 1,\n  "b": 2\n}', 'format');
    expect(options.resetEditorCursorToStart).toHaveBeenCalledTimes(1);
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
    expect(options.resetEditorCursorToStart).toHaveBeenCalledTimes(1);
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
});
