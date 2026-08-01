import type * as Monaco from 'monaco-editor';
import { toast } from 'svelte-sonner';
import type { SupportedEditorLanguageId } from '../../monaco/language-support';
import { trackEvent } from '../../analytics/ga4';

type FormatCommandKind = 'format' | 'minify' | 'compact' | 'sort';

export type FormatCommandTarget = {
  tabId: string;
  model: Monaco.editor.ITextModel;
  documentKey: string;
  revision: number;
  languageId: SupportedEditorLanguageId;
};

const formatCommandLabels: Record<FormatCommandKind, string> = {
  format: 'Format',
  minify: 'Minify',
  compact: 'Compact',
  sort: 'Sort',
};

type CreateEditorFormatControllerOptions = {
  getActiveTarget: () => FormatCommandTarget | null;
  getFormattingOptions: () => unknown;
  getNestEnabled: () => boolean;
  isImportActive: (target: FormatCommandTarget) => boolean;
  isTargetCurrent: (target: FormatCommandTarget) => boolean;
  isTargetVisible: (target: FormatCommandTarget) => boolean;
  callWasmWorker: <T>(method: string, input: unknown) => Promise<T>;
  replaceWholeDocumentText: (target: FormatCommandTarget, value: string, kind: FormatCommandKind) => Promise<boolean> | boolean;
  resetEditorCursorToStart: (target: FormatCommandTarget) => void;
};

export function createEditorFormatController(options: CreateEditorFormatControllerOptions) {
  const formatCommandQueueByTabId = new Map<string, Promise<void>>();

  async function runFormatCommand(target: FormatCommandTarget, kind: FormatCommandKind): Promise<void> {
    if (!options.isTargetCurrent(target)) return;
    const text = target.model.getValue();
    if (!text.trim()) {
      if (options.isTargetVisible(target)) toast.info('No content to process');
      trackEvent('format_document', { operation: kind, language: target.languageId, result: 'failure' });
      return;
    }
    const label = formatCommandLabels[kind];
    const toastId = options.isTargetVisible(target) ? toast.loading(`${label} queued...`) : null;
    try {
      const nextText = await options.callWasmWorker<string>(kind, {
        language: target.languageId,
        text,
        options: { ...(options.getFormattingOptions() as object | null | undefined), nest: options.getNestEnabled() },
      });
      if (!options.isTargetCurrent(target)) return;
      if (typeof nextText === 'string' && nextText !== text) {
        const replaced = await options.replaceWholeDocumentText(target, nextText, kind);
        if (!replaced) return;
        if (options.isTargetVisible(target)) options.resetEditorCursorToStart(target);
      }
      if (typeof nextText === 'string' && nextText !== text) {
        if (toastId != null && options.isTargetVisible(target)) toast.success(`${label} completed`, { id: toastId });
        trackEvent('format_document', { operation: kind, language: target.languageId, result: 'success' });
      } else if (typeof nextText === 'string') {
        if (toastId != null && options.isTargetVisible(target)) toast.info(`${label} completed (no changes)`, { id: toastId });
        trackEvent('format_document', { operation: kind, language: target.languageId, result: 'success' });
      } else {
        if (toastId != null && options.isTargetVisible(target)) toast.error(`${label} returned unexpected result`, { id: toastId });
        trackEvent('format_document', { operation: kind, language: target.languageId, result: 'failure' });
      }
    } catch (error) {
      if (toastId != null && options.isTargetVisible(target)) toast.error(`${label} failed`, { id: toastId });
      console.error('[editor] format command failed', error);
      trackEvent('format_document', { operation: kind, language: target.languageId, result: 'failure' });
    }
  }

  function enqueue(kind: FormatCommandKind): Promise<void> {
    const target = options.getActiveTarget();
    if (!target) {
      toast.info('No active editor');
      trackEvent('format_document', { operation: kind, result: 'failure' });
      return Promise.resolve();
    }
    if (options.isImportActive(target)) {
      toast.info('Import in progress');
      return Promise.resolve();
    }
    const previous = formatCommandQueueByTabId.get(target.tabId) ?? Promise.resolve();
    const queued = previous
      .catch((error) => {
        console.error('[editor] previous format command failed', error);
      })
      .then(() => runFormatCommand(target, kind));
    formatCommandQueueByTabId.set(target.tabId, queued);
    void queued.finally(() => {
      if (formatCommandQueueByTabId.get(target.tabId) === queued) formatCommandQueueByTabId.delete(target.tabId);
    });
    return queued;
  }

  function formatActive(): Promise<void> {
    return enqueue('format');
  }

  function minifyActive(): Promise<void> {
    return enqueue('minify');
  }

  function compactActive(): Promise<void> {
    return enqueue('compact');
  }

  function sortActive(): Promise<void> {
    return enqueue('sort');
  }

  return {
    enqueue,
    formatActive,
    minifyActive,
    compactActive,
    sortActive,
  };
}
