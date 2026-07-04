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
  const sampleJson =
    '{"object":{"int":42,"float":0.125,"bool":true,"nil":null},"table_without_header":["a","b","c"]}';

  test('clearing the sample JSON should not surface a graph syntax error card', async ({ page }) => {
    await page.goto('/editor');
    await waitForEditorReady(page);

    await setEditorContent(page, {
      sourceText: sampleJson,
      language: 'json',
    });
    await waitForGraphRendered(page);

    await replaceSourceByEdit(page, '');

    await expect(page.getByTestId('graph-diagnostic-syntax-error')).toHaveCount(0);
    await expect.poll(async () => (await readGraphClickProbes(page)).length, { timeout: 5_000 }).toBe(0);
  });

  test('replacing the sample JSON with a root scalar should not retain stale object or table probes', async ({ page }) => {
    await page.goto('/editor');
    await waitForEditorReady(page);

    await setEditorContent(page, {
      sourceText: sampleJson,
      language: 'json',
    });
    await waitForGraphRendered(page);

    await replaceSourceByEdit(page, '123');

    await expect
      .poll(
        async () => {
          const probes = await readGraphClickProbes(page);
          return {
            hasRootScalar: probes.some((probe) => probe.text === '123' && probe.path.length === 0),
            stalePaths: probes
              .map((probe) => probe.path.join('.'))
              .filter((path) => path === 'object' || path.startsWith('object.') || path === 'table_without_header' || path.startsWith('table_without_header.')),
          };
        },
        { timeout: 5_000 },
      )
      .toEqual({ hasRootScalar: true, stalePaths: [] });
  });
});
