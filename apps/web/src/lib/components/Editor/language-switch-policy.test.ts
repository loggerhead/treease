import { describe, expect, it } from 'vitest';
import { resolveLanguageSwitchPolicy } from './language-switch-policy';

describe('resolveLanguageSwitchPolicy', () => {
  it('uses the next language example before user input', () => {
    const result = resolveLanguageSwitchPolicy({
      nextLanguage: 'yaml',
      hasUserInput: false,
      currentText: '{"example": true}',
      nextExampleText: 'example: true\n',
    });

    expect(result).toEqual({
      kind: 'example',
      language: 'yaml',
      text: 'example: true\n',
      reason: 'language-example',
    });
  });

  it('preserves user input and reprocesses with the next language after editing', () => {
    const userInput = 'name: Alice\nitems:\n  - one\n';

    const result = resolveLanguageSwitchPolicy({
      nextLanguage: 'toml',
      hasUserInput: true,
      currentText: userInput,
      nextExampleText: 'title = "Example"\n',
    });

    expect(result).toEqual({
      kind: 'preserve-input',
      language: 'toml',
      text: userInput,
      reason: 'language-switch',
    });
  });
});
