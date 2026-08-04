import { expect, test } from './fixtures';
import {
  getMonacoValue,
  setEditorContent,
  waitForEditorRuntimeReady,
  waitForGraphRendered,
} from './utils';

test('document analysis failure leaves Editor editable and Graph recoverable', async ({ page }, testInfo) => {
  testInfo.annotations.push({ type: 'allow-browser-error', description: '[graph] document analysis failed' });
  await page.goto('/editor');
  await waitForEditorRuntimeReady(page);

  const invalidSource = '{"broken":';
  await setEditorContent(page, { sourceText: invalidSource, language: 'json' });

  await expect.poll(() => getMonacoValue(page, 'source-editor')).toBe(invalidSource);
  await expect(page.getByTestId('graph-error-message')).toHaveCount(0);

  // A failed document task is local to Graph/diagnostics; Monaco remains an
  // interactive surface and accepts a replacement without a page error.
  await setEditorContent(page, { sourceText: '{"recovered":true}', language: 'json' });
  await expect.poll(() => getMonacoValue(page, 'source-editor')).toBe('{"recovered":true}');
  await waitForGraphRendered(page);
  await expect(page.getByTestId('graph-error-message')).toHaveCount(0);
});

test('Graph failure does not replace the editable surface with an error banner', async ({ page }, testInfo) => {
  testInfo.annotations.push({ type: 'allow-browser-error', description: '[graph] document analysis failed' });
  await page.goto('/editor');
  await waitForEditorRuntimeReady(page);
  await setEditorContent(page, { sourceText: '{"broken":', language: 'json' });

  await expect(page.getByTestId('graph-error-message')).toHaveCount(0);
  await expect(page.getByTestId('monaco-source-editor')).toBeVisible();
  await expect(page.getByRole('status', { name: 'Editor loading status' })).toHaveCount(0);
});
