// Responsibility: prepare active documents for structure-definition generation.
import type { Settings } from '../../lib/settings/ui-settings';
import type { SupportedEditorLanguageId } from '../../lib/monaco/language-support';

type FormattingSettings = Settings['formatting'];

export type PrepareStructGenerationSourceInput = {
  text: string;
  language: SupportedEditorLanguageId;
  formatting: FormattingSettings;
  callWorker: <T>(method: string, input: unknown) => Promise<T>;
};

/** Returns JSON source accepted by the structure-generation API. */
export async function prepareStructGenerationSource(input: PrepareStructGenerationSourceInput): Promise<string> {
  if (input.language === 'json') return input.text;

  return input.callWorker<string>('convert', {
    sourceLanguage: input.language,
    targetFormat: 'json',
    text: input.text,
    options: input.formatting,
  });
}
