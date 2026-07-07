import { registerEditorHook, unregisterEditorHook } from '../test-bridge/register-editor-bridge';

type TestHookEditor = {
  getDomNode?: () => HTMLElement | null;
  getValue: () => string;
  setValue: (value: string) => void;
  setValueForTestHook?: (value: string) => void;
  focus?: () => void;
  setPosition?: (position: { lineNumber: number; column: number }) => void;
  revealPositionInCenter?: (position: { lineNumber: number; column: number }) => void;
  getScrollTop?: () => number;
  getScrollLeft?: () => number;
  setScrollPosition?: (position: { scrollTop: number; scrollLeft: number }) => void;
  executeEdits?: (source: string, edits: Array<{ range: unknown; text: string; forceMoveMarkers?: boolean }>) => void;
  onDidChangeModel?: (listener: () => void) => { dispose: () => void };
  getMarkers?: () => Array<{ owner?: string; message?: string; severity?: number }>;
  getModel?: () => {
    getLanguageId?: () => string | null;
    getLineContent?: (lineNumber: number) => string;
  } | null;
};

type TokenizeFn = (text: string, languageId: string) => Array<{ offset: number; type: string; language: string }>[];

function shouldAttachMonacoTestHook() {
  return import.meta.env.DEV || import.meta.env.MODE === 'test';
}

export function attachMonacoTestHook(editor: TestHookEditor, hookId: string, tokenize?: TokenizeFn) {
  if (!shouldAttachMonacoTestHook()) return () => {};
  let node = editor.getDomNode?.() ?? null;
  let domMarkFrame: number | null = null;
  let disposed = false;

  const markDomNode = () => {
    if (disposed) return false;
    node = editor.getDomNode?.() ?? node;
    if (!node) return false;
    node.dataset.monacoTestHook = hookId;
    node.dataset.testid = `monaco-${hookId}`;
    return true;
  };
  const scheduleDomMark = () => {
    if (markDomNode()) return;
    queueMicrotask(() => {
      if (markDomNode() || disposed) return;
      const retry = () => {
        domMarkFrame = null;
        if (markDomNode() || disposed) return;
        domMarkFrame = requestAnimationFrame(retry);
      };
      if (domMarkFrame == null) {
        domMarkFrame = requestAnimationFrame(retry);
      }
    });
  };
  scheduleDomMark();
  const modelChangeDisposable = editor.onDidChangeModel?.(scheduleDomMark);
  registerEditorHook(hookId, {
    getValue: () => editor.getValue(),
    setValue: (value: string) => {
      editor.setValue(value);
      editor.focus?.();
    },
    setValueExact: (value: string) => {
      (editor.setValueForTestHook ?? editor.setValue)(value);
      editor.focus?.();
    },
    applyEdits: (edits) => {
      if (!editor.executeEdits) return;
      editor.executeEdits('test-bridge-edit', edits.map((e: any) => ({ ...e, forceMoveMarkers: true })));
    },
    setPosition: (lineNumber: number, column: number) => {
      editor.focus?.();
      editor.setPosition?.({ lineNumber, column });
      editor.revealPositionInCenter?.({ lineNumber, column });
      editor.focus?.();
    },
    getScroll: () => ({
      scrollTop: editor.getScrollTop?.() ?? 0,
      scrollLeft: editor.getScrollLeft?.() ?? 0,
    }),
    setScroll: (scrollTop: number, scrollLeft = 0) => {
      editor.setScrollPosition?.({ scrollTop, scrollLeft });
      editor.focus?.();
    },
    getLanguage: () => editor.getModel?.()?.getLanguageId?.() ?? null,
    getMarkers: () => editor.getMarkers?.() ?? [],
    getRenderedTokenColor: (tokenText: string, lineNumber?: number) => {
      const currentNode = editor.getDomNode?.() ?? node;
      if (!currentNode) return null;
      const lineSelector = lineNumber != null ? `.view-line:nth-child(${lineNumber})` : '.view-line';
      const line = currentNode.querySelector(`.view-lines ${lineSelector}`) as HTMLElement | null;
      const spans = Array.from(
        currentNode.querySelectorAll(`.view-lines ${lineSelector} span, .view-lines ${lineSelector} *`),
      ) as HTMLElement[];
      const candidates = spans.filter((span) => (span.textContent ?? '').includes(tokenText));
      const target = candidates.sort((a, b) => (a.textContent ?? '').length - (b.textContent ?? '').length)[0];
      if (!target) return null;
      const lineColor = line ? getComputedStyle(line).color : null;
      let current: HTMLElement | null = target;
      while (current) {
        const color = getComputedStyle(current).color;
        if (
          color &&
          color !== 'rgb(0, 0, 0)' &&
          color !== 'rgba(0, 0, 0, 0)' &&
          color !== lineColor
        ) {
          return color;
        }
        if (current === line) break;
        current = current.parentElement;
      }
      return lineColor ?? getComputedStyle(target).color;
    },
    getTokenTypeAt: (lineNumber: number, column: number) => {
      if (!tokenize) return 'no-tokenize';
      const model = editor.getModel?.();
      if (!model) return 'no-model';
      const languageId = model.getLanguageId?.();
      if (!languageId) return 'no-language';
      const lineContent = model.getLineContent?.(lineNumber);
      if (lineContent == null) return 'no-content';
      const tokenized = tokenize(lineContent, languageId);
      if (!tokenized || tokenized.length === 0) return null;
      const tokens = tokenized[0];
      if (!tokens || tokens.length === 0) return null;
      const offset = column - 1;
      for (let i = tokens.length - 1; i >= 0; i--) {
        const token = tokens[i];
        if (offset >= token.offset) {
          return token.type ?? null;
        }
      }
      return null;
    },
  });
  return () => {
    disposed = true;
    if (domMarkFrame != null) {
      cancelAnimationFrame(domMarkFrame);
      domMarkFrame = null;
    }
    modelChangeDisposable?.dispose();
    unregisterEditorHook(hookId);
  };
}
