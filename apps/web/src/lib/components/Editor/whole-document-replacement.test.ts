import { describe, expect, it, vi } from 'vitest';
import { settleWholeDocumentReplacement } from './whole-document-replacement';

describe('settleWholeDocumentReplacement', () => {

  it('continues with the current language if language resolution fails', async () => {
    const onResolveLanguageError = vi.fn();
    const commitWholeDocumentReplacement = vi.fn(async () => {});

    const result = await settleWholeDocumentReplacement({
      text: 'not obvious',
      currentLanguage: 'json',
      shouldResolveLanguage: true,
      resolveLanguage: async () => {
        throw new Error('guess failed');
      },
      onResolveLanguageError,
      isStillCurrent: () => true,
      onDetectedLanguage: vi.fn(),
      commitWholeDocumentReplacement,
    });

    expect(result).toBe('json');
    expect(onResolveLanguageError).toHaveBeenCalledOnce();
    expect(commitWholeDocumentReplacement).toHaveBeenCalledWith('json');
  });
});
