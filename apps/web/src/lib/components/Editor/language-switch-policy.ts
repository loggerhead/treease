import type { SupportedEditorLanguageId } from '../../monaco/language-support';

export type LanguageSwitchPolicyInput = {
  nextLanguage: SupportedEditorLanguageId;
  hasUserInput: boolean;
  currentText: string;
  nextExampleText: string;
};

export type LanguageSwitchPolicyResult =
  | {
      kind: 'example';
      language: SupportedEditorLanguageId;
      text: string;
      reason: 'language-example';
    }
  | {
      kind: 'preserve-input';
      language: SupportedEditorLanguageId;
      text: string;
      reason: 'language-switch';
    };

export function resolveLanguageSwitchPolicy(input: LanguageSwitchPolicyInput): LanguageSwitchPolicyResult {
  if (!input.hasUserInput) {
    return {
      kind: 'example',
      language: input.nextLanguage,
      text: input.nextExampleText,
      reason: 'language-example',
    };
  }

  return {
    kind: 'preserve-input',
    language: input.nextLanguage,
    text: input.currentText,
    reason: 'language-switch',
  };
}
