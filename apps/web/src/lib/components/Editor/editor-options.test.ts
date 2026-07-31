import { describe, expect, it } from 'vitest';
import { createTreeaseMonacoEditorOptions } from './editor-options';

describe('createTreeaseMonacoEditorOptions', () => {
  it('returns the shared json4u-aligned baseline', () => {
    expect(createTreeaseMonacoEditorOptions('tree-sitter-light')).toEqual({
      theme: 'tree-sitter-light',
      'semanticHighlighting.enabled': true,
      fontSize: 13,
      scrollBeyondLastLine: false,
      automaticLayout: true,
      smoothScrolling: true,
      cursorSmoothCaretAnimation: 'on',
      wordWrap: 'on',
      minimap: { enabled: false },
      stickyScroll: {
        enabled: true,
        defaultModel: 'foldingProviderModel',
      },
    });
  });
});
