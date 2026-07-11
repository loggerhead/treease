import type { SupportedEditorLanguageId } from '../../monaco/language-support';
import { createViewRuntimeOperation } from '../../guards/view-runtime-operation';

type ResolveLanguage = (text: string, currentLanguage: SupportedEditorLanguageId) => Promise<SupportedEditorLanguageId>;

type WholeDocumentReplacementOptions = {
  text: string;
  currentLanguage: SupportedEditorLanguageId;
  shouldResolveLanguage: boolean;
  resolveLanguage: ResolveLanguage;
  onResolveLanguageError: (error: unknown) => void;
  isStillCurrent: () => boolean;
  onDetectedLanguage: (language: SupportedEditorLanguageId) => void;
  commitWholeDocumentReplacement: (language: SupportedEditorLanguageId) => Promise<void> | void;
};

export async function settleWholeDocumentReplacement(
  options: WholeDocumentReplacementOptions,
): Promise<SupportedEditorLanguageId | null> {
  const operation = createViewRuntimeOperation({
    captured: { token: 1 },
    getCurrent: () => ({ token: options.isStillCurrent() ? 1 : 2 }),
  });
  const outcome = await operation.run({
    execute: async ({ step }) => {
      if (!options.shouldResolveLanguage) return options.currentLanguage;
      try {
        return await step(() => options.resolveLanguage(options.text, options.currentLanguage));
      } catch (error) {
        if (!operation.isCurrent()) throw error;
        options.onResolveLanguageError(error);
        return options.currentLanguage;
      }
    },
    land: async (finalLanguage) => {
      if (finalLanguage !== options.currentLanguage) options.onDetectedLanguage(finalLanguage);
      await operation.step(() => Promise.resolve(options.commitWholeDocumentReplacement(finalLanguage)));
    },
  });
  return outcome.status === 'completed' ? outcome.value : null;
}
