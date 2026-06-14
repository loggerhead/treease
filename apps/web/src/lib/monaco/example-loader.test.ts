import { describe, expect, it } from 'vitest';
import { examplesByLanguage } from './example-loader';
import { exampleLanguageByExtension } from './language-support';

const exampleFiles = import.meta.glob('../../../../../example/*', {
  query: '?raw',
  import: 'default',
  eager: true,
});

describe('example-loader', () => {
  const entries = Object.entries(exampleFiles).sort(([a], [b]) => a.localeCompare(b));

  for (const [path, content] of entries) {
    const fileName = path.split('/').pop() ?? '';
    if (!fileName.startsWith('simple.')) continue;
    if (fileName.startsWith('simple.overrides.')) continue;
    const extension = fileName.split('.').pop();
    if (!extension) continue;
    const expected = exampleLanguageByExtension.get(extension);
    if (!expected) continue;

    it(`loads ${expected} example from ${fileName}`, () => {
      expect(examplesByLanguage.get(expected)).toBe(String(content ?? ''));
    });
  }
});
