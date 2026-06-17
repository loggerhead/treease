import type * as Monaco from 'monaco-editor';
import { toast } from 'svelte-sonner';
import type { SupportedEditorLanguageId } from '../../monaco/language-support';

type FormatCommandKind = 'format' | 'minify' | 'sort';

const formatCommandLabels: Record<FormatCommandKind, string> = {
  format: 'Format',
  minify: 'Minify',
  sort: 'Sort',
};

type CreateEditorFormatControllerOptions = {
  getModel: () => Monaco.editor.ITextModel | null;
  getLanguageId: () => SupportedEditorLanguageId;
  getFormattingOptions: () => unknown;
  getNestEnabled: () => boolean;
  isImportActive: () => boolean;
  callWasmWorker: <T>(method: string, input: unknown) => Promise<T>;
  replaceWholeDocumentText: (value: string, kind: FormatCommandKind) => boolean;
  resetEditorCursorToStart: () => void;
};

export function createEditorFormatController(options: CreateEditorFormatControllerOptions) {
  let formatCommandQueue: Promise<void> = Promise.resolve();

  async function runFormatCommand(kind: FormatCommandKind): Promise<void> {
    const activeModel = options.getModel();
    if (!activeModel) {
      toast.info('No active editor');
      return;
    }
    const text = activeModel.getValue();
    if (!text.trim()) {
      toast.info('No content to process');
      return;
    }
    const label = formatCommandLabels[kind];
    const toastId = toast.loading(`${label} queued...`);
    try {
      const nextText = await options.callWasmWorker<string>(kind, {
        language: options.getLanguageId(),
        text,
        options: { ...(options.getFormattingOptions() as object | null | undefined), nest: options.getNestEnabled() },
      });
      if (typeof nextText === 'string') {
        if (nextText !== text) {
          options.replaceWholeDocumentText(nextText, kind);
        }
        options.resetEditorCursorToStart();
      }
      if (typeof nextText === 'string' && nextText !== text) {
        toast.success(`${label} completed`, { id: toastId });
      } else if (typeof nextText === 'string') {
        toast.info(`${label} completed (no changes)`, { id: toastId });
      } else {
        toast.error(`${label} returned unexpected result`, { id: toastId });
      }
    } catch (error) {
      toast.error(`${label} failed`, { id: toastId });
      console.error('[editor] format command failed', error);
    }
  }

  function enqueue(kind: FormatCommandKind): Promise<void> {
    if (options.isImportActive()) {
      toast.info('Import in progress');
      return Promise.resolve();
    }
    formatCommandQueue = formatCommandQueue
      .catch((error) => {
        console.error('[editor] previous format command failed', error);
      })
      .then(() => runFormatCommand(kind));
    return formatCommandQueue;
  }

  function formatActive(): Promise<void> {
    return enqueue('format');
  }

  function minifyActive(): Promise<void> {
    return enqueue('minify');
  }

  function sortActive(): Promise<void> {
    return enqueue('sort');
  }

  return {
    enqueue,
    formatActive,
    minifyActive,
    sortActive,
  };
}
