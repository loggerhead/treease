import { describe, expect, it } from 'vitest';
import { isDocumentRevisionGuardCurrent } from './document-revision-guard';

describe('document-revision-guard', () => {
  it('accepts the same document and revision', () => {
    const guard: { documentKey: string; revision: number } = {
      documentKey: 'doc-key',
      revision: 3,
    };

    expect(
      isDocumentRevisionGuardCurrent(guard, {
        documentKey: 'doc-key',
        revision: 3,
      }),
    ).toBe(true);
  });

  it('rejects a different revision', () => {
    const guard: { documentKey: string; revision: number } = {
      documentKey: 'doc-key',
      revision: 3,
    };

    expect(
      isDocumentRevisionGuardCurrent(guard, {
        documentKey: 'doc-key',
        revision: 4,
      }),
    ).toBe(false);
  });
});
