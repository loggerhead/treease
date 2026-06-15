import { afterEach, describe, expect, it } from 'vitest';
import { editorLanguageFallback } from '../monaco/language-support';
import { editorStore } from '../store/editor-store';
import {
  applyCliGraphResultToEditorStore,
  buildCliGraphDocumentKey,
  resolveCliGraphLanguage,
} from './apply-result';

describe('cli graph apply result', () => {
  afterEach(() => {
    editorStore.reset();
  });

  it('resolves supported CLI graph languages and falls back for unknown values', () => {
    expect(resolveCliGraphLanguage('json')).toBe('json');
    expect(resolveCliGraphLanguage('not-a-language')).toBe(editorLanguageFallback);
  });

  it('builds a token-scoped document key', () => {
    expect(buildCliGraphDocumentKey('secret')).toBe('cli:secret');
  });

  it('applies a CLI graph result to the shared editor store', () => {
    applyCliGraphResultToEditorStore('secret', {
      sourceLabel: 'input.json',
      expression: '.items',
      language: 'json',
      text: '[1,2]',
    });

    const state = editorStore.get();
    expect(state.documentKey).toBe('cli:secret');
    expect(state.sourceText).toBe('[1,2]');
    expect(state.languageId).toBe('json');
    expect(state.editorRevision).toBe(1);
    expect(state.graphAppliedRevision).toBe(0);
    expect(state.workspace.tabsById['cli-graph']?.name).toBe('input.json');
  });
});
