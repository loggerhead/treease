import type { SupportedLanguageId } from './language-support';
import { callWasm } from './monaco/shared-api';

export async function guessLanguage(input: string): Promise<SupportedLanguageId | null> {
  return callWasm((mod) => ((mod as any).guess_language_wasm(input) as SupportedLanguageId | null) ?? null);
}
