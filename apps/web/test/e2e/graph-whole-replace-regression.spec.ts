import type { Page } from '@playwright/test';
import { expect, test } from './fixtures';
import {
  applyMonacoEdits,
  readGraphClickProbes,
  setEditorContent,
  waitForEditorReady,
  waitForGraphRendered,
} from './utils';

function fullDocumentRange(text: string) {
  const lines = text.split('\n');
  return {
    startLineNumber: 1,
    startColumn: 1,
    endLineNumber: lines.length,
    endColumn: (lines.at(-1)?.length ?? 0) + 1,
  };
}

async function replaceSourceByEdit(page: Page, nextText: string) {
  const currentText = await page.evaluate(() => window._treease?.editor.getValue('source-editor') ?? '');
  await applyMonacoEdits(page, 'source-editor', [
    {
      range: fullDocumentRange(currentText),
      text: nextText,
    },
  ]);
  await waitForGraphRendered(page);
}

test.describe('graph whole replace regressions', () => {
  test('clearing the sample JSON should not surface a graph syntax error card', async ({ page }) => {
    await page.goto('/editor');
    await waitForEditorReady(page);

    await setEditorContent(page, {
      sourceText: '{"object":{"int":42,"float":0.125,"bool":true,"nil":null},"table_without_header":["a","b","c"]}',
      language: 'json',
    });
    await waitForGraphRendered(page);

    await replaceSourceByEdit(page, '');

    await expect(page.getByTestId('graph-diagnostic-syntax-error')).toHaveCount(0);
    await expect.poll(async () => (await readGraphClickProbes(page)).length, { timeout: 5_000 }).toBe(0);
  });
});
