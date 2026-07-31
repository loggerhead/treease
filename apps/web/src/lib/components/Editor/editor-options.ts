import type * as Monaco from 'monaco-editor';

export function createTreeaseMonacoEditorOptions(
  theme: string,
): Monaco.editor.IStandaloneEditorConstructionOptions {
  return {
    theme,
    fontSize: 13,
    scrollBeyondLastLine: false,
    automaticLayout: true,
    smoothScrolling: true,
    cursorSmoothCaretAnimation: 'on',
    'semanticHighlighting.enabled': true,
    wordWrap: 'on',
    minimap: { enabled: false },
    stickyScroll: {
      enabled: true,
      defaultModel: 'foldingProviderModel',
    },
  };
}
