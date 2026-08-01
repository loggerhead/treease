import { afterEach, describe, expect, it } from 'vitest';

import {
  clearJsonBlockSelectionForDocument,
  getJsonBlockSelectionSnapshot,
  setJsonBlockSelection,
} from './full-edit-ui-store';

describe('JSON block selection state', () => {
  afterEach(() => setJsonBlockSelection(null));

  it('clears only the selection belonging to the requested document', () => {
    setJsonBlockSelection({
      sourceDocumentKey: 'document-1',
      blockDocumentKey: 'block-1',
      revision: 1,
      language: 'json',
      text: '{"a"',
      startByte: 0,
      endByte: 4,
      startLineNumber: 1,
      startColumn: 1,
      endLineNumber: 1,
      endColumn: 5,
    });

    clearJsonBlockSelectionForDocument('document-2');
    expect(getJsonBlockSelectionSnapshot()?.sourceDocumentKey).toBe('document-1');

    clearJsonBlockSelectionForDocument('document-1');
    expect(getJsonBlockSelectionSnapshot()).toBeNull();
  });
});
