import type { SupportedEditorLanguageId } from './language-support';
import { examplesByLanguage } from './example-loader';

export function getLanguageExample(languageId: SupportedEditorLanguageId): string {
  return examplesByLanguage.get(languageId) ?? '';
}
