import { beforeEach, describe, expect, it } from 'vitest';
import {
  clearActiveDocumentSemanticState,
  isActiveDocumentSemanticPending,
  isActiveDocumentSemanticValid,
  markActiveDocumentSemanticInvalid,
  markActiveDocumentSemanticPending,
  markActiveDocumentSemanticValid,
  shouldSuppressJsonBlockFallback,
} from './active-document-authority';

describe('active document authority fallback helpers', () => {
  beforeEach(() => {
    clearActiveDocumentSemanticState();
  });

  it('keeps whole-document revisions on the whole-document path after parseFailed', () => {
    markActiveDocumentSemanticValid({
      documentKey: 'doc-json',
      language: 'json',
      revision: 1,
      snapshotId: 1 as any,
    });
    markActiveDocumentSemanticPending({
      documentKey: 'doc-json',
      language: 'json',
      revision: 2,
    });
    markActiveDocumentSemanticInvalid({
      documentKey: 'doc-json',
      language: 'json',
      revision: 2,
      snapshotId: 2 as any,
    });

    expect(isActiveDocumentSemanticPending('doc-json', 2)).toBe(false);
    expect(isActiveDocumentSemanticValid('doc-json', 2)).toBe(false);
    expect(shouldSuppressJsonBlockFallback('doc-json', 2)).toBe(true);
  });

  it('keeps fresh invalid documents eligible for JSON block fallback', () => {
    markActiveDocumentSemanticPending({
      documentKey: 'doc-jsonl',
      language: 'json',
      revision: 1,
    });

    expect(isActiveDocumentSemanticPending('doc-jsonl', 1)).toBe(true);
    expect(shouldSuppressJsonBlockFallback('doc-jsonl', 1)).toBe(false);

    markActiveDocumentSemanticInvalid({
      documentKey: 'doc-jsonl',
      language: 'json',
      revision: 1,
      snapshotId: 3 as any,
    });

    expect(isActiveDocumentSemanticPending('doc-jsonl', 1)).toBe(false);
    expect(isActiveDocumentSemanticValid('doc-jsonl', 1)).toBe(false);
    expect(shouldSuppressJsonBlockFallback('doc-jsonl', 1)).toBe(false);
  });
});
