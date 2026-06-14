import type { SupportedEditorLanguageId } from '../../monaco/language-support';

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
  let finalLanguage = options.currentLanguage;
  if (options.shouldResolveLanguage) {
    try {
      finalLanguage = await options.resolveLanguage(options.text, options.currentLanguage);
    } catch (error) {
      options.onResolveLanguageError(error);
      finalLanguage = options.currentLanguage;
    }
  }
  if (!options.isStillCurrent()) return null;
  if (finalLanguage !== options.currentLanguage) {
    options.onDetectedLanguage(finalLanguage);
  }
  if (!options.isStillCurrent()) return null;
  await options.commitWholeDocumentReplacement(finalLanguage);
  return finalLanguage;
}
